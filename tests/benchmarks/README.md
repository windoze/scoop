# M31 性能用例

M33 的真实 C FFI 调用与 collector 等待基准见 [ffi/README.md](ffi/README.md)。

M34 的专项用例包括 [接口调用与转换](interfaces/README.md)、[纯计算循环的 GC 请求响应](poll-response/README.md)、[小值 ABI](small-values/README.md) 与 [容器存储](containers/README.md)。具体样本及取舍见 [M34 性能记录](../../docs/milestone34/PERFORMANCE.md)。

这些普通 Scoop 程序分别覆盖 primitive 循环、aggregate/enum、短命小/中对象、标量与引用数组、长寿旧图的少量写入，以及 String/List/泛型组合。输出用于确认工作负载确实完成，不以耗时阈值作为正确性条件。

`measure.py` 只调用正式 `scoop build` 与所生成的程序；每次报告保存冷/热构建、各次运行时间、产物大小和实际输出。使用独立工作目录，默认重复运行五次。传入配套工具、sysroot 和 runtime 路径可复跑 M30 保存的基线；Linux 通过重复的 `--build-arg` 指定现有 target、C compiler 与 unwind 参数。

```sh
mkdir -p tmp
export TMPDIR="$PWD/tmp"
python3 tests/benchmarks/measure.py --tools target/release \
  --sysroot sysroot --runtime runtime --work tmp/m31-benchmark \
  --revision "$(git rev-parse HEAD)" --profile debug
```

报告同时记录各次样本、中位数和最小/最大值。GC 统计由 runtime 的诊断输出提供时保留在每次 stderr；没有统计的旧 runtime 不推测扫描量或收集次数。数据与实际参数归档到 `docs/milestone31/`，源码使用同一输入规模比较各阶段。

M31 使用 `--gc-stats --compare-full`：每个 profile 只构建一次，每轮交替运行默认 nursery 和 `SCOOP_GC_FULL_ONLY=1`，记录两组时间、实际 GC 计数及停顿分布。脚本清除继承的 GC stress 开关，检查两种模式输出一致、full-only 没有 minor，以及直方图与实际收集次数一致。分别运行 debug/release 得到四组控制；共同的 ABI、分配布局和必要 lowering 不计入 debug→release 因子。原有六个程序保持原输入，`old-graph-large` 将旧图增大八倍，`survivors` 另外覆盖高存活率、跨代引用和 pin。

```sh
python3 tests/benchmarks/measure.py --tools target/release \
  --sysroot sysroot --runtime runtime --work tmp/m31-benchmark-debug \
  --revision "$(git rev-parse HEAD)" --profile debug --gc-stats --compare-full
```

Linux 例如追加 `--build-arg=--target --build-arg=x86_64-unknown-linux-gnu`，以及 `--build-arg=--unwind-prefix --build-arg=/absolute/sysroot/native/x86_64-unknown-linux-gnu/unwind`。M30 runtime 不提供这些 GC 统计，复跑旧基线时省略 `--gc-stats --compare-full`。容器和原生宿主分别记录环境，各自比较，不跨环境计算加速比。

当前 Linux 复现使用 `ssh nuc12`，仓库为 `~/repos/scoop`；工作目录和 `TMPDIR` 都放在该仓库的 `tmp/` 下。先完成工具构建与功能验证，再串行计时，避免并行编译或清理目录干扰测量。

M34 的 `mir-inlining.scoop` 用两千万次 xorshift 组合比较同一 release 配置下的 MIR 内联开关。kernel 与 helper 为 NoGC，使现有完整 poll 不掩盖调用和常量分支的差异；最终 checksum 为 `4062365736`。用 `--case mir-inlining --runs 7 --gc-stats` 单独运行，关闭内联的工具保存自 M34-3a。两套工具分别使用独立 cache，结果及热点机器码见 [M34 性能记录](../../docs/milestone34/PERFORMANCE.md)。

M34 的 `region-stores.scoop` 在保活对象之间执行八百万次单槽写入，单独记录四级 page map 内联屏障的成本；输出为 `9`。配合 `allocation`、`old-graph-large` 与 `survivors` 测量分配、稀疏旧区写入和存活图的组合影响。对照使用相同 release 编译优化与源码，每组七次、逐轮交替旧／新 heap，保留 GC 计数与机器码；不把引入可扩展堆所需的屏障成本隐藏在整体平均值中。

M34-7 的 `regions/region-churn.scoop` 连续两轮分配 180 万个 Node，分别在保留整个图、保留 64 个稀疏对象、全部离开作用域后执行 full 并采样；输出为 `113400000`。先将同目录的 `region-memory.c` 编译为 `libm34_memory.a`，再用普通 `scoop build --library-path` 链接。C companion 只读取两版 runtime 共有的映射计数，并用相同 OS API 记录进程当前／峰值 RSS；六条 phase JSON 写入 stderr，最终 GC 统计仍由 `SCOOP_GC_STATS=1` 输出。设置 `SCOOP_BENCH_PIN=1` 会在稀疏阶段 pin 首尾两个对象，再在退出本轮前 unpin，用相同程序对照 pin 对集中回收的影响。phase 0／3 为完整图，1／4 为稀疏图，2／5 为作用域退出后的状态；实际 RSS、成功 discard 和 unmap 是不同的测量值。
