# 真实短命对象分配

本程序测量保留分配后的 runtime 成本。每次分配都发布到 `retained`，下一轮先清空该槽，避免字段替换消除对象，也避免上一轮的发布槽人为延长存活期。原有 safepoint、对象清零、元数据登记、统计和 GC 均参与运行。

| `SCOOP_BENCH_KIND` | 负载 | 建议 `SCOOP_BENCH_COUNT` |
| --- | --- | --- |
| 0 | 24 字节 class | 20000000 |
| 1 | 48 字节 class | 20000000 |
| 2 | 64 个 Long 的数组，含初始化和首尾读取 | 1000000 |
| 3 | 带空 release hook 的 24 字节 class，走共享旧代分配 | 1000000 |
| 4 | 没有循环内分配的整数对照 | 20000000 |

复用 `../../interfaces/phases.c`，以 C11／O2 编译为 `libm34_interfaces.a`，再用 `scoop build --release --library-path DIR` 构建此 Cone。`SCOOP_BENCH_COUNT` 是迭代次数，支持零和短输入。stderr 第一行记录计时区间墙钟和 CPU，stdout 为每轮结果的 XOR；模式 0／3／4 的结果相同，模式 1 每轮为 `4 * i + 6`，模式 2 为 `2 * i + 63`。

使用同一编译器、相同程序和预编译系统 Cone，以 `--runtime-root DIR` 分别链接旧／新 runtime；不要让基线误用候选 sysroot runtime。通过 [compare.py](../../compare.py) 交替执行相同模式、次数和 `SCOOP_GC_WORKERS=1`，七对计时、预热一对；`--gc-stats` 单独运行并核对分配、收集和晋升数据。计时不包含构建，启动和输出成本由进程测量另行记录。
