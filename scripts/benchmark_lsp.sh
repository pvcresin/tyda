#!/usr/bin/env bash
set -euo pipefail

RUNS="${1:-3}"
SUBJECT_ARG="${2:-subject/gitlab/app}"
PROFILE="${3:-release}"

if ! [[ "$RUNS" =~ ^[0-9]+$ ]] || [[ "$RUNS" -lt 1 ]]; then
  echo "Usage: scripts/benchmark_lsp.sh [runs] [subject_path] [release|debug]" >&2
  exit 1
fi

if [[ "$PROFILE" != "release" && "$PROFILE" != "debug" ]]; then
  echo "profile must be 'release' or 'debug'" >&2
  exit 1
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ "$SUBJECT_ARG" = /* ]]; then
  SUBJECT_PATH="$SUBJECT_ARG"
  SUBJECT_LABEL="$SUBJECT_ARG"
else
  SUBJECT_PATH="$REPO_ROOT/$SUBJECT_ARG"
  SUBJECT_LABEL="$SUBJECT_ARG"
fi

if [[ ! -e "$SUBJECT_PATH" ]]; then
  echo "lsp benchmark subject not found: $SUBJECT_PATH" >&2
  exit 1
fi

if [[ ! -x /usr/bin/time ]]; then
  echo "expected /usr/bin/time for RSS measurement" >&2
  exit 1
fi

TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT
METRICS_TSV="$TMPDIR/metrics.tsv"

CARGO_TEST_ARGS=(cargo test)
if [[ "$PROFILE" = "release" ]]; then
  CARGO_TEST_ARGS+=(--release)
fi

echo "=== LSP Benchmark Target ==="
echo "subject: $SUBJECT_LABEL"
echo "runs: $RUNS"
echo "profile: $PROFILE"
echo ""

echo "=== Warmup ==="
TYDA_LSP_BENCH_ROOT="$SUBJECT_PATH" \
  "${CARGO_TEST_ARGS[@]}" lsp::tests::bench_display_analysis_mastodon_scale -- --nocapture >/dev/null 2>&1

echo "=== Timed Runs ==="
for run in $(seq 1 "$RUNS"); do
  LOG_PATH="$TMPDIR/run-$run.log"
  set +e
  TYDA_LSP_BENCH_ROOT="$SUBJECT_PATH" \
    /usr/bin/time -l "${CARGO_TEST_ARGS[@]}" lsp::tests::bench_display_analysis_mastodon_scale -- --nocapture \
    >"$LOG_PATH" 2>&1
  status=$?
  set -e
  if [[ "$status" -ne 0 ]] && ! grep -q "test result: ok" "$LOG_PATH"; then
    cat "$LOG_PATH" >&2
    exit "$status"
  fi

  PARSED="$(
    python3 - "$LOG_PATH" <<'PY'
import re
import sys

text = open(sys.argv[1], encoding="utf-8").read()
patterns = {
    "scan_ms": r"\[bench\] workspace scan: (\d+)ms",
    "first_ms": r"\[bench\] first display analysis: (\d+)ms",
    "cached_ms": r"\[bench\] cached display analysis: (\d+)ms",
    "unrelated_ms": r"\[bench\] unrelated dirty display analysis: (\d+)ms",
    "dirty_ms": r"\[bench\] dirty display analysis: (\d+)ms",
    "max_rss_bytes": r"^\s*(\d+)\s+maximum resident set size$",
}
values = {}
for key, pattern in patterns.items():
    match = re.search(pattern, text, re.MULTILINE)
    if not match:
        if key == "max_rss_bytes":
            values[key] = -1
            continue
        raise SystemExit(f"failed to parse {key} from {sys.argv[1]}")
    values[key] = int(match.group(1))
print(
    "\t".join(
        str(values[key])
        for key in [
            "scan_ms",
            "first_ms",
            "cached_ms",
            "unrelated_ms",
            "dirty_ms",
            "max_rss_bytes",
        ]
    )
)
PY
  )"

  IFS=$'\t' read -r scan_ms first_ms cached_ms unrelated_ms dirty_ms max_rss_bytes <<<"$PARSED"
  rss_label="$max_rss_bytes"
  if [[ "$max_rss_bytes" -lt 0 ]]; then
    rss_label="unavailable"
  fi
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$run" "$scan_ms" "$first_ms" "$cached_ms" "$unrelated_ms" "$dirty_ms" "$max_rss_bytes" \
    >>"$METRICS_TSV"
  echo \
    "run=$run scan_ms=$scan_ms first_display_ms=$first_ms cached_display_ms=$cached_ms unrelated_dirty_display_ms=$unrelated_ms dirty_display_ms=$dirty_ms max_rss_bytes=$rss_label"
done

echo ""
python3 - "$METRICS_TSV" "$SUBJECT_LABEL" "$PROFILE" <<'PY'
import datetime
import sys

metrics_path, subject, profile = sys.argv[1:]
rows = []
with open(metrics_path, encoding="utf-8") as handle:
    for line in handle:
        run, scan, first, cached, unrelated, dirty, rss = line.strip().split("\t")
        rows.append(
            {
                "run": int(run),
                "scan_ms": int(scan),
                "first_ms": int(first),
                "cached_ms": int(cached),
                "unrelated_ms": int(unrelated),
                "dirty_ms": int(dirty),
                "max_rss_bytes": int(rss),
            }
        )


def summarize(key: str) -> tuple[int, int, int]:
    values = [row[key] for row in rows]
    avg = round(sum(values) / len(values))
    return avg, min(values), max(values)


def summarize_optional(key: str) -> tuple[str, str, str]:
    values = [row[key] for row in rows if row[key] >= 0]
    if not values:
        return "-", "-", "-"
    avg = round(sum(values) / len(values))
    return str(avg), str(min(values)), str(max(values))


scan_avg, scan_min, scan_max = summarize("scan_ms")
first_avg, first_min, first_max = summarize("first_ms")
cached_avg, cached_min, cached_max = summarize("cached_ms")
unrelated_avg, unrelated_min, unrelated_max = summarize("unrelated_ms")
dirty_avg, dirty_min, dirty_max = summarize("dirty_ms")
rss_avg, rss_min, rss_max = summarize_optional("max_rss_bytes")

print("=== Summary ===")
print(f"runs={len(rows)}")
print(f"subject={subject}")
print(f"profile={profile}")
print(f"scan_ms_avg={scan_avg}")
print(f"scan_ms_min={scan_min}")
print(f"scan_ms_max={scan_max}")
print(f"first_display_ms_avg={first_avg}")
print(f"first_display_ms_min={first_min}")
print(f"first_display_ms_max={first_max}")
print(f"cached_display_ms_avg={cached_avg}")
print(f"cached_display_ms_min={cached_min}")
print(f"cached_display_ms_max={cached_max}")
print(f"unrelated_dirty_display_ms_avg={unrelated_avg}")
print(f"unrelated_dirty_display_ms_min={unrelated_min}")
print(f"unrelated_dirty_display_ms_max={unrelated_max}")
print(f"dirty_display_ms_avg={dirty_avg}")
print(f"dirty_display_ms_min={dirty_min}")
print(f"dirty_display_ms_max={dirty_max}")
print(f"max_rss_bytes_avg={rss_avg}")
print(f"max_rss_bytes_min={rss_min}")
print(f"max_rss_bytes_max={rss_max}")
print("")
print("=== Markdown Row ===")
today = datetime.date.today().isoformat()
print(
    f"| {today} | {profile} | {subject} | {len(rows)} | {scan_avg} | {first_avg} | {cached_avg} | {unrelated_avg} | {dirty_avg} | {rss_avg} | {rss_max} |"
)
PY
