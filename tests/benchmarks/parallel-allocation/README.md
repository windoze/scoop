# 分配统计对照

单 mutator 使用相邻的 `../allocation.scoop`：每轮分配一个 Small 和一个 Medium，共 80,000 个对象。
四 mutator 使用本目录的 `program.scoop`：每线程分配 200,000 个 Item，共 800,000 个对象。
两者分别与相同源码的旧 runtime 比较，不以不同工作负载的时间相除。

先用宿主 C 编译器将 `tests/fixtures/m13-callback/native.c` 编译成 object，再用 `ar rcs`
归档为工作目录中的 `libm34_allocation_bench.a`。它只负责启动、join 四个 pthread；实际循环、
allocation、TLAB 和 safepoint 都来自 Scoop 生成代码。构建 parallel 程序时传入
`--library-path <工作目录>`，两种程序使用同一工具、release profile、runtime、sysroot 与 cache。
Linux 另传实际 target 和 unwind prefix。

```sh
python3 tests/benchmarks/parallel-allocation/run.py \
  --single <单线程可执行文件> --parallel <四线程可执行文件> \
  --revision <实际版本> --output <报告.json> --runs 7
```

脚本复用已有 `tests/benchmarks/measure.py` 的计时、输出与 GC histogram 检查，分别运行关闭／开启
GC 诊断的组合，保留全部样本。其他构建、测试与清理须在该宿主的测量结束后执行。
