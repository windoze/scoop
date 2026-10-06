# M31 性能用例

这些普通 Scoop 程序分别覆盖 primitive 循环、aggregate/enum、短命小/中对象、标量与引用数组、长寿旧图的少量写入，以及 String/List/泛型组合。输出用于确认工作负载确实完成，不以耗时阈值作为正确性条件。

`measure.py` 只调用正式 `scoop build` 与所生成的程序；每次报告保存冷/热构建、各次运行时间、产物大小和实际输出。使用独立工作目录，默认重复运行五次。传入配套工具、sysroot 和 runtime 路径可复跑 M30 保存的基线；Linux 通过重复的 `--build-arg` 指定现有 target、C compiler 与 unwind 参数。

```sh
python3 tests/benchmarks/measure.py --tools target/release \
  --sysroot sysroot --runtime runtime --work /tmp/scoop-m31-benchmark \
  --revision "$(git rev-parse HEAD)" --profile debug
```

报告同时记录各次样本、中位数和最小/最大值。GC 统计由 runtime 的诊断输出提供时保留在每次 stderr；没有统计的旧 runtime 不推测扫描量或收集次数。数据与实际参数归档到 `docs/milestone31/`，源码使用同一输入规模比较各阶段。
