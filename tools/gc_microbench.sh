#!/usr/bin/env bash
set -euo pipefail

# GC microbench runner（TODO T1406d）。
#
# 目标：
# - 一键对比 baseline vs Immix 的吞吐与碎片化指标；
# - 不做阈值 gating：结果依赖机器/编译器版本，仅用于本地对比与回归观测。
#
# 用法：
#   tools/gc_microbench.sh <throughput|fragmentation> [gc_microbench args...]
#
# 示例：
#   tools/gc_microbench.sh throughput --object-size 256 --rounds 50 --batch 50000
#   tools/gc_microbench.sh fragmentation --object-size 256 --initial 200000 --pin-stride 100

scenario="${1:-}"
if [[ -z "${scenario}" || "${scenario}" == "-h" || "${scenario}" == "--help" ]]; then
  echo "Usage: tools/gc_microbench.sh <throughput|fragmentation> [gc_microbench args...]"
  exit 2
fi
shift || true

run_one() {
  local backend_name="$1"
  local feature="$2"
  shift 2

  echo "== ${backend_name} (${feature}) =="
  cargo run -p scoop_runtime \
    --release \
    --bin gc_microbench \
    --no-default-features \
    --features "${feature}" \
    -- "${scenario}" "$@"
  echo
}

run_one "baseline" "gc-baseline" "$@"
run_one "immix" "gc-immix" "$@"

