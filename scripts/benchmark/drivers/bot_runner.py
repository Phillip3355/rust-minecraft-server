#!/usr/bin/env python3
"""Unified deterministic load-driver wrapper for Phase 0 ~ 7 benchmarks.

Supports:
- soulfire: SoulFireCLI.jar (native MC 26.3 + offline accounts + SDK/scripted S2/S3 scenarios)
- botmark:  Pumpkin-MC/BotMark (lightweight Rust stress tool for S1-B idle / connection tests)
- azalea:   Custom azalea-rs 26.3 runner binary hook
- none:     0-player scenarios (S1-A)

Always pins bot processes to environment.bot_cpus via taskset so load generation
does not share cores with the server under test.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import signal
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from servers.common import load_manifest, parse_java_major, wrap_cpu_ram_cmd


def generate_accounts_file(out_path: Path, count: int, prefix: str = "bench_bot_") -> Path:
    out_path.parent.mkdir(parents=True, exist_ok=True)
    lines = [f"{prefix}{i:03d}" for i in range(1, count + 1)]
    out_path.write_text("\n".join(lines) + ("\n" if lines else ""), encoding="utf-8")
    return out_path


def generate_scenario_plan(mode: str, count: int, radius_blocks: int = 256) -> Dict[str, Any]:
    """Generate deterministic coordinates/trajectories for S1-B, S2, and S3."""
    bots: List[Dict[str, Any]] = []
    if mode == "idle":
        for i in range(count):
            angle = (2.0 * math.pi * i) / max(1, count)
            bots.append({
                "name": f"bench_bot_{i + 1:03d}",
                "anchor_x": round(8.0 * math.cos(angle), 2),
                "anchor_z": round(8.0 * math.sin(angle), 2),
                "action": "idle_keepalive",
            })
    elif mode == "walk_interact":
        # 5 deterministic cluster anchors within pregenerated safe zone
        clusters = [(64, 64), (-64, 64), (-64, -64), (64, -64), (0, 128)]
        for i in range(count):
            cx, cz = clusters[i % len(clusters)]
            offset = (i // len(clusters)) * 6
            bots.append({
                "name": f"bench_bot_{i + 1:03d}",
                "anchor_x": cx + offset,
                "anchor_z": cz + offset,
                "patrol_radius": min(32, max(16, radius_blocks // 8)),
                "actions": ["sprint_loop", "jump_periodic", "block_interact", "chat_10s"],
            })
    elif mode == "radial_explore":
        for i in range(count):
            deg = (360.0 * i) / max(1, count)
            rad = math.radians(deg)
            bots.append({
                "name": f"bench_bot_{i + 1:03d}",
                "start_x": 0.0,
                "start_z": 0.0,
                "azimuth_deg": round(deg, 2),
                "dir_x": round(math.cos(rad), 6),
                "dir_z": round(math.sin(rad), 6),
                "speed_blocks_per_sec": 5.6,
            })
    return {"mode": mode, "bot_count": count, "bots": bots}


class BotDriver:
    def __init__(
        self,
        project_root: Path,
        manifest: Dict[str, Any],
        driver_name: str,
        driver_mode: str,
        bot_count: int,
        run_dir: Path,
        radius_blocks: int = 256,
    ) -> None:
        self.project_root = project_root
        self.manifest = manifest
        self.driver_name = driver_name
        self.driver_mode = driver_mode
        self.bot_count = bot_count
        self.run_dir = run_dir
        self.radius_blocks = radius_blocks
        self.bot_dir = run_dir / "bots"
        self.log_path = self.bot_dir / "bot_driver.log"
        self.plan_path = self.bot_dir / "scenario_plan.json"
        self.proc: Optional[subprocess.Popen[str]] = None
        self._log_fh = None

    def preflight(self) -> List[str]:
        if self.driver_name == "none" or self.bot_count <= 0:
            return []
        errors: List[str] = []
        drv_cfg = self.manifest["drivers"]
        if self.driver_name == "botmark":
            bin_path = self.project_root / drv_cfg["botmark_bin"]
            if not bin_path.is_file() or not os.access(bin_path, os.X_OK):
                errors.append(
                    f"BotMark binary not found at {bin_path}. Run scripts/benchmark/fetch_artifacts.sh --botmark."
                )
        elif self.driver_name == "soulfire":
            jar_path = self.project_root / drv_cfg["soulfire_jar"]
            if not jar_path.is_file():
                errors.append(
                    f"SoulFireCLI.jar not found at {jar_path}. Run scripts/benchmark/fetch_artifacts.sh --soulfire."
                )
            java_bin = self.manifest["pin"].get("java_bin", "java")
            if parse_java_major(java_bin) is None:
                errors.append(f"Java runtime not found ({java_bin}) required for SoulFireCLI.jar.")
        elif self.driver_name == "azalea":
            bin_path = self.project_root / drv_cfg["azalea_bin"]
            if not bin_path.is_file() or not os.access(bin_path, os.X_OK):
                errors.append(f"Azalea scenario runner binary not found at {bin_path}.")
        else:
            errors.append(f"Unknown bot driver: {self.driver_name}")
        return errors

    def prepare(self) -> None:
        if self.driver_name == "none" or self.bot_count <= 0:
            return
        self.bot_dir.mkdir(parents=True, exist_ok=True)
        plan = generate_scenario_plan(self.driver_mode, self.bot_count, self.radius_blocks)
        self.plan_path.write_text(json.dumps(plan, indent=2) + "\n", encoding="utf-8")
        generate_accounts_file(self.bot_dir / "accounts.txt", self.bot_count)

    def build_command(self) -> List[str]:
        if self.driver_name == "none" or self.bot_count <= 0:
            return []
        env_cfg = self.manifest["environment"]
        drv_cfg = self.manifest["drivers"]
        target = f"{env_cfg['bind_host']}:{env_cfg['server_port']}"
        delay_ms = int(drv_cfg.get("join_delay_ms", 200))

        if self.driver_name == "botmark":
            bin_path = self.project_root / drv_cfg["botmark_bin"]
            enable_move = "true" if self.driver_mode != "idle" else "false"
            enable_jump = "true" if self.driver_mode == "walk_interact" else "false"
            enable_spam = "true" if self.driver_mode == "walk_interact" else "false"
            raw_cmd = [
                str(bin_path),
                "--ip",
                target,
                "--count",
                str(self.bot_count),
                "--delay",
                str(delay_ms),
                "--enable_movement",
                enable_move,
                "--enable_jumping",
                enable_jump,
                "--enable_spam_message",
                enable_spam,
            ]
        elif self.driver_name == "soulfire":
            jar_path = self.project_root / drv_cfg["soulfire_jar"]
            java_bin = self.manifest["pin"].get("java_bin", "java")
            proto_ver = str(drv_cfg.get("protocol_version", "26.3"))
            raw_cmd = [
                java_bin,
                "-Djava.awt.headless=true",
                "-Xmx2G",
                "-XX:+UseZGC",
                "-Dsf.flags.v2=true",
                "-jar",
                str(jar_path),
                "--account-file",
                str(self.bot_dir / "accounts.txt"),
                "--account-type",
                "OFFLINE",
                "--bot-address",
                target,
                "--bot-protocol-version",
                proto_ver,
                "--bot-join-delay-min",
                str(delay_ms),
                "--bot-join-delay-max",
                str(delay_ms + 150),
                "--start",
            ]
        elif self.driver_name == "azalea":
            bin_path = self.project_root / drv_cfg["azalea_bin"]
            raw_cmd = [
                str(bin_path),
                "--address",
                target,
                "--count",
                str(self.bot_count),
                "--plan",
                str(self.plan_path),
            ]
        else:
            raise ValueError(f"Unsupported driver: {self.driver_name}")

        return wrap_cpu_ram_cmd(raw_cmd, cpus=env_cfg.get("bot_cpus"))

    def start(self) -> Optional[int]:
        cmd = self.build_command()
        if not cmd:
            return None
        self._log_fh = open(self.log_path, "w", encoding="utf-8", buffering=1)
        self.proc = subprocess.Popen(
            cmd,
            cwd=str(self.bot_dir),
            stdout=self._log_fh,
            stderr=subprocess.STDOUT,
            text=True,
        )
        return self.proc.pid

    def stop(self) -> None:
        if self.proc is None:
            return
        if self.proc.poll() is None:
            self.proc.send_signal(signal.SIGTERM)
            try:
                self.proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait(timeout=5)
        if self._log_fh is not None:
            self._log_fh.close()
            self._log_fh = None


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Deterministic bot driver wrapper")
    parser.add_argument("--manifest", type=Path, default=Path(__file__).resolve().parents[1] / "bench_manifest.toml")
    parser.add_argument("--scenario", required=True, help="Scenario ID from bench_manifest.toml (e.g. S1-B, S2, S3)")
    parser.add_argument("--driver", help="Override driver (soulfire|botmark|azalea|none)")
    parser.add_argument("--count", type=int, help="Override bot count")
    parser.add_argument("--run-dir", type=Path, default=Path(".work/benchmark/driver_preview"))
    parser.add_argument("--dry-run", action="store_true", help="Render scenario plan & print command without executing")
    args = parser.parse_args(argv)

    project_root = Path(__file__).resolve().parents[3]
    manifest = load_manifest(args.manifest)
    scen = manifest["scenarios"][args.scenario]
    driver_name = args.driver or scen["driver"]
    bot_count = args.count if args.count is not None else int(scen["bot_count"])

    drv = BotDriver(
        project_root=project_root,
        manifest=manifest,
        driver_name=driver_name,
        driver_mode=scen["driver_mode"],
        bot_count=bot_count,
        run_dir=args.run_dir,
        radius_blocks=int(scen.get("radius_blocks", 256)),
    )
    drv.prepare()
    cmd = drv.build_command()
    preflight_errors = drv.preflight()
    print(json.dumps({
        "driver": driver_name,
        "mode": scen["driver_mode"],
        "bot_count": bot_count,
        "command": cmd,
        "plan_path": str(drv.plan_path) if bot_count > 0 else None,
        "preflight_errors": preflight_errors,
    }, indent=2))
    if args.dry_run:
        return 0
    if preflight_errors:
        return 2
    pid = drv.start()
    print(f"Started bot driver PID={pid}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
