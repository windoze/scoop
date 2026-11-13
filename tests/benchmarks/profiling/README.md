# PGO 调用点对照

`call-sites.scoop` 在同一个循环内从两个位置调用 `mix`。`SCOOP_BENCH_KIND=0` 只执行常用位置；非零值还会每 1,024 次执行另一位置。`SCOOP_BENCH_COUNT` 控制迭代次数，最终状态始终输出，不能把结果未使用的循环当作优化收益。

样例复用 [phases.c](../interfaces/phases.c) 的内核墙钟与进程 CPU 计时。下面是 Darwin/Linux 本机运行方式；先安装对应目标的 profiling 组件，并将 `SCOOP_SYSROOT` 和 `SCOOP_TARGET` 设置为实际工具链根目录与目标。Windows 的原生 CLI 闭环由 [profiling fixture](../../fixtures/compiler-correctness/profiling/runtime/fixture.toml) 验证。

```sh
mkdir -p out/pgo-bench
clang -O2 -std=c11 -c tests/benchmarks/interfaces/phases.c -o out/pgo-bench/phases.o
ar rcs out/pgo-bench/libscoop_scalar_phases.a out/pgo-bench/phases.o

scoop build tests/benchmarks/profiling/call-sites.scoop \
  --target "$SCOOP_TARGET" --release --export-ir full \
  --system-library-root "$SCOOP_SYSROOT/lib/$SCOOP_TARGET/profiling/release" \
  --library-path out/pgo-bench --emit optimizations \
  --dump-dir out/pgo-bench/baseline-dump -o out/pgo-bench/baseline

SCOOP_BENCH_KIND=0 SCOOP_BENCH_COUNT=5000000 \
  scoop pgo instrument tests/benchmarks/profiling/call-sites.scoop \
  --target "$SCOOP_TARGET" --library-path out/pgo-bench \
  --data-dir out/pgo-bench/training

scoop pgo optimize --data-dir out/pgo-bench/training \
  --emit optimizations --dump-dir out/pgo-bench/pgo-dump \
  -o out/pgo-bench/optimized

SCOOP_BENCH_KIND=0 SCOOP_BENCH_COUNT=10000000 out/pgo-bench/baseline
SCOOP_BENCH_KIND=0 SCOOP_BENCH_COUNT=10000000 out/pgo-bench/optimized
SCOOP_BENCH_KIND=1 SCOOP_BENCH_COUNT=7000000 out/pgo-bench/baseline
SCOOP_BENCH_KIND=1 SCOOP_BENCH_COUNT=7000000 out/pgo-bench/optimized
```

先核对同一输入的标准输出，再预热并交错重复 baseline/optimized。运行期间停止其他构建、压缩和基准。标准错误中的 `wall_ns`、`cpu_ns` 只覆盖内核；进程启动、退出和 profiling 文件写出要另计。训练程序的时间用于观察采集开销。

优化报告应能定位 `exercise` 中的两个 `mix` 调用点及原始次数。当前实测为常用位置 `5,000,000 / 1`、未执行位置 `0 / 1`：前者内联，后者保留；普通 release 两者均报告 `not-profitable`。这些决定说明反馈进入了现有成本模型，实际速度仍需测量。[验收记录](../../../docs/profiling/VALIDATION.md)同时保留收益不明显及出现回归的结果。
