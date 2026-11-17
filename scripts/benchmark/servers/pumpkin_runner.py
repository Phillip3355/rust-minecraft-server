#!/usr/bin/env python3
"""Pumpkin 0.2.2+26.3-26.60 server runner (pure Rust / Anvil or Pump chunk format)."""

from __future__ import annotations

import os
import re
import shutil
import signal
import socket
import subprocess
import time
from pathlib import Path
from typing import Any, Dict, List, Optional

from servers.common import render_template, wrap_cpu_ram_cmd

PUMPKIN_READY_RE = re.compile(r"(Listening on|You now can connect|Server started|Started Java)", re.IGNORECASE)


class PumpkinRunner:
    engine = "pumpkin"

    def __init__(
        self,
        project_root: Path,
        manifest: Dict[str, Any],
        run_dir: Path,
        seed: int,
        view_distance: int,
        simulation_distance: int,
        chunk_format: str = "anvil",
        proxy_mode: Optional[str] = None,
    ) -> None:
        self.project_root = project_root
        self.manifest = manifest
        self.run_dir = run_dir
        self.seed = seed
        self.view_distance = view_distance
        self.simulation_distance = simulation_distance
        self.chunk_format = chunk_format
        self.proxy_mode = proxy_mode or manifest["environment"].get("proxy_mode", "standalone")
        self.log_path = self.run_dir / "server.log"
        self.proc: Optional[subprocess.Popen[str]] = None
        self._log_fh = None

    def preflight(self) -> List[str]:
        errors: List[str] = []
        pumpkin_bin = self.project_root / self.manifest["pin"]["pumpkin_binary"]
        if not pumpkin_bin.is_file() or not os.access(pumpkin_bin, os.X_OK):
            errors.append(
                f"Pumpkin release binary not found or not executable at {pumpkin_bin}. "
                "Build via ./scripts/build.sh and copy/symlink target/x86_64-unknown-linux-gnu/release/pumpkin."
            )
        return errors

    def prepare(self, world_source: Optional[Path] = None) -> None:
        self.run_dir.mkdir(parents=True, exist_ok=True)
        defaults = self.manifest["defaults"]
        env_cfg = self.manifest["environment"]
        tmpl_dir = self.project_root / "tests" / "performance" / "configs" / "pumpkin"

        proxy_enabled = self.proxy_mode != "standalone"
        vars_map = {
            "SEED": self.seed,
            "VIEW_DISTANCE": self.view_distance,
            "SIMULATION_DISTANCE": self.simulation_distance,
            "BIND_HOST": env_cfg["bind_host"],
            "SERVER_PORT": env_cfg["server_port"],
            "MSMP_PORT": env_cfg.get("msmp_port", 25585),
            "MSMP_SECRET": env_cfg.get("msmp_secret", "phase0-local-bench-secret"),
            "DIFFICULTY": defaults["difficulty"],
            "GAMEMODE": defaults["gamemode"],
            "ONLINE_MODE": str(bool(defaults["online_mode"]) and not proxy_enabled).lower(),
            "WHITE_LIST": str(bool(defaults["white_list"])).lower(),
            "ENFORCE_WHITELIST": str(bool(defaults["enforce_whitelist"])).lower(),
            "SPAWN_PROTECTION": defaults["spawn_protection"],
            "MAX_PLAYERS": defaults["max_players"],
            "PROXY_ENABLED": str(proxy_enabled).lower(),
            "CHUNK_FORMAT": self.chunk_format,
        }

        render_template(tmpl_dir / "pumpkin.toml.tmpl", self.run_dir / "pumpkin.toml", vars_map)

        world_dest = self.run_dir / "world"
        if world_dest.exists():
            shutil.rmtree(world_dest)
        if world_source is not None:
            shutil.copytree(world_source, world_dest)
            for root, dirs, files in os.walk(world_dest):
                os.chmod(root, 0o755)
                for d in dirs:
                    os.chmod(Path(root) / d, 0o755)
                for f in files:
                    os.chmod(Path(root) / f, 0o644)

    def build_command(self) -> List[str]:
        env_cfg = self.manifest["environment"]
        pumpkin_bin = self.project_root / self.manifest["pin"]["pumpkin_binary"]
        return wrap_cpu_ram_cmd(
            [str(pumpkin_bin)],
            cpus=env_cfg.get("server_cpus"),
            ram_cap_mib=int(env_cfg.get("ram_cap_mib", 4096)),
            use_systemd_cgroup=bool(env_cfg.get("use_systemd_cgroup", False)),
        )

    def start(self) -> int:
        cmd = self.build_command()
        self._log_fh = open(self.log_path, "w", encoding="utf-8", buffering=1)
        self.proc = subprocess.Popen(
            cmd,
            cwd=str(self.run_dir),
            stdin=subprocess.PIPE,
            stdout=self._log_fh,
            stderr=subprocess.STDOUT,
            text=True,
        )
        return self.proc.pid

    def _port_open(self) -> bool:
        env_cfg = self.manifest["environment"]
        host = env_cfg["bind_host"]
        port = int(env_cfg["server_port"])
        try:
            with socket.create_connection((host, port), timeout=0.5):
                return True
        except OSError:
            return False

    def wait_ready(self, timeout_sec: float = 120.0) -> bool:
        deadline = time.monotonic() + timeout_sec
        while time.monotonic() < deadline:
            if self.proc is not None and self.proc.poll() is not None:
                return False
            if self.log_path.is_file():
                txt = self.log_path.read_text(encoding="utf-8", errors="replace")
                if PUMPKIN_READY_RE.search(txt):
                    return True
            if self._port_open():
                return True
            time.sleep(0.5)
        return False

    def send_command(self, cmd: str) -> None:
        if self.proc is not None and self.proc.stdin is not None and self.proc.poll() is None:
            try:
                self.proc.stdin.write(cmd.rstrip("\n") + "\n")
                self.proc.stdin.flush()
            except BrokenPipeError:
                pass

    def poll_metrics(self) -> None:
        self.send_command("tps")
        self.send_command("tick query")

    def stop(self, timeout_sec: float = 45.0, force_kill: bool = False) -> int:
        if self.proc is None:
            return 0
        if force_kill:
            self.proc.kill()
            rc = self.proc.wait(timeout=10)
        else:
            self.send_command("stop")
            try:
                rc = self.proc.wait(timeout=timeout_sec)
            except subprocess.TimeoutExpired:
                self.proc.send_signal(signal.SIGTERM)
                try:
                    rc = self.proc.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    self.proc.kill()
                    rc = self.proc.wait(timeout=10)
        if self._log_fh is not None:
            self._log_fh.close()
            self._log_fh = None
        return rc
