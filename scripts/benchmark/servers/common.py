#!/usr/bin/env python3
"""Shared utilities for Paper 26.3 and Pumpkin 26.3 benchmark runners."""

from __future__ import annotations

import hashlib
import os
import platform
import re
import shutil
import subprocess
import time
import tomllib
from pathlib import Path
from string import Template
from typing import Any, Dict, List, Optional, Tuple


def load_manifest(manifest_path: Path) -> Dict[str, Any]:
    with open(manifest_path, "rb") as fh:
        return tomllib.load(fh)


def render_template(template_path: Path, output_path: Path, vars_map: Dict[str, Any]) -> None:
    raw = template_path.read_text(encoding="utf-8")
    rendered = Template(raw).substitute({k: str(v) for k, v in vars_map.items()})
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(rendered, encoding="utf-8")


def parse_java_major(java_bin: str) -> Optional[int]:
    """Return Java major version integer, or None if unavailable."""
    exe = shutil.which(java_bin) if not Path(java_bin).is_file() else java_bin
    if not exe:
        return None
    try:
        proc = subprocess.run([exe, "-version"], capture_output=True, text=True, timeout=5)
    except (OSError, subprocess.SubprocessError):
        return None
    combined = (proc.stdout or "") + "\n" + (proc.stderr or "")
    m = re.search(r'version\s+"(\d+)(?:\.(\d+))?', combined)
    if not m:
        return None
    major = int(m.group(1))
    if major == 1 and m.group(2):
        return int(m.group(2))
    return major


def read_cpu_governor() -> str:
    gov_path = Path("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
    if gov_path.exists():
        try:
            return gov_path.read_text(encoding="utf-8").strip()
        except OSError:
            return "unreadable"
    return "unavailable"


def read_cpu_model() -> str:
    cpuinfo = Path("/proc/cpuinfo")
    if cpuinfo.exists():
        for line in cpuinfo.read_text(encoding="utf-8", errors="replace").splitlines():
            if line.lower().startswith("model name"):
                return line.split(":", 1)[1].strip()
    return platform.processor() or "unknown"


def sample_idle_cpu_pct(window_sec: float = 0.5) -> Optional[float]:
    """Measure host CPU busy % over a short window via /proc/stat."""
    stat_path = Path("/proc/stat")
    if not stat_path.exists():
        return None

    def _read_cpu() -> Tuple[int, int]:
        line = stat_path.read_text(encoding="utf-8").splitlines()[0]
        parts = [int(x) for x in line.split()[1:]]
        idle = parts[3] + (parts[4] if len(parts) > 4 else 0)
        total = sum(parts)
        return idle, total

    i1, t1 = _read_cpu()
    time.sleep(window_sec)
    i2, t2 = _read_cpu()
    dt = t2 - t1
    di = i2 - i1
    if dt <= 0:
        return None
    return round((1.0 - (di / dt)) * 100.0, 2)


def collect_environment_fingerprint(manifest: Dict[str, Any], engine: str, chunk_format: str, seed: int, view_distance: int, sim_distance: int) -> Dict[str, Any]:
    env_cfg = manifest["environment"]
    pin_cfg = manifest["pin"]
    java_major = parse_java_major(pin_cfg.get("java_bin", "java"))
    return {
        "git_commit_sha": pin_cfg.get("pumpkin_commit"),
        "pumpkin_version": pin_cfg.get("pumpkin_version"),
        "paper_version": pin_cfg.get("paper_version"),
        "paper_build": pin_cfg.get("paper_build"),
        "java_major": java_major,
        "kernel": platform.release(),
        "cpu_model": read_cpu_model(),
        "cpu_governor": read_cpu_governor(),
        "pinned_cpus": env_cfg.get("server_cpus"),
        "bot_cpus": env_cfg.get("bot_cpus"),
        "ram_cap_mib": env_cfg.get("ram_cap_mib"),
        "proxy_mode": env_cfg.get("proxy_mode", "standalone"),
        "engine": engine,
        "chunk_format": chunk_format,
        "seed": seed,
        "view_distance": view_distance,
        "simulation_distance": sim_distance,
    }


def wrap_cpu_ram_cmd(
    cmd: List[str],
    cpus: Optional[str],
    ram_cap_mib: Optional[int] = None,
    use_systemd_cgroup: bool = False,
) -> List[str]:
    """Wrap command with taskset and optional systemd-run cgroup v2 limits."""
    wrapped = list(cmd)
    if cpus and shutil.which("taskset"):
        wrapped = ["taskset", "-c", str(cpus), *wrapped]
    if use_systemd_cgroup and ram_cap_mib and shutil.which("systemd-run"):
        prefix = [
            "systemd-run",
            "--user",
            "--scope",
            "-p",
            f"MemoryMax={int(ram_cap_mib)}M",
            "-p",
            "MemorySwapMax=0",
        ]
        if cpus:
            prefix.extend(["-p", f"AllowedCPUs={cpus}"])
        wrapped = [*prefix, *wrapped]
    return wrapped


def verify_snapshot_sha256(snapshot_dir: Path, manifest_file: Path) -> Tuple[bool, str]:
    """Verify Golden Snapshot files against a sha256sum manifest."""
    if not snapshot_dir.is_dir():
        return False, f"Snapshot directory missing: {snapshot_dir}"
    if not manifest_file.is_file():
        return False, f"Snapshot sha256 manifest missing: {manifest_file}"

    expected: Dict[str, str] = {}
    for line in manifest_file.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split(None, 1)
        if len(parts) != 2:
            continue
        digest, rel = parts[0], parts[1].lstrip("*").lstrip("./")
        expected[rel] = digest

    if not expected:
        return False, f"Snapshot manifest is empty: {manifest_file}"

    for rel, digest in expected.items():
        target = snapshot_dir / rel
        if not target.is_file():
            return False, f"Snapshot file missing: {target}"
        h = hashlib.sha256()
        with open(target, "rb") as fh:
            for chunk in iter(lambda: fh.read(1024 * 1024), b""):
                h.update(chunk)
        if h.hexdigest() != digest:
            return False, f"Checksum mismatch for {rel}: got {h.hexdigest()}, expected {digest}"

    return True, f"Verified {len(expected)} snapshot files"


def dir_size_bytes(path: Path) -> int:
    total = 0
    if not path.exists():
        return 0
    for root, _dirs, files in os.walk(path):
        for name in files:
            try:
                total += (Path(root) / name).stat().st_size
            except OSError:
                pass
    return total
