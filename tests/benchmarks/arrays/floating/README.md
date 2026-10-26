# 严格浮点数组内核

该 Cone 使用已有的 `tests/benchmarks/interfaces/phases.c`，先将其编译成目标平台的 `libm34_interfaces.a`，再通过 `scoop build --release --library-path` 链接。`SCOOP_BENCH_COUNT` 选择正元素数，`SCOOP_BENCH_KIND` 选择内核：

| kind | 内核 |
| --- | --- |
| 0／1 | Float／Double 逐元素乘加，保持两次舍入 |
| 2／3 | Float／Double 按源顺序求和 |
| 4 | Float 位型复制 |
| 5 | Double 缩窄到 Float |
| 6／7 | Float／Double 除法 |
| 8／9 | Float／Double 数组分配及初始化 |
| 10 | 既有 NoGc 管线的严格 Double 递推对照 |

大数组执行 128 轮；少于 256 元素时执行 100000 轮。kind 0～7 的创建和初始化位于 phase 外；kind 8／9 显式包含分配。kind 10 执行 `count * 128` 次递推。运行后输出位型校验值，用相同输入的独立 A/B 进程检查一致性。正常耗时与 GC 统计分开采集，不将采样或 GC stress 的时间用作普通 release 性能。

`src/kernels.scoop` 同时由正式优化和移动 GC 专测消费，覆盖实际测量的源码；专测负责 NaN、负零、subnormal、禁止融合、累加顺序和异常前缀。基准不使用 fast-math 或本机 CPU 特化；有序归约未向量化时保留其实际结果。
