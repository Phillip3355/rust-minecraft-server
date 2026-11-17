#!/usr/bin/env python3
"""Phase 0 ~ Phase 7 Benchmark Orchestrator (Paper 26.3 vs Pumpkin 26.3).

Implements the 8-step Run Lifecycle defined in docs/BENCHMARK_PROTOCOL.md §5.2.
Never fabricates performance numbers; missing binaries or snapshots fail preflight.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Any, Dict, List, Optional

sys.path.insert(0, str(Path(__file__).resolve().parent))

from collectors.log_parser import parse_server_log
from collectors.proc_sampler import ProcSampler
from drivers.bot_runner import BotDriver
from report_summary import build_summary, render_matrix_markdown
from servers.common import (
    collect_environment_fingerprint,
    dir_size_bytes,
    load_manifest,
    read_cpu_governor,
    sample_idle_cpu_pct,
    verify_snapshot_sha256,
)
from servers.paper_runner import PaperRunner
from servers.pumpkin_runner import PumpkinRunner


def resolve_seed(manifest: Dict[str, Any], scen_cfg: Dict[str, Any], seed_override: Optional[int]) -> int:
    if seed_override is not None:
        return int(seed_override)
    seed_key = scen_cfg.get("seed_key", "vanilla_overworld")
    return int(manifest["seeds"][seed_key])


def make_server_runner(
    project_root: Path,
    manifest: Dict[str, Any],
    profile_name: str,
    run_dir: Path,
    seed: int,
    view_distance: int,
    sim_distance: int,
    proxy_mode: Optional[str],
):
    prof = manifest["profiles"][profile_name]
    engine = prof["engine"]
    if engine == "paper":
        return PaperRunner(
            project_root=project_root,
            manifest=manifest,
            run_dir=run_dir,
            seed=seed,
            view_distance=view_distance,
            simulation_distance=sim_distance,
            proxy_mode=proxy_mode,
        )
    if engine == "pumpkin":
        return PumpkinRunner(
            project_root=project_root,
            manifest=manifest,
            run_dir=run_dir,
            seed=seed,
            view_distance=view_distance,
            simulation_distance=sim_distance,
            chunk_format=prof["chunk_format"],
            proxy_mode=proxy_mode,
        )
    raise ValueError(f"Profile '{profile_name}' (engine={engine}) is not yet executable in Phase 0.")


def run_single(
    project_root: Path,
    manifest: Dict[str, Any],
    profile_name: str,
    scenario_name: str,
    repeat_idx: int,
    args: argparse.Namespace,
) -> Dict[str, Any]:
    prof_cfg = manifest["profiles"][profile_name]
    scen_cfg = manifest["scenarios"][scenario_name]
    defaults = manifest["defaults"]
    env_cfg = manifest["environment"]

    seed = resolve_seed(manifest, scen_cfg, args.seed)
    view_dist = int(args.view_distance if args.view_distance is not None else scen_cfg.get("view_distance", defaults["view_distance"]))
    sim_dist = int(args.simulation_distance if args.simulation_distance is not None else scen_cfg.get("simulation_distance", defaults["simulation_distance"]))
    bot_count = int(args.bot_count if args.bot_count is not None else scen_cfg["bot_count"])
    driver_name = args.driver or scen_cfg["driver"]
    warmup_sec = float(args.warmup_sec if args.warmup_sec is not None else scen_cfg["warmup_sec"])
    measure_sec = float(args.measure_sec if args.measure_sec is not None else scen_cfg["measure_sec"])

    ts = time.strftime("%Y%m%dT%H%M%SZ", time.gmtime())
    run_id = f"{ts}_{profile_name}_{scenario_name}_r{repeat_idx}"
    results_dir = project_root / env_cfg["results_dir"] / run_id
    workdir = project_root / env_cfg["workdir_root"] / run_id

    runner = make_server_runner(
        project_root=project_root,
        manifest=manifest,
        profile_name=profile_name,
        run_dir=workdir,
        seed=seed,
        view_distance=view_dist,
        sim_distance=sim_dist,
        proxy_mode=args.proxy_mode,
    )
    bot_driver = BotDriver(
        project_root=project_root,
        manifest=manifest,
        driver_name=driver_name,
        driver_mode=scen_cfg["driver_mode"],
        bot_count=bot_count,
        run_dir=workdir,
        radius_blocks=int(scen_cfg.get("radius_blocks", 256)),
    )

    world_mode = scen_cfg["world_mode"]
    snapshot_dir = project_root / env_cfg["snapshot_dir"]
    snapshot_manifest = project_root / env_cfg["snapshot_manifest"]

    preflight_errors: List[str] = []
    preflight_errors.extend(runner.preflight())
    preflight_errors.extend(bot_driver.preflight())
    if world_mode == "snapshot":
        ok_snap, snap_msg = verify_snapshot_sha256(snapshot_dir, snapshot_manifest)
        if not ok_snap:
            preflight_errors.append(snap_msg)

    env_fp = collect_environment_fingerprint(
        manifest=manifest,
        engine=prof_cfg["engine"],
        chunk_format=prof_cfg["chunk_format"],
        seed=seed,
        view_distance=view_dist,
        sim_distance=sim_dist,
    )
    env_fp["idle_cpu_pct_before"] = sample_idle_cpu_pct(0.2)

    if args.dry_run:
        runner.prepare(world_source=None)
        bot_driver.prepare()
        return {
            "dry_run": True,
            "run_id": run_id,
            "profile": profile_name,
            "scenario": scenario_name,
            "workdir": str(workdir),
            "results_dir": str(results_dir),
            "environment": env_fp,
            "server_command": runner.build_command(),
            "bot_command": bot_driver.build_command(),
            "preflight_errors": preflight_errors,
        }

    if preflight_errors:
        raise RuntimeError("Preflight failed:\n  - " + "\n  - ".join(preflight_errors))

    results_dir.mkdir(parents=True, exist_ok=True)
    world_src = snapshot_dir if world_mode == "snapshot" else None
    runner.prepare(world_source=world_src)
    bot_driver.prepare()

    if env_cfg.get("cache_mode") == "cold":
        # Cold cache requires root privileges; record outcome honestly if attempted.
        subprocess.run(["sync"], check=False)
        drop_path = Path("/proc/sys/vm/drop_caches")
        if os.geteuid() == 0 and drop_path.exists():
            drop_path.write_text("3\n", encoding="utf-8")

    server_pid = runner.start()
    sampler = ProcSampler(
        root_pid=server_pid,
        out_csv=results_dir / "samples.csv",
        interval_sec=float(defaults.get("sample_interval_sec", 1.0)),
        with_pidstat=bool(args.with_pidstat),
    )

    try:
        if not runner.wait_ready(timeout_sec=float(defaults.get("ready_timeout_sec", 120))):
            raise RuntimeError(f"Server did not become ready within timeout. Check {runner.log_path}")

        sampler.set_phase("warmup")
        sampler.start()
        bot_driver.start()

        # Warmup phase
        time.sleep(warmup_sec)

        # Measurement phase
        sampler.set_phase("measure")
        poll_interval = float(defaults.get("console_poll_interval_sec", 5.0))
        measure_end = time.monotonic() + measure_sec
        while time.monotonic() < measure_end:
            runner.poll_metrics()
            remaining = measure_end - time.monotonic()
            if remaining <= 0:
                break
            time.sleep(min(poll_interval, remaining))

        bot_driver.stop()
        sampler.stop()

        force_kill = bool(scen_cfg.get("sigkill_recovery_check", False))
        runner.stop(timeout_sec=float(defaults.get("stop_timeout_sec", 45)), force_kill=force_kill)

        if force_kill:
            # S4 crash-recovery verification: restart once and verify clean boot + graceful stop.
            runner.start()
            if not runner.wait_ready(timeout_sec=float(defaults.get("ready_timeout_sec", 120))):
                raise RuntimeError("S4 post-SIGKILL recovery boot failed!")
            runner.stop(timeout_sec=float(defaults.get("stop_timeout_sec", 45)), force_kill=False)

    finally:
        bot_driver.stop()
        sampler.stop()
        if runner.proc is not None and runner.proc.poll() is None:
            runner.stop(timeout_sec=10, force_kill=True)

    world_bytes = dir_size_bytes(workdir / "world")
    if runner.log_path.is_file():
        shutil.copy2(runner.log_path, results_dir / "server.log")
    if bot_driver.log_path.is_file():
        shutil.copy2(bot_driver.log_path, results_dir / "bot_driver.log")

    manifest_out = {
        "run_id": run_id,
        "profile": profile_name,
        "scenario": scenario_name,
        "repeat": repeat_idx,
        "warmup_sec": warmup_sec,
        "measure_sec": measure_sec,
        "world_size_bytes": world_bytes,
        "environment": env_fp,
    }
    (results_dir / "manifest.json").write_text(json.dumps(manifest_out, indent=2) + "\n", encoding="utf-8")

    log_metrics = parse_server_log(
        results_dir / "server.log",
        prof_cfg["engine"],
        results_dir / "ticks.jsonl",
    )
    summary = build_summary(
        run_id=run_id,
        profile=profile_name,
        scenario=scenario_name,
        environment=env_fp,
        samples_csv=results_dir / "samples.csv",
        log_metrics=log_metrics,
        world_size_bytes=world_bytes,
        measure_sec=measure_sec,
    )
    (results_dir / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    return summary


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Phase 0 ~ Phase 7 Benchmark Orchestrator")
    parser.add_argument(
        "--manifest",
        type=Path,
        default=Path(__file__).resolve().parent / "bench_manifest.toml",
        help="Path to bench_manifest.toml",
    )
    parser.add_argument("--profile", default="paper-baseline", help="Comma-separated profile IDs")
    parser.add_argument("--scenario", default="S1-A", help="Comma-separated scenario IDs (S1-A,S1-B,S2,S3,S4)")
    parser.add_argument("--matrix", action="store_true", help="Run default Phase 0 matrix (paper-baseline,pumpkin-native-anvil x S1-A,S1-B,S2,S3)")
    parser.add_argument("--repeats", type=int, help="Override repeat count")
    parser.add_argument("--seed", type=int, help="Override 64-bit integer world seed")
    parser.add_argument("--view-distance", type=int, help="Override view-distance")
    parser.add_argument("--simulation-distance", type=int, help="Override simulation-distance")
    parser.add_argument("--bot-count", type=int, help="Override bot count")
    parser.add_argument("--driver", help="Override bot driver (soulfire|botmark|azalea|none)")
    parser.add_argument("--proxy-mode", choices=["standalone", "velocity"], help="Override server proxy mode")
    parser.add_argument("--warmup-sec", type=float, help="Override warmup duration (seconds)")
    parser.add_argument("--measure-sec", type=float, help="Override measure duration (seconds)")
    parser.add_argument("--with-pidstat", action="store_true", help="Also run pidstat sidecar if installed")
    parser.add_argument("--preflight", action="store_true", help="Check local environment readiness and exit")
    parser.add_argument("--dry-run", action="store_true", help="Render configs and print execution plan without starting servers")
    args = parser.parse_args(argv)

    project_root = Path(__file__).resolve().parents[2]
    manifest = load_manifest(args.manifest)

    if args.preflight:
        dummy_dir = project_root / ".work" / "benchmark" / "preflight_tmp"
        p_runner = PaperRunner(project_root, manifest, dummy_dir / "paper", 0, 10, 10)
        k_runner = PumpkinRunner(project_root, manifest, dummy_dir / "pumpkin", 0, 10, 10)
        sf_drv = BotDriver(project_root, manifest, "soulfire", "walk_interact", 1, dummy_dir / "sf")
        bm_drv = BotDriver(project_root, manifest, "botmark", "idle", 1, dummy_dir / "bm")
        snap_ok, snap_msg = verify_snapshot_sha256(
            project_root / manifest["environment"]["snapshot_dir"],
            project_root / manifest["environment"]["snapshot_manifest"],
        )
        report = {
            "cpu_governor": read_cpu_governor(),
            "idle_cpu_pct": sample_idle_cpu_pct(0.2),
            "tools": {
                "taskset": shutil.which("taskset") is not None,
                "pidstat": shutil.which("pidstat") is not None,
                "systemd_run": shutil.which("systemd-run") is not None,
            },
            "paper_errors": p_runner.preflight(),
            "pumpkin_errors": k_runner.preflight(),
            "soulfire_errors": sf_drv.preflight(),
            "botmark_errors": bm_drv.preflight(),
            "snapshot": {"ready": snap_ok, "detail": snap_msg},
        }
        print(json.dumps(report, indent=2))
        return 0

    if args.matrix:
        profiles = ["paper-baseline", "pumpkin-native-anvil"]
        scenarios = ["S1-A", "S1-B", "S2", "S3"]
    else:
        profiles = [p.strip() for p in args.profile.split(",") if p.strip()]
        scenarios = [s.strip() for s in args.scenario.split(",") if s.strip()]

    outputs: List[Dict[str, Any]] = []
    for scen in scenarios:
        scen_cfg = manifest["scenarios"][scen]
        repeats = 1 if args.dry_run else int(args.repeats or scen_cfg.get("repeats", manifest["defaults"]["repeats"]))
        for prof in profiles:
            for r in range(1, repeats + 1):
                out = run_single(project_root, manifest, prof, scen, r, args)
                outputs.append(out)

    if args.dry_run:
        print(json.dumps(outputs, indent=2))
        return 0

    results_root = project_root / manifest["environment"]["results_dir"]
    md = render_matrix_markdown(results_root)
    (results_root / "SUMMARY.md").write_text(md, encoding="utf-8")
    print(md)
    return 0


if __name__ == "__main__":
    sys.exit(main())
