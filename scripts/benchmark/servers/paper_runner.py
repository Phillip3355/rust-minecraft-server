#!/usr/bin/env python3
"""Paper 26.3 server runner (Java 25+, standalone or Velocity-proxy mode)."""

from __future__ import annotations

import os
import re
import shutil
import signal
import subprocess
import time
from pathlib import Path
from typing import Any, Dict, List, Optional

from servers.common import parse_java_major, render_template, wrap_cpu_ram_cmd

PAPER_READY_RE = re.compile(r"Done \([0-9.]+s\)! For help, type \"help\"")


class PaperRunner:
    engine = "paper"

    def __init__(
        self,
        project_root: Path,
        manifest: Dict[str, Any],
        run_dir: Path,
        seed: int,
        view_distance: int,
        simulation_distance: int,
        proxy_mode: Optional[str] = None,
    ) -> None:
        self.project_root = project_root
        self.manifest = manifest
        self.run_dir = run_dir
        self.seed = seed
        self.view_distance = view_distance
        self.simulation_distance = simulation_distance
        self.proxy_mode = proxy_mode or manifest["environment"].get("proxy_mode", "standalone")
        self.log_path = self.run_dir / "server.log"
        self.proc: Optional[subprocess.Popen[str]] = None
        self._log_fh = None

    def preflight(self) -> List[str]:
        errors: List[str] = []
        pin = self.manifest["pin"]
        java_bin = pin.get("java_bin", "java")
        min_major = int(pin.get("java_min_major", 25))
        major = parse_java_major(java_bin)
        if major is None:
            errors.append(f"Java executable not found: {java_bin} (Paper 26.3 requires Java {min_major}+)")
        elif major < min_major:
            errors.append(f"Java {major} found, but Paper 26.3 requires Java {min_major}+")

        paper_jar = self.project_root / pin["paper_jar"]
        if not paper_jar.is_file():
            errors.append(
                f"Paper 26.3 jar not found at {paper_jar}. Run scripts/benchmark/fetch_artifacts.sh --paper."
            )
        return errors

    def prepare(self, world_source: Optional[Path] = None) -> None:
        self.run_dir.mkdir(parents=True, exist_ok=True)
        defaults = self.manifest["defaults"]
        env_cfg = self.manifest["environment"]
        tmpl_dir = self.project_root / "tests" / "performance" / "configs" / "paper"

        velocity_enabled = self.proxy_mode == "velocity"
        vars_map = {
            "SEED": self.seed,
            "VIEW_DISTANCE": self.view_distance,
            "SIMULATION_DISTANCE": self.simulation_distance,
            "BIND_HOST": env_cfg["bind_host"],
            "SERVER_PORT": env_cfg["server_port"],
            "DIFFICULTY": defaults["difficulty"],
            "GAMEMODE": defaults["gamemode"],
            "ONLINE_MODE": str(bool(defaults["online_mode"]) and not velocity_enabled).lower(),
            "WHITE_LIST": str(bool(defaults["white_list"])).lower(),
            "ENFORCE_WHITELIST": str(bool(defaults["enforce_whitelist"])).lower(),
            "SPAWN_PROTECTION": defaults["spawn_protection"],
            "MAX_PLAYERS": defaults["max_players"],
            "VELOCITY_ENABLED": str(velocity_enabled).lower(),
            "VELOCITY_SECRET": env_cfg.get("velocity_forwarding_secret", ""),
        }

        render_template(tmpl_dir / "server.properties.tmpl", self.run_dir / "server.properties", vars_map)
        render_template(tmpl_dir / "paper-global.yml.tmpl", self.run_dir / "config" / "paper-global.yml", vars_map)
        render_template(
            tmpl_dir / "paper-world-defaults.yml.tmpl",
            self.run_dir / "config" / "paper-world-defaults.yml",
            vars_map,
        )
        (self.run_dir / "eula.txt").write_text("eula=true\n", encoding="utf-8")

        world_dest = self.run_dir / "world"
        if world_dest.exists():
            shutil.rmtree(world_dest)
        if world_source is not None:
            shutil.copytree(world_source, world_dest)
            # Ensure copied read-only snapshot files are writable for the server run.
            for root, dirs, files in os.walk(world_dest):
                os.chmod(root, 0o755)
                for d in dirs:
                    os.chmod(Path(root) / d, 0o755)
                for f in files:
                    os.chmod(Path(root) / f, 0o644)

    def build_command(self) -> List[str]:
        pin = self.manifest["pin"]
        env_cfg = self.manifest["environment"]
        paper_jar = self.project_root / pin["paper_jar"]
        heap = int(env_cfg["jvm_heap_mib"])
        flags = list(env_cfg.get("jvm_flags", []))
        raw_cmd = [
            pin.get("java_bin", "java"),
            f"-Xms{heap}M",
            f"-Xmx{heap}M",
            *flags,
            "-jar",
            str(paper_jar),
            "--nogui",
        ]
        return wrap_cpu_ram_cmd(
            raw_cmd,
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

    def wait_ready(self, timeout_sec: float = 120.0) -> bool:
        deadline = time.monotonic() + timeout_sec
        while time.monotonic() < deadline:
            if self.proc is not None and self.proc.poll() is not None:
                return False
            if self.log_path.is_file():
                txt = self.log_path.read_text(encoding="utf-8", errors="replace")
                if PAPER_READY_RE.search(txt):
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
        self.send_command("mspt")

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
