#!/usr/bin/env python3
"""1Hz OS process-tree sampler using Linux /proc (with optional pidstat sidecar).

Collects RSS, PSS, SwapPss, user/sys CPU%, context switches, thread counts,
and disk read/write bytes across a root PID and all descendant processes.
Separates Rust vs JVM processes for Phase 3 PatchBukkit accounting.
"""

from __future__ import annotations

import argparse
import csv
import os
import shutil
import signal
import subprocess
import sys
import threading
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Optional, Set, Tuple

CLK_TCK = os.sysconf("SC_CLK_TCK")


@dataclass
class ProcSnapshot:
    pid: int
    role: str  # "rust" | "jvm" | "other"
    rss_kib: int
    pss_kib: int
    swap_pss_kib: int
    utime_ticks: int
    stime_ticks: int
    num_threads: int
    vol_ctxt: int
    nonvol_ctxt: int
    read_bytes: int
    write_bytes: int


def _read_text(path: Path) -> Optional[str]:
    try:
        return path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None


def classify_pid(pid: int) -> str:
    """Classify a process as 'jvm', 'rust', or 'other' using comm/cmdline."""
    comm = (_read_text(Path(f"/proc/{pid}/comm")) or "").strip().lower()
    cmdline_raw = _read_text(Path(f"/proc/{pid}/cmdline")) or ""
    cmdline = cmdline_raw.replace("\x00", " ").lower()
    if comm == "java" or "/bin/java" in cmdline or "paper" in cmdline and ".jar" in cmdline:
        return "jvm"
    if "pumpkin" in comm or "pumpkin" in cmdline:
        return "rust"
    return "other"


def discover_process_tree(root_pid: int) -> List[int]:
    """Return sorted list of root_pid and all descendant PIDs currently alive."""
    if not Path(f"/proc/{root_pid}").exists():
        return []

    ppid_map: Dict[int, List[int]] = {}
    for entry in os.listdir("/proc"):
        if not entry.isdigit():
            continue
        pid = int(entry)
        stat_txt = _read_text(Path(f"/proc/{pid}/stat"))
        if not stat_txt:
            continue
        rparen = stat_txt.rfind(")")
        if rparen == -1:
            continue
        rest = stat_txt[rparen + 2 :].split()
        if len(rest) >= 2:
            try:
                ppid = int(rest[1])
            except ValueError:
                continue
            ppid_map.setdefault(ppid, []).append(pid)

    seen: Set[int] = set()
    stack = [root_pid]
    while stack:
        cur = stack.pop()
        if cur in seen or not Path(f"/proc/{cur}").exists():
            continue
        seen.add(cur)
        # Check /proc/<pid>/task/*/children first, plus ppid_map fallback.
        task_dir = Path(f"/proc/{cur}/task")
        try:
            for tid in os.listdir(task_dir):
                children_txt = _read_text(task_dir / tid / "children")
                if children_txt:
                    for tok in children_txt.split():
                        if tok.isdigit():
                            stack.append(int(tok))
        except OSError:
            pass
        for child in ppid_map.get(cur, []):
            stack.append(child)

    return sorted(seen)


def read_proc_snapshot(pid: int) -> Optional[ProcSnapshot]:
    """Read /proc metrics for a single PID. Returns None if process exited."""
    stat_txt = _read_text(Path(f"/proc/{pid}/stat"))
    if not stat_txt:
        return None
    rparen = stat_txt.rfind(")")
    if rparen == -1:
        return None
    fields = stat_txt[rparen + 2 :].split()
    # fields[0]=state, [1]=ppid, ..., [11]=utime, [12]=stime, ..., [17]=num_threads
    if len(fields) < 18:
        return None
    try:
        utime = int(fields[11])
        stime = int(fields[12])
        num_threads = int(fields[17])
    except ValueError:
        return None

    rss_kib = 0
    pss_kib = 0
    swap_pss_kib = 0
    smaps = _read_text(Path(f"/proc/{pid}/smaps_rollup"))
    if smaps:
        for line in smaps.splitlines():
            parts = line.split()
            if len(parts) >= 2 and parts[1].isdigit():
                val = int(parts[1])
                if line.startswith("Rss:"):
                    rss_kib = val
                elif line.startswith("Pss:"):
                    pss_kib = val
                elif line.startswith("SwapPss:"):
                    swap_pss_kib = val

    vol_ctxt = 0
    nonvol_ctxt = 0
    status_txt = _read_text(Path(f"/proc/{pid}/status"))
    if status_txt:
        for line in status_txt.splitlines():
            parts = line.split()
            if len(parts) >= 2 and parts[1].isdigit():
                val = int(parts[1])
                if line.startswith("VmRSS:") and rss_kib == 0:
                    rss_kib = val
                elif line.startswith("voluntary_ctxt_switches:"):
                    vol_ctxt = val
                elif line.startswith("nonvoluntary_ctxt_switches:"):
                    nonvol_ctxt = val
    if pss_kib == 0 and rss_kib > 0:
        pss_kib = rss_kib

    read_bytes = 0
    write_bytes = 0
    io_txt = _read_text(Path(f"/proc/{pid}/io"))
    if io_txt:
        for line in io_txt.splitlines():
            if line.startswith("read_bytes:"):
                read_bytes = int(line.split()[1])
            elif line.startswith("write_bytes:"):
                write_bytes = int(line.split()[1])

    return ProcSnapshot(
        pid=pid,
        role=classify_pid(pid),
        rss_kib=rss_kib,
        pss_kib=pss_kib,
        swap_pss_kib=swap_pss_kib,
        utime_ticks=utime,
        stime_ticks=stime,
        num_threads=num_threads,
        vol_ctxt=vol_ctxt,
        nonvol_ctxt=nonvol_ctxt,
        read_bytes=read_bytes,
        write_bytes=write_bytes,
    )


CSV_COLUMNS = [
    "timestamp_utc",
    "elapsed_sec",
    "phase",
    "pid_count",
    "total_threads",
    "rss_mib",
    "pss_mib",
    "swap_pss_mib",
    "rust_rss_mib",
    "rust_pss_mib",
    "jvm_rss_mib",
    "jvm_pss_mib",
    "cpu_total_pct",
    "cpu_user_pct",
    "cpu_sys_pct",
    "vol_ctxt_delta",
    "nonvol_ctxt_delta",
    "read_bytes_total",
    "write_bytes_total",
    "read_mb_s",
    "write_mb_s",
]


class ProcSampler:
    """Background 1Hz sampler for a server process tree."""

    def __init__(
        self,
        root_pid: int,
        out_csv: Path,
        interval_sec: float = 1.0,
        with_pidstat: bool = False,
        pidstat_log: Optional[Path] = None,
    ) -> None:
        self.root_pid = root_pid
        self.out_csv = Path(out_csv)
        self.interval_sec = interval_sec
        self.with_pidstat = with_pidstat
        self.pidstat_log = pidstat_log or self.out_csv.with_name("pidstat.log")
        self._phase = "warmup"
        self._phase_lock = threading.Lock()
        self._stop_event = threading.Event()
        self._thread: Optional[threading.Thread] = None
        self._pidstat_proc: Optional[subprocess.Popen[bytes]] = None
        self._prev_time: Optional[float] = None
        self._prev_cpu: Dict[int, Tuple[int, int]] = {}
        self._prev_ctxt: Dict[int, Tuple[int, int]] = {}
        self._prev_io: Dict[int, Tuple[int, int]] = {}
        self.sample_count = 0

    def set_phase(self, phase: str) -> None:
        with self._phase_lock:
            self._phase = phase

    def _get_phase(self) -> str:
        with self._phase_lock:
            return self._phase

    def start(self) -> None:
        self.out_csv.parent.mkdir(parents=True, exist_ok=True)
        if self.with_pidstat and shutil.which("pidstat"):
            log_fh = open(self.pidstat_log, "wb")
            self._pidstat_proc = subprocess.Popen(
                ["pidstat", "-u", "-r", "-d", "-w", "-p", str(self.root_pid), "1"],
                stdout=log_fh,
                stderr=subprocess.STDOUT,
            )
            self._pidstat_log_fh = log_fh
        else:
            self._pidstat_log_fh = None

        self._thread = threading.Thread(target=self._run_loop, name="proc-sampler", daemon=True)
        self._thread.start()

    def stop(self) -> None:
        self._stop_event.set()
        if self._thread is not None:
            self._thread.join(timeout=max(2.0, self.interval_sec * 2))
        if self._pidstat_proc is not None:
            try:
                self._pidstat_proc.send_signal(signal.SIGTERM)
                self._pidstat_proc.wait(timeout=3)
            except (ProcessLookupError, subprocess.TimeoutExpired):
                self._pidstat_proc.kill()
        if getattr(self, "_pidstat_log_fh", None) is not None:
            self._pidstat_log_fh.close()

    def sample_once(self, start_mono: float) -> Optional[Dict[str, object]]:
        pids = discover_process_tree(self.root_pid)
        if not pids:
            return None
        snaps: List[ProcSnapshot] = []
        for pid in pids:
            snap = read_proc_snapshot(pid)
            if snap is not None:
                snaps.append(snap)
        if not snaps:
            return None

        now_mono = time.monotonic()
        dt = (now_mono - self._prev_time) if self._prev_time is not None else 0.0

        du_ticks = 0
        ds_ticks = 0
        dvol = 0
        dnonvol = 0
        dread = 0
        dwrite = 0

        cur_cpu: Dict[int, Tuple[int, int]] = {}
        cur_ctxt: Dict[int, Tuple[int, int]] = {}
        cur_io: Dict[int, Tuple[int, int]] = {}

        for s in snaps:
            cur_cpu[s.pid] = (s.utime_ticks, s.stime_ticks)
            cur_ctxt[s.pid] = (s.vol_ctxt, s.nonvol_ctxt)
            cur_io[s.pid] = (s.read_bytes, s.write_bytes)
            if s.pid in self._prev_cpu:
                pu, ps = self._prev_cpu[s.pid]
                du_ticks += max(0, s.utime_ticks - pu)
                ds_ticks += max(0, s.stime_ticks - ps)
                pv, pnv = self._prev_ctxt[s.pid]
                dvol += max(0, s.vol_ctxt - pv)
                dnonvol += max(0, s.nonvol_ctxt - pnv)
                pr, pw = self._prev_io[s.pid]
                dread += max(0, s.read_bytes - pr)
                dwrite += max(0, s.write_bytes - pw)

        self._prev_time = now_mono
        self._prev_cpu = cur_cpu
        self._prev_ctxt = cur_ctxt
        self._prev_io = cur_io

        user_pct = (du_ticks / CLK_TCK / dt * 100.0) if dt > 0 else 0.0
        sys_pct = (ds_ticks / CLK_TCK / dt * 100.0) if dt > 0 else 0.0
        read_mb_s = (dread / (1024 * 1024) / dt) if dt > 0 else 0.0
        write_mb_s = (dwrite / (1024 * 1024) / dt) if dt > 0 else 0.0

        kib_to_mib = 1.0 / 1024.0
        return {
            "timestamp_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "elapsed_sec": round(now_mono - start_mono, 3),
            "phase": self._get_phase(),
            "pid_count": len(snaps),
            "total_threads": sum(s.num_threads for s in snaps),
            "rss_mib": round(sum(s.rss_kib for s in snaps) * kib_to_mib, 3),
            "pss_mib": round(sum(s.pss_kib for s in snaps) * kib_to_mib, 3),
            "swap_pss_mib": round(sum(s.swap_pss_kib for s in snaps) * kib_to_mib, 3),
            "rust_rss_mib": round(sum(s.rss_kib for s in snaps if s.role == "rust") * kib_to_mib, 3),
            "rust_pss_mib": round(sum(s.pss_kib for s in snaps if s.role == "rust") * kib_to_mib, 3),
            "jvm_rss_mib": round(sum(s.rss_kib for s in snaps if s.role == "jvm") * kib_to_mib, 3),
            "jvm_pss_mib": round(sum(s.pss_kib for s in snaps if s.role == "jvm") * kib_to_mib, 3),
            "cpu_total_pct": round(user_pct + sys_pct, 2),
            "cpu_user_pct": round(user_pct, 2),
            "cpu_sys_pct": round(sys_pct, 2),
            "vol_ctxt_delta": dvol,
            "nonvol_ctxt_delta": dnonvol,
            "read_bytes_total": sum(s.read_bytes for s in snaps),
            "write_bytes_total": sum(s.write_bytes for s in snaps),
            "read_mb_s": round(read_mb_s, 4),
            "write_mb_s": round(write_mb_s, 4),
        }

    def _run_loop(self) -> None:
        start_mono = time.monotonic()
        with open(self.out_csv, "w", newline="", encoding="utf-8") as fh:
            writer = csv.DictWriter(fh, fieldnames=CSV_COLUMNS)
            writer.writeheader()
            next_tick = start_mono
            while not self._stop_event.is_set():
                row = self.sample_once(start_mono)
                if row is None:
                    break
                writer.writerow(row)
                fh.flush()
                self.sample_count += 1
                next_tick += self.interval_sec
                sleep_for = max(0.0, next_tick - time.monotonic())
                self._stop_event.wait(sleep_for)


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="1Hz /proc process-tree metric sampler")
    parser.add_argument("--pid", type=int, required=True, help="Root process PID to monitor")
    parser.add_argument("--out", type=Path, required=True, help="Output CSV path")
    parser.add_argument("--interval", type=float, default=1.0, help="Sampling interval in seconds")
    parser.add_argument("--duration", type=float, default=0.0, help="Duration in seconds (0 = until process exits or SIGINT)")
    parser.add_argument("--phase", default="measure", help="Initial phase label (warmup|measure)")
    parser.add_argument("--with-pidstat", action="store_true", help="Also run pidstat sidecar if installed")
    args = parser.parse_args(argv)

    sampler = ProcSampler(
        root_pid=args.pid,
        out_csv=args.out,
        interval_sec=args.interval,
        with_pidstat=args.with_pidstat,
    )
    sampler.set_phase(args.phase)
    sampler.start()
    try:
        if args.duration > 0:
            time.sleep(args.duration)
        else:
            while Path(f"/proc/{args.pid}").exists():
                time.sleep(0.5)
    except KeyboardInterrupt:
        pass
    finally:
        sampler.stop()
    return 0


if __name__ == "__main__":
    sys.exit(main())
