#!/usr/bin/env python3
"""Unit & local-integration tests for the Phase 0 benchmark harness."""

from __future__ import annotations

import csv
import json
import subprocess
import sys
import tempfile
import time
import tomllib
import unittest
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[2]
BENCH_DIR = PROJECT_ROOT / "scripts" / "benchmark"
sys.path.insert(0, str(BENCH_DIR))

from collectors.log_parser import parse_server_log
from collectors.proc_sampler import ProcSampler, discover_process_tree
from drivers.bot_runner import BotDriver, generate_scenario_plan
from report_summary import build_summary, render_matrix_markdown
from servers.common import load_manifest, verify_snapshot_sha256
from servers.paper_runner import PaperRunner
from servers.pumpkin_runner import PumpkinRunner


class TestBenchmarkHarness(unittest.TestCase):
    def setUp(self) -> None:
        self.manifest = load_manifest(BENCH_DIR / "bench_manifest.toml")

    def test_manifest_scenarios_and_profiles(self) -> None:
        for prof in ("paper-baseline", "pumpkin-native-anvil", "pumpkin-native-pump"):
            self.assertIn(prof, self.manifest["profiles"])
        for scen in ("S1-A", "S1-B", "S2", "S3", "S4"):
            self.assertIn(scen, self.manifest["scenarios"])
        self.assertIsInstance(self.manifest["seeds"]["vanilla_overworld"], int)

    def test_paper_template_rendering_standalone_and_proxy(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp) / "paper_run"
            runner = PaperRunner(
                project_root=PROJECT_ROOT,
                manifest=self.manifest,
                run_dir=run_dir,
                seed=1790155153199293024,
                view_distance=10,
                simulation_distance=10,
                proxy_mode="velocity",
            )
            runner.prepare(world_source=None)
            props = (run_dir / "server.properties").read_text(encoding="utf-8")
            self.assertIn("level-seed=1790155153199293024", props)
            self.assertIn("view-distance=10", props)
            self.assertIn("online-mode=false", props)
            pg = (run_dir / "config" / "paper-global.yml").read_text(encoding="utf-8")
            self.assertIn("enabled: true", pg)
            self.assertEqual((run_dir / "eula.txt").read_text(encoding="utf-8").strip(), "eula=true")
            cmd = runner.build_command()
            self.assertIn("-Xms3072M", cmd)
            self.assertIn("--nogui", cmd)

    def test_pumpkin_template_rendering_anvil_and_pump(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp) / "pumpkin_run"
            runner = PumpkinRunner(
                project_root=PROJECT_ROOT,
                manifest=self.manifest,
                run_dir=run_dir,
                seed=1790155153199293024,
                view_distance=10,
                simulation_distance=8,
                chunk_format="anvil",
            )
            runner.prepare(world_source=None)
            parsed = tomllib.loads((run_dir / "pumpkin.toml").read_text(encoding="utf-8"))
            self.assertEqual(parsed["seed"], "1790155153199293024")
            self.assertEqual(parsed["networking"]["java"]["view_distance"], 10)
            self.assertEqual(parsed["networking"]["java"]["simulation_distance"], 8)
            self.assertFalse(parsed["commands"]["use_tty"])
            self.assertTrue(parsed["commands"]["use_console"])
            self.assertEqual(parsed["world"]["chunk"]["type"], "anvil")
            self.assertTrue(parsed["networking"]["management"]["enabled"])

    def test_proc_sampler_real_subprocess_tree(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out_csv = Path(tmp) / "samples.csv"
            # Spawn a parent python process that itself spawns a child sleep process.
            child_script = (
                "import subprocess, time; "
                "p = subprocess.Popen(['python3', '-c', 'import time; time.sleep(5)']); "
                "time.sleep(5); p.terminate(); p.wait()"
            )
            proc = subprocess.Popen([sys.executable, "-c", child_script])
            try:
                time.sleep(0.3)
                pids = discover_process_tree(proc.pid)
                self.assertGreaterEqual(len(pids), 2)

                sampler = ProcSampler(root_pid=proc.pid, out_csv=out_csv, interval_sec=0.25)
                sampler.set_phase("warmup")
                sampler.start()
                time.sleep(0.6)
                sampler.set_phase("measure")
                time.sleep(0.7)
                sampler.stop()
            finally:
                proc.terminate()
                proc.wait(timeout=5)

            with out_csv.open("r", encoding="utf-8") as fh:
                rows = list(csv.DictReader(fh))
            self.assertGreaterEqual(len(rows), 3)
            phases = {r["phase"] for r in rows}
            self.assertIn("measure", phases)
            self.assertGreater(float(rows[-1]["rss_mib"]), 0.0)
            self.assertGreater(float(rows[-1]["pss_mib"]), 0.0)
            self.assertGreaterEqual(int(rows[-1]["pid_count"]), 2)

    def test_log_parser_paper_and_pumpkin(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            p_log = Path(tmp) / "paper.log"
            p_log.write_text(
                "\n".join([
                    "[12:00:01 INFO]: TPS from last 1m, 5m, 15m: *20.0, 20.0, 20.0",
                    "[12:00:01 INFO]: Server tick times (avg/min/max) from last 5s, 10s, 1m:",
                    "[12:00:01 INFO]: ◴ 4.2/2.1/11.5, 4.3/2.0/12.0, 4.5/1.9/14.2",
                    "[12:00:06 INFO]: TPS from last 1m, 5m, 15m: 19.8, 19.9, 20.0",
                    "[12:00:06 INFO]: Loaded chunks: 1250",
                    "[12:00:06 INFO]: Active entities: 412",
                ]),
                encoding="utf-8",
            )
            res = parse_server_log(p_log, "paper", Path(tmp) / "paper_ticks.jsonl")
            self.assertEqual(res["tps"]["min"], 19.8)
            self.assertEqual(res["tps"]["pct_below_20"], 50.0)
            self.assertEqual(res["mspt_ms"]["mean"], 4.2)
            self.assertEqual(res["mspt_ms"]["max"], 11.5)
            self.assertEqual(res["parity_checks"]["loaded_chunks_end"], 1250)
            self.assertEqual(res["parity_checks"]["active_entities_end"], 412)

            k_log = Path(tmp) / "pumpkin.log"
            k_log.write_text(
                "\n".join([
                    "[12:00:01 INFO]: TPS: 20.0 | MSPT: 2.4ms | p50: 2.1ms p95: 3.8ms p99: 5.5ms",
                    "[12:00:02 INFO]: Generated chunk in 1.25ms",
                    "[12:00:03 INFO]: Generated chunk in 2.75ms",
                ]),
                encoding="utf-8",
            )
            k_res = parse_server_log(k_log, "pumpkin", Path(tmp) / "pumpkin_ticks.jsonl")
            self.assertEqual(k_res["tps"]["mean"], 20.0)
            self.assertEqual(k_res["mspt_ms"]["p95"], 3.8)
            self.assertEqual(k_res["chunk_gen"]["chunks_generated"], 2)
            self.assertAlmostEqual(k_res["chunk_gen"]["ms_per_chunk_p50"], 2.0, places=2)

    def test_report_summary_schema_and_null_preservation(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            samples_csv = tmp_path / "samples.csv"
            samples_csv.write_text(
                "\n".join([
                    "timestamp_utc,elapsed_sec,phase,pid_count,total_threads,rss_mib,pss_mib,swap_pss_mib,rust_rss_mib,rust_pss_mib,jvm_rss_mib,jvm_pss_mib,cpu_total_pct,cpu_user_pct,cpu_sys_pct,vol_ctxt_delta,nonvol_ctxt_delta,read_bytes_total,write_bytes_total,read_mb_s,write_mb_s",
                    "2026-11-17T00:00:01Z,1.0,measure,1,8,100.0,90.0,0.0,100.0,90.0,0.0,0.0,50.0,45.0,5.0,10,2,1048576,2097152,1.0,2.0",
                    "2026-11-17T00:00:02Z,2.0,measure,1,8,120.0,110.0,0.0,120.0,110.0,0.0,0.0,60.0,54.0,6.0,12,1,2097152,4194304,1.0,2.0",
                ]),
                encoding="utf-8",
            )
            summary = build_summary(
                run_id="test_run_r1",
                profile="pumpkin-native-anvil",
                scenario="S2",
                environment={"seed": 123},
                samples_csv=samples_csv,
                log_metrics={"tps": {}, "mspt_ms": {}, "chunk_gen": {}, "parity_checks": {}},
                world_size_bytes=10 * 1024 * 1024,
                measure_sec=2.0,
            )
            # Uncollected log metrics must stay None (never fabricated).
            self.assertIsNone(summary["metrics"]["tps"]["mean"])
            self.assertEqual(summary["metrics"]["memory_mib"]["total_tree_pss_mean"], 100.0)
            self.assertEqual(summary["metrics"]["memory_mib"]["total_tree_pss_peak"], 110.0)
            self.assertEqual(summary["metrics"]["disk_io"]["write_mb_total"], 2.0)
            self.assertEqual(summary["metrics"]["disk_io"]["world_size_mib"], 10.0)

            r_dir = tmp_path / "results" / "test_run_r1"
            r_dir.mkdir(parents=True)
            (r_dir / "summary.json").write_text(json.dumps(summary), encoding="utf-8")
            md = render_matrix_markdown(tmp_path / "results")
            self.assertIn("`S2`", md)
            self.assertIn("TBD", md)

    def test_bot_driver_plans_and_snapshot_verify(self) -> None:
        s3_plan = generate_scenario_plan("radial_explore", 8)
        self.assertEqual(len(s3_plan["bots"]), 8)
        self.assertEqual(s3_plan["bots"][1]["azimuth_deg"], 45.0)

        with tempfile.TemporaryDirectory() as tmp:
            drv = BotDriver(PROJECT_ROOT, self.manifest, "soulfire", "walk_interact", 10, Path(tmp))
            drv.prepare()
            cmd = drv.build_command()
            self.assertIn("--bot-protocol-version", cmd)
            self.assertIn("26.3", cmd)

            snap = Path(tmp) / "snap"
            snap.mkdir()
            (snap / "level.dat").write_bytes(b"hello")
            import hashlib
            digest = hashlib.sha256(b"hello").hexdigest()
            manifest_file = Path(tmp) / "snap.sha256"
            manifest_file.write_text(f"{digest}  level.dat\n", encoding="utf-8")
            ok, _ = verify_snapshot_sha256(snap, manifest_file)
            self.assertTrue(ok)


if __name__ == "__main__":
    unittest.main()
