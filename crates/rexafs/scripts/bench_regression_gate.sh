#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-informational}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
BASELINE_FILE="$ROOT_DIR/crates/rexafs/benchmarks/baseline.json"
OUT_FILE="$ROOT_DIR/target/bench-regression-summary.md"
THRESHOLD_PCT="$(jq -r '.thresholds.regression_percent' "$BASELINE_FILE")"

calc_pct() {
  local baseline="$1"
  local current="$2"
  awk -v b="$baseline" -v c="$current" 'BEGIN { printf "%.3f", ((c - b) / b) * 100.0 }'
}

format_ms() {
  local ns="$1"
  awk -v v="$ns" 'BEGIN { printf "%.3f", v / 1000000.0 }'
}

bench_names=()
while IFS= read -r name; do
  bench_names+=("$name")
done < <(jq -r '.benchmarks | keys[]' "$BASELINE_FILE")

mkdir -p "$(dirname "$OUT_FILE")"
{
  echo "# Benchmark Regression Report"
  echo
  echo "- Mode: $MODE"
  echo "- Threshold (regression): ${THRESHOLD_PCT}%"
  echo
  echo "| Benchmark | Baseline Median (ms) | Current Median (ms) | Delta (%) | Status |"
  echo "|---|---:|---:|---:|---|"
} > "$OUT_FILE"

REGRESSED=0
MISSING=0

for bench_name in "${bench_names[@]}"; do
  baseline_ns="$(jq -r --arg n "$bench_name" '.benchmarks[$n].median_ns' "$BASELINE_FILE")"
  estimates_file="$ROOT_DIR/target/criterion/${bench_name}/new/estimates.json"

  if [[ ! -f "$estimates_file" ]]; then
    MISSING=1
    {
      echo "| ${bench_name} | $(format_ms "$baseline_ns") | n/a | n/a | missing current estimate |"
    } >> "$OUT_FILE"
    continue
  fi

  current_ns="$(jq -r '.median.point_estimate' "$estimates_file")"
  delta_pct="$(calc_pct "$baseline_ns" "$current_ns")"

  status="ok"
  if awk -v d="$delta_pct" -v t="$THRESHOLD_PCT" 'BEGIN { exit !(d > t) }'; then
    status="regressed"
    REGRESSED=1
  fi

  {
    echo "| ${bench_name} | $(format_ms "$baseline_ns") | $(format_ms "$current_ns") | ${delta_pct}% | ${status} |"
  } >> "$OUT_FILE"
done

if [[ "$MODE" == "blocking" && "$MISSING" -eq 1 ]]; then
  echo "Missing benchmark estimate files in blocking mode."
  cat "$OUT_FILE"
  exit 1
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
