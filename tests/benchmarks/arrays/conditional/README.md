# 条件数组内核

该 Cone 与其他数组基准一样，使用 `tests/benchmarks/interfaces/phases.c` 编译得到的目标 `libm34_interfaces.a`，通过 `scoop build --release --library-path` 链接。`SCOOP_BENCH_COUNT` 选择正元素数，`SCOOP_BENCH_KIND` 选择内核。

| kind | 内核 |
| --- | --- |
| 0／11／12 | Long 条件求和：混合／全部不选／全部选中 |
| 1／13／14 | Int 双分支变换：混合／全部 then／全部 else |
| 2 | Int 条件写入 |
| 3／4 | Float／Double 嵌套限幅 |
| 5 | Double 掩码顺序求和 |
| 6 | 按符号写入两个目标数组 |
| 7 | 条件数组分配及初始化 |
| 8 | 保留提前退出的搜索对照 |
| 9 | NoGc 整数递推对照 |
| 10 | 已支持的直线求和对照 |
| 15 | 同数组双分支变换 |

大数组执行 128 轮；少于 256 元素时执行 100000 轮。访问内核的数组准备在 phase 外，kind 7 明确包含分配，kind 9 执行 `count * 128` 次递推。每种模式返回校验值；正常耗时与 GC 统计独立采集。条件内核源文件也由正式优化／移动 GC 专测消费；使用基础目标与普通 release，不启用 fast-math 或本机 CPU 特化。
