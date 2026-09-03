#!/usr/bin/env python3

"""Compare paired performance measurements and enforce regression limits."""

from __future__ import annotations

import argparse
import json
import statistics
from pathlib import Path


METRICS = (
    ("cli_elapsed_ms", "time", "ms"),
    ("cli_max_rss_bytes", "memory", "bytes"),
    ("lsp_scan_ms", "time", "ms"),
    ("lsp_max_rss_bytes", "memory", "bytes"),
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-sha", required=True)
    parser.add_argument("--head-sha", required=True)
    parser.add_argument("--metrics", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--runs", required=True, type=int)
    parser.add_argument("--subject", required=True)
    parser.add_argument("--time-fail-percent", required=True, type=float)
    parser.add_argument("--time-warn-percent", required=True, type=float)
    parser.add_argument("--memory-fail-percent", required=True, type=float)
    parser.add_argument("--memory-warn-percent", required=True, type=float)
    parser.add_argument("--min-time-delta-ms", required=True, type=int)
    parser.add_argument("--min-memory-delta-bytes", required=True, type=int)
    return parser.parse_args()


def median(values: list[int]) -> int:
    return round(statistics.median(values))


def format_value(value: int, unit: str) -> str:
    if unit == "bytes":
        return f"{value / 1024 / 1024:.1f} MiB"
    return f"{value} ms"


def main() -> int:
    args = parse_args()
    measurements: dict[str, dict[str, list[int]]] = {
        metric: {"base": [], "head": []} for metric, _, _ in METRICS
    }
    with args.metrics.open(encoding="utf-8") as handle:
        for line in handle:
            if not line.strip():
                continue
            run, variant, metric, value = line.rstrip("\n").split("\t")
            del run
            measurements[metric][variant].append(int(value))

    for metric, values in measurements.items():
        for variant in ("base", "head"):
            if len(values[variant]) != args.runs:
                raise SystemExit(
                    f"expected {args.runs} {variant} samples for {metric}, "
                    f"got {len(values[variant])}"
                )

    results = []
    failed = False
    warned = False
    for metric, kind, unit in METRICS:
        values = measurements[metric]
        base_median = median(values["base"])
        head_median = median(values["head"])
        delta = head_median - base_median
        delta_percent = (delta * 100.0 / base_median) if base_median else 0.0
        if kind == "time":
            warn_percent = args.time_warn_percent
            fail_percent = args.time_fail_percent
            minimum_delta = args.min_time_delta_ms
        else:
            warn_percent = args.memory_warn_percent
            fail_percent = args.memory_fail_percent
            minimum_delta = args.min_memory_delta_bytes

        warn = delta > 0 and delta_percent > warn_percent and delta >= minimum_delta
        fail = delta > 0 and delta_percent > fail_percent and delta >= minimum_delta
        warned |= warn
        failed |= fail
        status = "FAIL" if fail else "WARN" if warn else "OK"
        print(
            f"{status:4} {metric:20} "
            f"base={format_value(base_median, unit):>12} "
            f"head={format_value(head_median, unit):>12} "
            f"delta={delta_percent:+6.1f}%"
        )
        if warn:
            message = (
                f"{metric} increased by {delta_percent:.1f}% "
                f"({format_value(base_median, unit)} -> {format_value(head_median, unit)})"
            )
            if fail:
                print(f"::error title=Performance regression::{message}")
            else:
                print(f"::warning title=Performance drift::{message}")

        results.append(
            {
                "name": metric,
                "kind": kind,
                "unit": unit,
                "base_samples": values["base"],
                "head_samples": values["head"],
                "base_median": base_median,
                "head_median": head_median,
                "delta_percent": round(delta_percent, 2),
                "warn": warn,
                "fail": fail,
                "warn_percent": warn_percent,
                "fail_percent": fail_percent,
                "minimum_delta": minimum_delta,
            }
        )

    output = {
        "subject": args.subject,
        "runs": args.runs,
        "base_sha": args.base_sha,
        "head_sha": args.head_sha,
        "thresholds": {
            "time_warn_percent": args.time_warn_percent,
            "time_fail_percent": args.time_fail_percent,
            "memory_warn_percent": args.memory_warn_percent,
            "memory_fail_percent": args.memory_fail_percent,
            "min_time_delta_ms": args.min_time_delta_ms,
            "min_memory_delta_bytes": args.min_memory_delta_bytes,
        },
        "metrics": results,
        "status": "failed" if failed else "passed",
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, indent=2) + "\n", encoding="utf-8")

    if warned and not failed:
        print("Performance drift is within the allowed CI tolerance.")
    if failed:
        print("Performance regression exceeded the CI tolerance.")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
