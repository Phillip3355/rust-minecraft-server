#!/usr/bin/env python3
"""Aggregate samples.csv + ticks.jsonl/log metrics into summary.json and Markdown matrices.

Follows docs/BENCHMARK_PROTOCOL.md §5.4 and §6.4 strictly: missing or uncollected
values remain null (JSON) / TBD (Markdown), never fabricated.
"""

from __future__ import annotations

import argparse
import csv
import json
import statistics
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple


def read_proc_samples(samples_csv: Path, phase_filter: str = "measure") -> List[Dict[str, float]]:
    if not samples_csv.is_file():
        return []
    rows_all: List[Dict[str, float]] = []
    rows_phase: List[Dict[str, float]] = []
    with open(samples_csv, "r", encoding="utf-8") as fh:
        reader = csv.DictReader(fh)
        for row in reader:
            parsed: Dict[str, float] = {}
            for k in (
                "rss_mib",
                "pss_mib",
                "rust_rss_mib",
                "jvm_pss_mib",
                "cpu_total_pct",
                "cpu_user_pct",
                "cpu_sys_pct",
                "read_bytes_total",
                "write_bytes_total",
            ):
                if row.get(k) not in (None, ""):
                    parsed[k] = float(row[k])
            rows_all.append(parsed)
            if row.get("phase") == phase_filter:
                rows_phase.append(parsed)
    return rows_phase if rows_phase else rows_all


def _mean(vals: List[float]) -> Optional[float]:
    return round(sum(vals) / len(vals), 3) if vals else None


def _max(vals: List[float]) -> Optional[float]:
    return round(max(vals), 3) if vals else None


def build_summary(
    run_id: str,
    profile: str,
    scenario: str,
    environment: Dict[str, Any],
    samples_csv: Path,
    log_metrics: Dict[str, Any],
    world_size_bytes: Optional[int],
    measure_sec: float,
    bot_completion_rate: Optional[float] = None,
) -> Dict[str, Any]:
    samples = read_proc_samples(samples_csv, phase_filter="measure")

    rss_vals = [r["rss_mib"] for r in samples if "rss_mib" in r]
    pss_vals = [r["pss_mib"] for r in samples if "pss_mib" in r]
    rust_rss_vals = [r["rust_rss_mib"] for r in samples if "rust_rss_mib" in r]
    jvm_pss_vals = [r["jvm_pss_mib"] for r in samples if "jvm_pss_mib" in r]
    cpu_tot = [r["cpu_total_pct"] for r in samples if "cpu_total_pct" in r]
    cpu_usr = [r["cpu_user_pct"] for r in samples if "cpu_user_pct" in r]
    cpu_sys = [r["cpu_sys_pct"] for r in samples if "cpu_sys_pct" in r]

    read_mb_total = None
    write_mb_total = None
    if samples and "read_bytes_total" in samples[0] and "read_bytes_total" in samples[-1]:
        read_mb_total = round(
            max(0.0, samples[-1]["read_bytes_total"] - samples[0]["read_bytes_total"]) / (1024 * 1024),
            3,
        )
    if samples and "write_bytes_total" in samples[0] and "write_bytes_total" in samples[-1]:
        write_mb_total = round(
            max(0.0, samples[-1]["write_bytes_total"] - samples[0]["write_bytes_total"]) / (1024 * 1024),
            3,
        )

    chunks_gen = log_metrics.get("chunk_gen", {}).get("chunks_generated")
    chunks_per_sec = (
        round(chunks_gen / measure_sec, 3)
        if isinstance(chunks_gen, (int, float)) and measure_sec > 0
        else None
    )

    tps_in = log_metrics.get("tps", {})
    mspt_in = log_metrics.get("mspt_ms", {})
    cg_in = log_metrics.get("chunk_gen", {})
    par_in = log_metrics.get("parity_checks", {})

    return {
        "run_id": run_id,
        "profile": profile,
        "scenario": scenario,
        "environment": environment,
        "metrics": {
            "tps": {
                "mean": tps_in.get("mean"),
                "min": tps_in.get("min"),
                "pct_below_20": tps_in.get("pct_below_20"),
            },
            "mspt_ms": {
                "mean": mspt_in.get("mean"),
                "p50": mspt_in.get("p50"),
                "p95": mspt_in.get("p95"),
                "p99": mspt_in.get("p99"),
                "max": mspt_in.get("max"),
                "pct_over_50ms": mspt_in.get("pct_over_50ms"),
            },
            "memory_mib": {
                "total_tree_rss_mean": _mean(rss_vals),
                "total_tree_pss_mean": _mean(pss_vals),
                "total_tree_pss_peak": _max(pss_vals),
                "rust_rss_mean": _mean(rust_rss_vals),
                "jvm_pss_mean": _mean(jvm_pss_vals),
            },
            "cpu_pct": {
                "process_tree_mean": _mean(cpu_tot),
                "user_mean": _mean(cpu_usr),
                "sys_mean": _mean(cpu_sys),
            },
            "chunk_gen": {
                "chunks_generated": chunks_gen,
                "chunks_per_sec": chunks_per_sec,
                "ms_per_chunk_p50": cg_in.get("ms_per_chunk_p50"),
                "ms_per_chunk_p95": cg_in.get("ms_per_chunk_p95"),
            },
            "disk_io": {
                "read_mb_total": read_mb_total,
                "write_mb_total": write_mb_total,
                "world_size_mib": round(world_size_bytes / (1024 * 1024), 3)
                if world_size_bytes is not None
                else None,
            },
        },
        "parity_checks": {
            "loaded_chunks_end": par_in.get("loaded_chunks_end"),
            "active_entities_end": par_in.get("active_entities_end"),
            "bot_completion_rate": bot_completion_rate,
            "errors_or_disconnects": par_in.get("errors_or_disconnects"),
        },
    }


def _fmt_med_range(vals: List[Optional[float]]) -> str:
    clean = [v for v in vals if isinstance(v, (int, float))]
    if not clean:
        return "TBD"
    med = round(statistics.median(clean), 2)
    if len(clean) == 1:
        return f"{med}"
    return f"{med} ({round(min(clean), 2)}–{round(max(clean), 2)})"


def render_matrix_markdown(results_root: Path) -> str:
    summaries: List[Dict[str, Any]] = []
    for path in sorted(results_root.glob("*/summary.json")):
        summaries.append(json.loads(path.read_text(encoding="utf-8")))

    if not summaries:
        return "No summary.json files found.\n"

    grouped: Dict[Tuple[str, str], List[Dict[str, Any]]] = {}
    for s in summaries:
        grouped.setdefault((s["scenario"], s["profile"]), []).append(s)

    lines = [
        "| 시나리오 | 프로파일 | 반복수 | TPS Mean (Med/Range) | MSPT p50 / p95 / p99 (ms) | PSS Mean / Peak (MiB) | CPU Mean (%) | 청크 생성 (chunks/s) | Disk Write (MB) |",
        "|---|---|---:|---|---|---|---|---|---|",
    ]
    for (scen, prof), items in sorted(grouped.items()):
        tps_str = _fmt_med_range([x["metrics"]["tps"]["mean"] for x in items])
        p50 = _fmt_med_range([x["metrics"]["mspt_ms"]["p50"] for x in items])
        p95 = _fmt_med_range([x["metrics"]["mspt_ms"]["p95"] for x in items])
        p99 = _fmt_med_range([x["metrics"]["mspt_ms"]["p99"] for x in items])
        pss_mean = _fmt_med_range([x["metrics"]["memory_mib"]["total_tree_pss_mean"] for x in items])
        pss_peak = _fmt_med_range([x["metrics"]["memory_mib"]["total_tree_pss_peak"] for x in items])
        cpu_str = _fmt_med_range([x["metrics"]["cpu_pct"]["process_tree_mean"] for x in items])
        cg_str = _fmt_med_range([x["metrics"]["chunk_gen"]["chunks_per_sec"] for x in items])
        dw_str = _fmt_med_range([x["metrics"]["disk_io"]["write_mb_total"] for x in items])
        lines.append(
            f"| `{scen}` | `{prof}` | {len(items)} | {tps_str} | {p50} / {p95} / {p99} | {pss_mean} / {pss_peak} | {cpu_str} | {cg_str} | {dw_str} |"
        )
    return "\n".join(lines) + "\n"


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Generate summary.json or Markdown matrix report")
    parser.add_argument("--matrix-dir", type=Path, help="Directory containing <run_id>/summary.json folders")
    parser.add_argument("--run-dir", type=Path, help="Single run directory containing manifest.json, samples.csv, server.log")
    parser.add_argument("--out", type=Path, help="Optional output file path")
    args = parser.parse_args(argv)

    if args.matrix_dir:
        md = render_matrix_markdown(args.matrix_dir)
        if args.out:
            args.out.write_text(md, encoding="utf-8")
        else:
            print(md, end="")
        return 0

    if args.run_dir:
        from collectors.log_parser import parse_server_log

        manifest_json = json.loads((args.run_dir / "manifest.json").read_text(encoding="utf-8"))
        log_metrics = parse_server_log(
            args.run_dir / "server.log",
            manifest_json["environment"]["engine"],
            args.run_dir / "ticks.jsonl",
        )
        summary = build_summary(
            run_id=manifest_json["run_id"],
            profile=manifest_json["profile"],
            scenario=manifest_json["scenario"],
            environment=manifest_json["environment"],
            samples_csv=args.run_dir / "samples.csv",
            log_metrics=log_metrics,
            world_size_bytes=manifest_json.get("world_size_bytes"),
            measure_sec=float(manifest_json.get("measure_sec", 0)),
        )
        out_path = args.out or (args.run_dir / "summary.json")
        out_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
        return 0

    parser.error("Provide either --matrix-dir or --run-dir")
    return 2


if __name__ == "__main__":
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    sys.exit(main())
