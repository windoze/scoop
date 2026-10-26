# 宽元素引用写入基准

用相同源码、运行库和 release 设置比较两版编译器。先将 `../../interfaces/phases.c` 编译为 `libm34_interfaces.a`，构建本 Cone 时通过 `--library-path` 提供。计时辅助代码只在显式链接该基准时进入产物。

`SCOOP_BENCH_COUNT` 是正的数组长度，建议 65536；每个内核写入 128 轮。输入和目标在计时前准备并完成一次 full GC；阶段内不主动分配或收集，结束后读取全部元素的 key 与标量字段生成校验和。阶段时间在 stderr，另用普通进程计时报告整体成本；GC 诊断单独运行。

| `SCOOP_BENCH_KIND` | 操作 |
| --- | --- |
| 0 / 4 | 24 B class／32 B interface 元素反复写入同一引用，更新标量字段 |
| 1 / 5 | 两个非空引用交替写入，每轮更换 |
| 2 / 6 | 两套固定输入交替，约一半槽更换引用 |
| 3 / 7 | class／interface 可空值反复清空，同时更新标量字段 |
| 8 | 两个引用槽的 aggregate 对照，保留范围屏障 |
| 9 | GC-free 的 24 B aggregate 对照 |

模式 0～3 使用可空 class 引用，4～7 使用可空接口；其 null、object 和 itab 表示均来自实际泛型布局。比较最终 `write` 实例及调用处机器码，不能只根据源级循环或单一内核推断所有工作负载获益。相关 GC 行为在 `gc-correctness/poll/reference-arrays` 验证，不用计时阈值判断正确性。
