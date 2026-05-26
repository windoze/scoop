#!/usr/bin/env bash
set -u
set -o pipefail

ROOT_DIR="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
FIXTURE_DIR="$ROOT_DIR/tests/fixtures/run-pass"
OUT_DIR="${1:-$ROOT_DIR/target/run-pass-gc-scan}"
TIMEOUT_SECS="${TIMEOUT_SECS:-60}"
PYTHON_BIN="${PYTHON:-python3}"
RUN_FIXTURES="$ROOT_DIR/tools/run_fixtures.py"

mkdir -p "$OUT_DIR/logs"

PASS_FILE="$OUT_DIR/pass.txt"
FAIL_FILE="$OUT_DIR/fail.txt"
TIMEOUT_FILE="$OUT_DIR/timeout.txt"
SUMMARY_FILE="$OUT_DIR/summary.txt"

: > "$PASS_FILE"
: > "$FAIL_FILE"
: > "$TIMEOUT_FILE"
: > "$SUMMARY_FILE"

printf 'Building scoop and scoopc binaries once...\n'
if ! cargo build -p scoop -p scoopc >"$OUT_DIR/build.log" 2>&1; then
  printf 'Build failed. See %s\n' "$OUT_DIR/build.log"
  exit 1
fi

SCOOP_BIN="$ROOT_DIR/target/debug/scoop"
SCOOPC_BIN="$ROOT_DIR/target/debug/scoopc"
if [[ ! -x "$SCOOP_BIN" ]]; then
  printf 'Missing scoop binary: %s\n' "$SCOOP_BIN"
  exit 1
fi
if [[ ! -x "$SCOOPC_BIN" ]]; then
  printf 'Missing scoopc binary: %s\n' "$SCOOPC_BIN"
  exit 1
fi

fixtures=("$FIXTURE_DIR"/*.scoop)
IFS=$'\n' fixtures=($(printf '%s\n' "${fixtures[@]}" | sort))
unset IFS

total=${#fixtures[@]}
pass_count=0
fail_count=0
timeout_count=0
index=0

for fixture in "${fixtures[@]}"; do
  index=$((index + 1))
  base="$(basename "$fixture")"
  stem="${base%.scoop}"
  log_file="$OUT_DIR/logs/$stem.log"

  printf '[%d/%d] %s\n' "$index" "$total" "$base"

  if timeout --signal=TERM --kill-after=5s "${TIMEOUT_SECS}s" \
    env \
      CARGO_TERM_COLOR=never \
      SCOOP_BIN="$SCOOP_BIN" \
      SCOOPC_BIN="$SCOOPC_BIN" \
      SCOOP_GC_VERIFY_ROOTS=1 \
      "$PYTHON_BIN" "$RUN_FIXTURES" -j 1 --exit-on-failure --gc-move --gc-stress "$fixture" >"$log_file" 2>&1; then
    printf '%s\n' "$fixture" >> "$PASS_FILE"
    pass_count=$((pass_count + 1))
    continue
  else
    status=$?
  fi

  if [[ "${status:-0}" -eq 124 || "${status:-0}" -eq 137 ]]; then
    printf '%s\n' "$fixture" >> "$TIMEOUT_FILE"
    timeout_count=$((timeout_count + 1))
    continue
  fi

  printf '%s\n' "$fixture" >> "$FAIL_FILE"
  fail_count=$((fail_count + 1))
done

{
  printf 'total=%d\n' "$total"
  printf 'pass=%d\n' "$pass_count"
  printf 'fail=%d\n' "$fail_count"
  printf 'timeout=%d\n' "$timeout_count"
  printf '\n[failed]\n'
  cat "$FAIL_FILE"
  printf '\n[timeout]\n'
  cat "$TIMEOUT_FILE"
} > "$SUMMARY_FILE"

cat "$SUMMARY_FILE"

if [[ "$fail_count" -ne 0 || "$timeout_count" -ne 0 ]]; then
  exit 1
fi
