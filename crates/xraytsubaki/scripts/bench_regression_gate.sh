#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-informational}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
BASELINE_FILE="$ROOT_DIR/crates/xraytsubaki/benchmarks/baseline.json"
OUT_FILE="$ROOT_DIR/target/bench-regression-summary.md"
THRESHOLD_PCT="$(jq -r '.thresholds.regression_percent' "$BASELINE_FILE")"

read_metric() {
  local bench_name="$1"
  local file="$ROOT_DIR/target/criterion/${bench_name}/new/estimates.json"
  jq -r '.median.point_estimate' "$file"
}

calc_pct() {
  local baseline="$1"
  local current="$2"
  awk -v b="$baseline" -v c="$current" 'BEGIN { printf "%.3f", ((c - b) / b) * 100.0 }'
}

format_ms() {
  local ns="$1"
  awk -v v="$ns" 'BEGIN { printf "%.3f", v / 1000000.0 }'
}

SINGLE_BASELINE="$(jq -r '.benchmarks.xas_group_benchmark_single.median_ns' "$BASELINE_FILE")"
PAR_BASELINE="$(jq -r '.benchmarks.xas_group_benchmark_parallel.median_ns' "$BASELINE_FILE")"
SINGLE_CURRENT="$(read_metric "xas_group_benchmark_single")"
PAR_CURRENT="$(read_metric "xas_group_benchmark_parallel")"

SINGLE_DELTA_PCT="$(calc_pct "$SINGLE_BASELINE" "$SINGLE_CURRENT")"
PAR_DELTA_PCT="$(calc_pct "$PAR_BASELINE" "$PAR_CURRENT")"

mkdir -p "$(dirname "$OUT_FILE")"
cat > "$OUT_FILE" <<MARKDOWN
# Benchmark Regression Report

- Mode: $MODE
- Threshold (regression): ${THRESHOLD_PCT}%

| Benchmark | Baseline Median (ms) | Current Median (ms) | Delta (%) |
|---|---:|---:|---:|
| xas_group_benchmark_single | $(format_ms "$SINGLE_BASELINE") | $(format_ms "$SINGLE_CURRENT") | ${SINGLE_DELTA_PCT}% |
| xas_group_benchmark_parallel | $(format_ms "$PAR_BASELINE") | $(format_ms "$PAR_CURRENT") | ${PAR_DELTA_PCT}% |
MARKDOWN

REGRESSED=0
if awk -v d="$SINGLE_DELTA_PCT" -v t="$THRESHOLD_PCT" 'BEGIN { exit !(d > t) }'; then
  REGRESSED=1
fi
if awk -v d="$PAR_DELTA_PCT" -v t="$THRESHOLD_PCT" 'BEGIN { exit !(d > t) }'; then
  REGRESSED=1
fi

if [[ "$MODE" == "blocking" && "$REGRESSED" -eq 1 ]]; then
  echo "Performance regression exceeded threshold (${THRESHOLD_PCT}%)."
  cat "$OUT_FILE"
  exit 1
fi

if [[ "$REGRESSED" -eq 1 ]]; then
  echo "Regression detected (informational mode)."
else
  echo "No threshold regressions detected."
fi
cat "$OUT_FILE"
