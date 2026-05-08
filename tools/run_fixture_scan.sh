#!/usr/bin/env bash
set -u
set -o pipefail

usage() {
  cat <<'EOF'
Usage: tools/run_fixture_scan.sh [options] [fixtures-root]

Run Scoop fixtures one by one with a per-fixture timeout.

Options:
  --timeout-secs N   Per-fixture timeout in seconds (default: 30)
  --scoop-bin PATH   Scoop binary to run (default: target/debug/scoop)
  --out-dir DIR      Output directory (default: target/fixture-scan)
  --env KEY=VALUE    Extra environment variable for each fixture run
  --no-build         Skip `cargo build -p scoop`
  -h, --help         Show this help

Examples:
  tools/run_fixture_scan.sh
  tools/run_fixture_scan.sh --env SCOOP_GC_MOVE=1 --env SCOOP_GC_STRESS=1
  tools/run_fixture_scan.sh tests/fixtures/typecheck
EOF
}

ROOT_DIR="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
TARGET_INPUT="tests/fixtures"
SCOOP_BIN_INPUT="target/debug/scoop"
OUT_DIR_INPUT="target/fixture-scan"
TIMEOUT_SECS="30"
NO_BUILD="0"
ENV_OVERRIDES=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --timeout-secs)
      TIMEOUT_SECS="${2:-}"
      shift 2
      ;;
    --scoop-bin)
      SCOOP_BIN_INPUT="${2:-}"
      shift 2
      ;;
    --out-dir)
      OUT_DIR_INPUT="${2:-}"
      shift 2
      ;;
    --env)
      if [[ -z "${2:-}" || "$2" != *=* ]]; then
        printf 'Invalid --env value: %s\n' "${2:-<missing>}" >&2
        exit 2
      fi
      ENV_OVERRIDES+=("$2")
      shift 2
      ;;
    --no-build)
      NO_BUILD="1"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    --)
      shift
      break
      ;;
    -*)
      printf 'Unknown option: %s\n\n' "$1" >&2
      usage >&2
      exit 2
      ;;
    *)
      if [[ "$TARGET_INPUT" != "tests/fixtures" ]]; then
        printf 'Only one fixtures-root may be provided.\n\n' >&2
        usage >&2
        exit 2
      fi
      TARGET_INPUT="$1"
      shift
      ;;
  esac
done

if [[ $# -gt 0 ]]; then
  printf 'Unexpected extra arguments.\n\n' >&2
  usage >&2
  exit 2
fi

if [[ ! "$TIMEOUT_SECS" =~ ^[1-9][0-9]*$ ]]; then
  printf 'Invalid --timeout-secs: %s\n' "$TIMEOUT_SECS" >&2
  exit 2
fi

resolve_path() {
  case "$1" in
    /*) printf '%s\n' "$1" ;;
    *) printf '%s\n' "$ROOT_DIR/$1" ;;
  esac
}

display_path() {
  case "$1" in
    "$ROOT_DIR"/*) printf '%s\n' "${1#"$ROOT_DIR"/}" ;;
    "$ROOT_DIR") printf '.\n' ;;
    *) printf '%s\n' "$1" ;;
  esac
}

has_parent_dir_name() {
  local path="$1"
  local want="$2"
  local parent

  parent="$(basename "$(dirname "$path")")"
  [[ "$parent" == "$want" ]]
}

is_run_pass_cone_case_root() {
  local path="$1"

  [[ -d "$path" ]] \
    && has_parent_dir_name "$path" "run_pass_cone" \
    && [[ -f "$path/Cone.toml" ]] \
    && [[ -f "$path/src/main.scoop" ]]
}

list_case_dirs() {
  local root="$1"
  if [[ -d "$root" ]]; then
    find "$root" -mindepth 1 -maxdepth 1 -type d -print | LC_ALL=C sort
  fi
}

append_units_for_target() {
  local target="$1"
  local base

  if [[ -f "$target" ]]; then
    printf '%s\n' "$target" >> "$UNSORTED_LIST"
    return
  fi

  if [[ ! -d "$target" ]]; then
    printf 'Fixtures path does not exist: %s\n' "$target" >&2
    exit 1
  fi

  if has_parent_dir_name "$target" "resolve_multi" \
    || has_parent_dir_name "$target" "resolve_cone" \
    || has_parent_dir_name "$target" "typecheck_multi" \
    || has_parent_dir_name "$target" "typecheck_cone" \
    || has_parent_dir_name "$target" "typecheck_cone_archive" \
    || is_run_pass_cone_case_root "$target"; then
    printf '%s\n' "$target" >> "$UNSORTED_LIST"
    return
  fi

  base="$(basename "$target")"
  case "$base" in
    resolve_multi|resolve_cone|typecheck_multi|typecheck_cone|typecheck_cone_archive|run_pass_cone)
      list_case_dirs "$target" >> "$UNSORTED_LIST"
      return
      ;;
  esac

  find "$target" \
    \( \
      -path "$target/resolve_multi" \
      -o -path "$target/resolve_cone" \
      -o -path "$target/typecheck_multi" \
      -o -path "$target/typecheck_cone" \
      -o -path "$target/typecheck_cone_archive" \
      -o -path "$target/run_pass_cone" \
    \) -prune -o \
    -type f -name '*.scoop' -print >> "$UNSORTED_LIST"

  list_case_dirs "$target/resolve_multi" >> "$UNSORTED_LIST"
  list_case_dirs "$target/resolve_cone" >> "$UNSORTED_LIST"
  list_case_dirs "$target/typecheck_multi" >> "$UNSORTED_LIST"
  list_case_dirs "$target/typecheck_cone" >> "$UNSORTED_LIST"
  list_case_dirs "$target/typecheck_cone_archive" >> "$UNSORTED_LIST"
  list_case_dirs "$target/run_pass_cone" >> "$UNSORTED_LIST"
}

TARGET_PATH="$(resolve_path "$TARGET_INPUT")"
SCOOP_BIN="$(resolve_path "$SCOOP_BIN_INPUT")"
OUT_DIR="$(resolve_path "$OUT_DIR_INPUT")"

mkdir -p "$OUT_DIR/logs"

BUILD_LOG="$OUT_DIR/build.log"
ALL_FILE="$OUT_DIR/all.txt"
PASS_FILE="$OUT_DIR/pass.txt"
FAIL_FILE="$OUT_DIR/fail.txt"
TIMEOUT_FILE="$OUT_DIR/timeout.txt"
SUMMARY_FILE="$OUT_DIR/summary.txt"
UNSORTED_LIST="$(mktemp "${TMPDIR:-/tmp}/scoop-fixture-scan.unsorted.XXXXXX")"

trap 'rm -f "$UNSORTED_LIST"' EXIT

: > "$UNSORTED_LIST"
: > "$PASS_FILE"
: > "$FAIL_FILE"
: > "$TIMEOUT_FILE"

append_units_for_target "$TARGET_PATH"
LC_ALL=C sort -u "$UNSORTED_LIST" > "$ALL_FILE"

total="$(wc -l < "$ALL_FILE" | tr -d ' ')"
if [[ "$total" == "0" ]]; then
  printf 'No fixtures found under %s\n' "$(display_path "$TARGET_PATH")" >&2
  exit 1
fi

if [[ "$NO_BUILD" != "1" ]]; then
  printf 'Building scoop binary once...\n'
  if ! (cd "$ROOT_DIR" && cargo build -p scoop > "$BUILD_LOG" 2>&1); then
    printf 'Build failed. See %s\n' "$(display_path "$BUILD_LOG")" >&2
    exit 1
  fi
fi

if [[ ! -x "$SCOOP_BIN" ]]; then
  printf 'Missing scoop binary: %s\n' "$(display_path "$SCOOP_BIN")" >&2
  exit 1
fi

command_shape="$(display_path "$SCOOP_BIN") test --fixtures <fixture>"
if [[ ${#ENV_OVERRIDES[@]} -gt 0 ]]; then
  command_shape="env ${ENV_OVERRIDES[*]} ${command_shape}"
fi

pass_count=0
fail_count=0
timeout_count=0
index=0

while IFS= read -r fixture; do
  index=$((index + 1))
  rel_fixture="$(display_path "$fixture")"
  log_stem="${rel_fixture//\//__}"
  log_stem="${log_stem// /_}"
  log_file="$OUT_DIR/logs/${log_stem}.log"

  printf '[%d/%d] RUN %s\n' "$index" "$total" "$rel_fixture"

  if timeout --signal=TERM --kill-after=5s "${TIMEOUT_SECS}s" \
    env CARGO_TERM_COLOR=never "${ENV_OVERRIDES[@]}" \
    "$SCOOP_BIN" test --fixtures "$fixture" > "$log_file" 2>&1; then
    printf '[%d/%d] PASS %s\n' "$index" "$total" "$rel_fixture"
    printf '%s\n' "$rel_fixture" >> "$PASS_FILE"
    pass_count=$((pass_count + 1))
    continue
  fi

  status=$?
  if [[ "$status" -eq 124 || "$status" -eq 137 ]]; then
    printf '[%d/%d] TIMEOUT %s\n' "$index" "$total" "$rel_fixture"
    printf '%s\n' "$rel_fixture" >> "$TIMEOUT_FILE"
    timeout_count=$((timeout_count + 1))
    continue
  fi

  printf '[%d/%d] FAIL %s\n' "$index" "$total" "$rel_fixture"
  printf '%s\n' "$rel_fixture" >> "$FAIL_FILE"
  fail_count=$((fail_count + 1))
done < "$ALL_FILE"

{
  printf 'target=%s\n' "$(display_path "$TARGET_PATH")"
  printf 'command=%s\n' "$command_shape"
  printf 'total=%d\n' "$total"
  printf 'pass=%d\n' "$pass_count"
  printf 'fail=%d\n' "$fail_count"
  printf 'timeout=%d\n' "$timeout_count"
  printf 'per_fixture_timeout=%ss\n' "$TIMEOUT_SECS"
  printf '\n[failed]\n'
  cat "$FAIL_FILE"
  printf '\n[timeout]\n'
  cat "$TIMEOUT_FILE"
} > "$SUMMARY_FILE"

cat "$SUMMARY_FILE"
