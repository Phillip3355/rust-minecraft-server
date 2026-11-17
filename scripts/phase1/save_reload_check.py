#!/usr/bin/env python3
"""Phase 1 save / reload / crash-recovery integrity check for Pumpkin (Anvil).

Drives the pinned Pumpkin binary through N start→stop cycles (optionally with a
SIGKILL crash cycle), snapshots the world directory after every stop and runs
`tools/world-verify` on each snapshot:

  verify   <snap>                      structural integrity (gate)
  diff     <prev> <snap>               lost chunks / lost players (gate)
  diff     <prev> <snap> --strict      changed blocks/inventories (informational
                                       unless --strict-gate: idle servers may
                                       still random-tick loaded spawn chunks)
  conserve <prev> <snap> --strict-loss item duplication / loss (gate)

Nothing is simulated: if the Pumpkin binary is missing the run is recorded as
NOT_RUN (exit 3), never as a pass. `--self-test` exercises this pipeline
offline against the verifier's synthetic fixture world (no server involved)
and is labelled as such in the result file.

Exit: 0 PASS, 1 FAIL (integrity gate), 2 usage/infra error, 3 NOT_RUN.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Any, Dict, List, Optional

ROOT = Path(__file__).resolve().parents[2]
BENCH = ROOT / "scripts" / "benchmark"
VERIFY_CRATE = ROOT / "tools" / "world-verify"
VERIFY_BIN = VERIFY_CRATE / "target" / "release" / "world-verify"

# Categories that fail the run when they appear as errors in a gating diff.
GATE_DIFF_CATEGORIES = {"chunk_lost", "player_lost", "level_dat"}


def now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds")


def ensure_verifier() -> Path:
    if not VERIFY_BIN.is_file():
        r = subprocess.run(
            ["cargo", "build", "--offline", "--release", "--manifest-path", str(VERIFY_CRATE / "Cargo.toml")],
            capture_output=True,
            text=True,
        )
        if r.returncode != 0 or not VERIFY_BIN.is_file():
            sys.stderr.write(r.stderr)
            raise SystemExit("error: could not build tools/world-verify (cargo build --offline --release)")
    return VERIFY_BIN


def run_verify(bin_: Path, args: List[str], json_out: Path) -> Dict[str, Any]:
    cmd = [str(bin_), *args, "--json", str(json_out)]
    r = subprocess.run(cmd, capture_output=True, text=True)
    rec: Dict[str, Any] = {"cmd": cmd[1:], "exit": r.returncode}
    if json_out.is_file():
        try:
            rec["report"] = json.loads(json_out.read_text(encoding="utf-8"))
        except json.JSONDecodeError as e:
            rec["report_error"] = f"invalid JSON from world-verify: {e}"
    if r.returncode == 2:
        rec["stderr"] = r.stderr[-2000:]
    return rec


def error_categories(rec: Dict[str, Any]) -> List[str]:
    rep = rec.get("report") or {}
    return sorted({i["category"] for i in rep.get("issues", []) if i.get("severity") == "error"})


def check_snapshot(bin_: Path, prev: Optional[Path], snap: Path, out: Path, strict_gate: bool,
                   allow: List[str], expect_dv: Optional[int]) -> Dict[str, Any]:
    out.mkdir(parents=True, exist_ok=True)
    res: Dict[str, Any] = {"snapshot": str(snap), "checks": {}, "gate_failures": []}
    vargs = ["verify", str(snap)]
    if expect_dv is not None:
        vargs += ["--expect-data-version", str(expect_dv)]
    v = run_verify(bin_, vargs, out / "verify.json")
    res["checks"]["verify"] = v
    if v["exit"] != 0:
        res["gate_failures"].append(f"verify exit {v['exit']}: {error_categories(v)}")
    if prev is not None:
        d = run_verify(bin_, ["diff", str(prev), str(snap)], out / "diff.json")
        res["checks"]["diff"] = d
        bad = [c for c in error_categories(d) if c in GATE_DIFF_CATEGORIES]
        if bad or d["exit"] == 2:
            res["gate_failures"].append(f"diff: {bad or 'exit 2'}")
        ds = run_verify(bin_, ["diff", str(prev), str(snap), "--strict"], out / "diff_strict.json")
        res["checks"]["diff_strict"] = ds
        if strict_gate and ds["exit"] != 0:
            res["gate_failures"].append(f"diff --strict: {error_categories(ds)}")
        cargs = ["conserve", str(prev), str(snap), "--strict-loss"]
        for a in allow:
            cargs += ["--allow", a]
        c = run_verify(bin_, cargs, out / "conserve.json")
        res["checks"]["conserve"] = c
        bad = [x for x in error_categories(c) if x in ("item_duplication", "item_loss", "item_stack")]
        if bad or c["exit"] == 2:
            res["gate_failures"].append(f"conserve: {bad or 'exit 2'}")
    return res


def snapshot(world: Path, dest: Path) -> Path:
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree(world, dest, symlinks=True)
    return dest


def self_test(args: argparse.Namespace, bin_: Path, out: Path) -> Dict[str, Any]:
    """Offline pipeline check: fixture world, identical copy, then an injected dupe."""
    fx = out / "fixture-world"
    if fx.exists():
        shutil.rmtree(fx)
    r = subprocess.run([str(bin_), "fixture", str(fx), "--compression", "4"], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit(f"fixture generation failed: {r.stderr}")
    s0 = snapshot(fx, out / "snapshots" / "cycle-0")
    s1 = snapshot(fx, out / "snapshots" / "cycle-1")
    cycles = [
        {"cycle": 0, "kind": "self-test-baseline", **check_snapshot(bin_, None, s0, out / "checks" / "cycle-0", True, [], None)},
        {"cycle": 1, "kind": "self-test-identical-copy", **check_snapshot(bin_, s0, s1, out / "checks" / "cycle-1", True, [], None)},
    ]
    # Negative control: truncate a region file in a third snapshot; the gate MUST fail.
    s2 = snapshot(fx, out / "snapshots" / "cycle-2-negative-control")
    reg = s2 / "region" / "r.0.0.mca"
    reg.write_bytes(reg.read_bytes()[: 2 * 4096 + 50])
    neg = check_snapshot(bin_, s0, s2, out / "checks" / "cycle-2", True, [], None)
    neg_ok = len(neg["gate_failures"]) > 0
    cycles.append({"cycle": 2, "kind": "self-test-negative-control (must fail)", "detected": neg_ok, **neg})
    passed = all(not c["gate_failures"] for c in cycles[:2]) and neg_ok
    return {"mode": "self-test (synthetic fixture, NO Pumpkin server involved)", "cycles": cycles,
            "status": "SELF_TEST_PASS" if passed else "SELF_TEST_FAIL"}


def real_run(args: argparse.Namespace, bin_: Path, out: Path) -> Dict[str, Any]:
    sys.path.insert(0, str(BENCH))
    from servers.common import load_manifest  # noqa: E402
    from servers.pumpkin_runner import PumpkinRunner  # noqa: E402

    manifest = load_manifest(BENCH / "bench_manifest.toml")
    seed = int(args.seed if args.seed is not None else manifest["seeds"]["vanilla_overworld"])
    run_dir = out / "server"
    runner = PumpkinRunner(ROOT, manifest, run_dir, seed, args.view_distance, args.simulation_distance, "anvil")
    pre = runner.preflight()
    if pre:
        return {"mode": "pumpkin", "status": "NOT_RUN", "reason": pre}
    runner.prepare(Path(args.world_source) if args.world_source else None)
    world = run_dir / "world"
    cycles: List[Dict[str, Any]] = []
    prev: Optional[Path] = None
    plan = ["graceful"] * args.cycles
    if args.crash_cycle:
        plan.insert(min(1, len(plan)), "sigkill")
    for i, kind in enumerate(plan):
        rec: Dict[str, Any] = {"cycle": i, "kind": kind, "started": now()}
        pid = runner.start()
        rec["pid"] = pid
        ready = runner.wait_ready(args.ready_timeout)
        rec["ready"] = ready
        if ready:
            for line in (Path(args.console_script).read_text().splitlines() if args.console_script else []):
                if line.strip() and not line.lstrip().startswith("#"):
                    runner.send_command(line.strip())
                    time.sleep(0.5)
            time.sleep(args.idle_sec)
        rec["stop_exit"] = runner.stop(force_kill=(kind == "sigkill"))
        shutil.copy2(runner.log_path, out / f"server-cycle-{i}.log")
        rec["stopped"] = now()
        if not ready:
            rec["gate_failures"] = [f"server not ready within {args.ready_timeout}s (see server-cycle-{i}.log)"]
            cycles.append(rec)
            break
        if not world.is_dir():
            rec["gate_failures"] = [f"world directory {world} not created by server"]
            cycles.append(rec)
            break
        snap = snapshot(world, out / "snapshots" / f"cycle-{i}-{kind}")
        if kind == "sigkill":
            # A crash may legitimately lose unsaved progress; it must not corrupt
            # previously saved data. Verify structure now; the next graceful
            # cycle diffs against the last *clean* snapshot.
            rec.update(check_snapshot(bin_, None, snap, out / "checks" / f"cycle-{i}", False, [], args.expect_data_version))
        else:
            rec.update(check_snapshot(bin_, prev, snap, out / "checks" / f"cycle-{i}", args.strict_gate,
                                      args.allow, args.expect_data_version))
            prev = snap
        cycles.append(rec)
    failed = any(c.get("gate_failures") for c in cycles) or len(cycles) < len(plan)
    return {"mode": "pumpkin", "seed": seed, "plan": plan, "cycles": cycles, "status": "FAIL" if failed else "PASS"}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", default=str(ROOT / ".work" / "phase1" / dt.datetime.now().strftime("%Y%m%d-%H%M%S")))
    ap.add_argument("--self-test", action="store_true", help="offline pipeline check without a server")
    ap.add_argument("--cycles", type=int, default=3, help="graceful start/stop cycles (>=2 to get a reload diff)")
    ap.add_argument("--crash-cycle", action="store_true", help="insert one SIGKILL cycle after the first clean cycle")
    ap.add_argument("--world-source", help="copy this world in before cycle 0 (never modified in place)")
    ap.add_argument("--seed", type=int)
    ap.add_argument("--view-distance", type=int, default=10)
    ap.add_argument("--simulation-distance", type=int, default=10)
    ap.add_argument("--idle-sec", type=float, default=30.0)
    ap.add_argument("--ready-timeout", type=float, default=180.0)
    ap.add_argument("--console-script", help="file of console commands sent after ready (one per line)")
    ap.add_argument("--strict-gate", action="store_true", help="fail on any block/inventory change across reload")
    ap.add_argument("--allow", action="append", default=[], help="item id excluded from conservation")
    ap.add_argument("--expect-data-version", type=int, help="assert chunk DataVersion (take from a vanilla 26.3 world)")
    args = ap.parse_args()

    out = Path(args.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    bin_ = ensure_verifier()
    result = self_test(args, bin_, out) if args.self_test else real_run(args, bin_, out)
    result.update({"tool": "phase1/save_reload_check.py", "finished": now(), "out": str(out)})
    (out / "result.json").write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(f"status: {result['status']}  ({out / 'result.json'})")
    for c in result.get("cycles", []):
        print(f"  cycle {c['cycle']} [{c['kind']}]: gate_failures={c.get('gate_failures', [])}")
    if result["status"] == "NOT_RUN":
        for r in result.get("reason", []):
            print(f"  NOT_RUN: {r}")
    return {"PASS": 0, "SELF_TEST_PASS": 0, "FAIL": 1, "SELF_TEST_FAIL": 1, "NOT_RUN": 3}.get(result["status"], 2)


if __name__ == "__main__":
    sys.exit(main())
