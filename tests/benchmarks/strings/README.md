# f-string 脱糖对照

`interpolation.scoop` 用同一份源码分别交给修改前后的编译器，测量短插值展开的实际效果。复用 `../interfaces/phases.c` 的计时、模式与轮数入口，结果字节数进入输出校验和。

| `SCOOP_BENCH_KIND` | 负载 |
| --- | --- |
| 0 | 一个已有 String |
| 1 | 两个已有 String |
| 2 | 五段：两个 String 与文本 |
| 3 | 八个 String 片段 |
| 4 | 九个 String 片段，builder 后备 |
| 5 | 五段：变化的 Long、Int 与文本 |
| 6 | 五段：Char 与文本 |
| 7 | 五段：用户 ToString、变化的 Int 与文本 |
| 8 | 超过 64 bytes 的文本、String 和尾部文本 |
| 9 | 普通 String `+` 链对照 |

在仓库忽略的工作目录中将 phase helper 编译为 `libscoop_scalar_phases.a`；构建时显式传 `--library-path`、`--sysroot`、配套 `--scoopc` 及各自的 cache/target/output 路径。Linux 另外指定目标和 C 编译器。两个编译器消费同一份已准备的 sysroot。

运行前预热各模式一次，随后七对交替 AB/BA，每次设置 `SCOOP_BENCH_COUNT=1000000`。计时报告中位 ns/次；逐对核对 stdout 校验和。Linux 对照固定同一个 CPU，并与构建成本测量错开。GC 统计单独用十万轮、`SCOOP_GC_STATS=1` 收集，不混入主计时。单片段返回已有 String 后，循环可能被进一步简化，因此该项时间不能解释为每次都实际执行一次调用。

完整文件大小与代码段大小分别报告；冷 consumer 构建另以三个全新 cache/target 目录测量，保留源码、系统库和工具。原始 JSON 与日志放在仓库外，结论记录到 `docs/program-performance/PROGRESS.md`。
