# M31 性能用例

这些普通 Scoop 程序分别覆盖 primitive 循环、aggregate/enum、短命小/中对象、标量与引用数组、长寿旧图的少量写入，以及 String/List/泛型组合。输出用于确认工作负载确实完成，不以耗时阈值作为正确性条件。

`measure.py` 只调用正式 `scoop build` 与所生成的程序；每次报告保存冷/热构建、各次运行时间、产物大小和实际输出。使用独立工作目录，默认重复运行五次。传入配套工具、sysroot 和 runtime 路径可复跑 M30 保存的基线；Linux 通过重复的 `--build-arg` 指定现有 target、C compiler 与 unwind 参数。

```sh
python3 tests/benchmarks/measure.py --tools target/release \
  --sysroot sysroot --runtime runtime --work /tmp/scoop-m31-benchmark \
  --revision "$(git rev-parse HEAD)" --profile debug
```

报告同时记录各次样本、中位数和最小/最大值。GC 统计由 runtime 的诊断输出提供时保留在每次 stderr；没有统计的旧 runtime 不推测扫描量或收集次数。数据与实际参数归档到 `docs/milestone31/`，源码使用同一输入规模比较各阶段。

M31 使用 `--gc-stats --compare-full`：每个 profile 只构建一次，每轮交替运行默认 nursery 和 `SCOOP_GC_FULL_ONLY=1`，记录两组时间、实际 GC 计数及停顿分布。脚本清除继承的 GC stress 开关，检查两种模式输出一致、full-only 没有 minor，以及直方图与实际收集次数一致。分别运行 debug/release 得到四组控制；共同的 ABI、分配布局和必要 lowering 不计入 debug→release 因子。原有六个程序保持原输入，`old-graph-large` 将旧图增大八倍，`survivors` 另外覆盖高存活率、跨代引用和 pin。

```sh
python3 tests/benchmarks/measure.py --tools target/release \
  --sysroot sysroot --runtime runtime --work target/m31-benchmark-debug \
  --revision "$(git rev-parse HEAD)" --profile debug --gc-stats --compare-full
```

Linux 例如追加 `--build-arg=--target --build-arg=x86_64-unknown-linux-gnu`，以及 `--build-arg=--unwind-prefix --build-arg=/absolute/sysroot/native/x86_64-unknown-linux-gnu/unwind`。M30 runtime 不提供这些 GC 统计，复跑旧基线时省略 `--gc-stats --compare-full`。容器和原生宿主分别记录环境，各自比较，不跨环境计算加速比。
