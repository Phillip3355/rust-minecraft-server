#!/usr/bin/env python3
"""Parse Paper 26.3 and Pumpkin 26.3 server logs into ticks.jsonl and structured aggregates.

Never fabricates metrics: missing or unprinted fields stay None.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional

ANSI_RE = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")

# Paper /tps: "TPS from last 1m, 5m, 15m: 20.0, 19.98, 19.95" (optional leading '*')
PAPER_TPS_RE = re.compile(
    r"TPS from last [^:]+:\s*\*?([0-9]+(?:\.[0-9]+)?)(?:,\s*\*?([0-9]+(?:\.[0-9]+)?))?(?:,\s*\*?([0-9]+(?:\.[0-9]+)?))?"
)
# Paper /mspt header + values line: "2.6/1.5/10.1, 2.6/1.5/10.1, 2.6/1.4/10.1"
PAPER_MSPT_HDR_RE = re.compile(r"Server tick times \(avg/min/max\)")
PAPER_MSPT_VAL_RE = re.compile(
    r"([0-9]+(?:\.[0-9]+)?)/([0-9]+(?:\.[0-9]+)?)/([0-9]+(?:\.[0-9]+)?)"
)

# Spark tick durations: "min/med/95%ile/max" -> "1.2/2.3/4.5/10.1"
SPARK_MSPT_HDR_RE = re.compile(r"Tick durations \(min/med/95%ile/max", re.IGNORECASE)
SPARK_MSPT_VAL_RE = re.compile(
    r"([0-9]+(?:\.[0-9]+)?)/([0-9]+(?:\.[0-9]+)?)/([0-9]+(?:\.[0-9]+)?)/([0-9]+(?:\.[0-9]+)?)"
)

# Pumpkin tps / tick query flexible patterns
PUMPKIN_TPS_RE = re.compile(r"\bTPS\b[^0-9]*([0-9]+(?:\.[0-9]+)?)", re.IGNORECASE)
PUMPKIN_MSPT_RE = re.compile(r"\bMSPT\b[^0-9]*([0-9]+(?:\.[0-9]+)?)", re.IGNORECASE)
PUMPKIN_PCT_RE = re.compile(
    r"(?:p50|median)[^0-9]*([0-9]+(?:\.[0-9]+)?)|(?:p95)[^0-9]*([0-9]+(?:\.[0-9]+)?)|(?:p99)[^0-9]*([0-9]+(?:\.[0-9]+)?)",
    re.IGNORECASE,
)

# Raw per-tick / chunk-gen structured log lines (optional patch/plugin output)
RAW_TICK_RE = re.compile(r"TICK_MSPT[=:]\s*([0-9]+(?:\.[0-9]+)?)")
CHUNK_GEN_MS_RE = re.compile(
    r"(?:Generated chunk(?:\s+in)?|CHUNK_GEN_MS)[=:\s]+([0-9]+(?:\.[0-9]+)?)\s*ms?",
    re.IGNORECASE,
)
CHUNK_GEN_COUNT_RE = re.compile(r"Generated\s+(\d+)\s+chunks?", re.IGNORECASE)

# Parity markers
LOADED_CHUNKS_RE = re.compile(r"(?:Loaded chunks|loaded_chunks)[=:\s]+(\d+)", re.IGNORECASE)
ACTIVE_ENTITIES_RE = re.compile(r"(?:Active entities|Entities|active_entities)[=:\s]+(\d+)", re.IGNORECASE)
DISCONNECT_ERR_RE = re.compile(
    r"(lost connection|disconnected with reason|Internal Exception|ERROR\]|FATAL\]|panic(?:ked)? at)",
    re.IGNORECASE,
)


def strip_ansi(line: str) -> str:
    return ANSI_RE.sub("", line).rstrip("\r\n")


def percentile(sorted_vals: List[float], pct: float) -> Optional[float]:
    if not sorted_vals:
        return None
    if len(sorted_vals) == 1:
        return round(sorted_vals[0], 4)
    idx = (len(sorted_vals) - 1) * (pct / 100.0)
    lo = int(idx)
    hi = min(lo + 1, len(sorted_vals) - 1)
    frac = idx - lo
    return round(sorted_vals[lo] * (1.0 - frac) + sorted_vals[hi] * frac, 4)


def parse_server_log(log_path: Path, engine: str, ticks_jsonl_out: Optional[Path] = None) -> Dict[str, Any]:
    """Parse a server log into ticks.jsonl events and return aggregated metrics."""
    lines = log_path.read_text(encoding="utf-8", errors="replace").splitlines()
    events: List[Dict[str, Any]] = []

    tps_samples: List[float] = []
    mspt_avg_samples: List[float] = []
    mspt_raw_samples: List[float] = []
    mspt_p50_samples: List[float] = []
    mspt_p95_samples: List[float] = []
    mspt_p99_samples: List[float] = []
    mspt_max_samples: List[float] = []
    chunk_gen_ms_samples: List[float] = []
    chunk_gen_total_count = 0
    loaded_chunks_end: Optional[int] = None
    active_entities_end: Optional[int] = None
    errors_or_disconnects = 0

    expect_paper_mspt = False
    expect_spark_mspt = False

    for lineno, raw_line in enumerate(lines, start=1):
        line = strip_ansi(raw_line)
        if not line.strip():
            continue

        if DISCONNECT_ERR_RE.search(line):
            errors_or_disconnects += 1

        if m := LOADED_CHUNKS_RE.search(line):
            loaded_chunks_end = int(m.group(1))
        if m := ACTIVE_ENTITIES_RE.search(line):
            active_entities_end = int(m.group(1))

        if m := RAW_TICK_RE.search(line):
            val = float(m.group(1))
            mspt_raw_samples.append(val)
            events.append({"line": lineno, "kind": "raw_tick", "mspt_ms": val})

        if m := CHUNK_GEN_MS_RE.search(line):
            val = float(m.group(1))
            chunk_gen_ms_samples.append(val)
            chunk_gen_total_count += 1
            events.append({"line": lineno, "kind": "chunk_gen", "ms_per_chunk": val})
        elif m := CHUNK_GEN_COUNT_RE.search(line):
            cnt = int(m.group(1))
            chunk_gen_total_count += cnt
            events.append({"line": lineno, "kind": "chunk_gen_batch", "count": cnt})

        # Paper /tps and /mspt
        if m := PAPER_TPS_RE.search(line):
            tps_1m = float(m.group(1))
            tps_samples.append(tps_1m)
            events.append({"line": lineno, "kind": "tps", "tps_1m": tps_1m, "source": "paper_tps"})
            continue

        if PAPER_MSPT_HDR_RE.search(line):
            expect_paper_mspt = True
            continue
        if expect_paper_mspt:
            expect_paper_mspt = False
            triplets = PAPER_MSPT_VAL_RE.findall(line)
            if triplets:
                # First triplet is shortest window (5s).
                avg_v, _min_v, max_v = (float(x) for x in triplets[0])
                mspt_avg_samples.append(avg_v)
                mspt_max_samples.append(max_v)
                events.append({"line": lineno, "kind": "mspt", "avg_ms": avg_v, "max_ms": max_v, "source": "paper_mspt"})
                continue

        if SPARK_MSPT_HDR_RE.search(line):
            expect_spark_mspt = True
            continue
        if expect_spark_mspt:
            expect_spark_mspt = False
            quads = SPARK_MSPT_VAL_RE.findall(line)
            if quads:
                _min_v, med_v, p95_v, max_v = (float(x) for x in quads[0])
                mspt_p50_samples.append(med_v)
                mspt_p95_samples.append(p95_v)
                mspt_max_samples.append(max_v)
                events.append({
                    "line": lineno,
                    "kind": "mspt",
                    "p50_ms": med_v,
                    "p95_ms": p95_v,
                    "max_ms": max_v,
                    "source": "spark",
                })
                continue

        # Pumpkin console output (tps / tick query)
        if engine.startswith("pumpkin"):
            ptps = PUMPKIN_TPS_RE.search(line)
            pmspt = PUMPKIN_MSPT_RE.search(line)
            pcts = PUMPKIN_PCT_RE.findall(line)
            if ptps or pmspt or pcts:
                ev: Dict[str, Any] = {"line": lineno, "kind": "pumpkin_stat", "source": "pumpkin_console"}
                if ptps:
                    val = float(ptps.group(1))
                    tps_samples.append(val)
                    ev["tps"] = val
                if pmspt:
                    val = float(pmspt.group(1))
                    mspt_avg_samples.append(val)
                    ev["mspt_ms"] = val
                for p50, p95, p99 in pcts:
                    if p50:
                        v = float(p50)
                        mspt_p50_samples.append(v)
                        ev["p50_ms"] = v
                    if p95:
                        v = float(p95)
                        mspt_p95_samples.append(v)
                        ev["p95_ms"] = v
                    if p99:
                        v = float(p99)
                        mspt_p99_samples.append(v)
                        ev["p99_ms"] = v
                events.append(ev)

    if ticks_jsonl_out is not None:
        ticks_jsonl_out.parent.mkdir(parents=True, exist_ok=True)
        with open(ticks_jsonl_out, "w", encoding="utf-8") as fh:
            for ev in events:
                fh.write(json.dumps(ev) + "\n")

    # Aggregate TPS
    tps_mean = round(sum(tps_samples) / len(tps_samples), 3) if tps_samples else None
    tps_min = round(min(tps_samples), 3) if tps_samples else None
    tps_pct_below_20 = (
        round(sum(1 for t in tps_samples if t < 20.0) / len(tps_samples) * 100.0, 2)
        if tps_samples
        else None
    )

    # Aggregate MSPT (prefer raw tick samples for true quantiles, fallback to reported quantiles/avgs)
    if mspt_raw_samples:
        s_raw = sorted(mspt_raw_samples)
        mspt_mean = round(sum(s_raw) / len(s_raw), 3)
        mspt_p50 = percentile(s_raw, 50.0)
        mspt_p95 = percentile(s_raw, 95.0)
        mspt_p99 = percentile(s_raw, 99.0)
        mspt_max = round(s_raw[-1], 3)
        pct_over_50 = round(sum(1 for x in s_raw if x > 50.0) / len(s_raw) * 100.0, 2)
    else:
        mspt_mean = round(sum(mspt_avg_samples) / len(mspt_avg_samples), 3) if mspt_avg_samples else None
        mspt_p50 = percentile(sorted(mspt_p50_samples), 50.0) if mspt_p50_samples else None
        mspt_p95 = percentile(sorted(mspt_p95_samples), 50.0) if mspt_p95_samples else None
        mspt_p99 = percentile(sorted(mspt_p99_samples), 50.0) if mspt_p99_samples else None
        mspt_max = round(max(mspt_max_samples), 3) if mspt_max_samples else (
            round(max(mspt_avg_samples), 3) if mspt_avg_samples else None
        )
        pct_over_50 = (
            round(sum(1 for x in mspt_avg_samples if x > 50.0) / len(mspt_avg_samples) * 100.0, 2)
            if mspt_avg_samples
            else None
        )

    s_chunk = sorted(chunk_gen_ms_samples)
    return {
        "tps": {
            "mean": tps_mean,
            "min": tps_min,
            "pct_below_20": tps_pct_below_20,
            "sample_count": len(tps_samples),
        },
        "mspt_ms": {
            "mean": mspt_mean,
            "p50": mspt_p50,
            "p95": mspt_p95,
            "p99": mspt_p99,
            "max": mspt_max,
            "pct_over_50ms": pct_over_50,
            "raw_tick_count": len(mspt_raw_samples),
        },
        "chunk_gen": {
            "chunks_generated": chunk_gen_total_count if chunk_gen_total_count > 0 else None,
            "ms_per_chunk_p50": percentile(s_chunk, 50.0),
            "ms_per_chunk_p95": percentile(s_chunk, 95.0),
        },
        "parity_checks": {
            "loaded_chunks_end": loaded_chunks_end,
            "active_entities_end": active_entities_end,
            "errors_or_disconnects": errors_or_disconnects,
        },
        "event_count": len(events),
    }


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Parse Paper/Pumpkin server log to ticks.jsonl + JSON summary")
    parser.add_argument("--log", type=Path, required=True, help="Path to server stdout/latest.log")
    parser.add_argument("--engine", required=True, choices=["paper", "pumpkin", "pumpkin-compat"])
    parser.add_argument("--ticks-out", type=Path, help="Output ticks.jsonl path")
    parser.add_argument("--json-out", type=Path, help="Optional parsed summary JSON path")
    args = parser.parse_args(argv)

    summary = parse_server_log(args.log, args.engine, args.ticks_out)
    payload = json.dumps(summary, indent=2)
    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(payload + "\n", encoding="utf-8")
    else:
        print(payload)
    return 0


if __name__ == "__main__":
    sys.exit(main())
