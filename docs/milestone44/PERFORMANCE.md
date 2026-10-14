# M44 性能记录

本文件只记录实际完成的测量；其余设计场景和三目标验证继续见 [PROGRESS](PROGRESS.md)。

## 当前图基线

源码基线 `44a3c0bf`，编译器为 M43 完成后的固定 release 工具（摘要在报告中），
runtime 仅增加线程测试观测点，尚未优化 relocation。主机 Apple M4 Pro、64 GiB，
macOS 27.0.1／AArch64，LLVM 22.1.8、Apple clang 21.0.0；生成程序为 release。
固定 4 个 mark worker，普通 GC 策略与默认 block／nursery 配置，关闭 moving／minor
stress 和 full-only。两次对照必须使用同一输入和观测配置。

使用现有 wide／array／chain 程序，每图 131071 个节点、每进程主动 full 五次，
预热一次后测七次进程，共每图 35 次有效 collection。计时期间无本任务的构建、
测试或清理；[原始报告](validation/graphs-baseline-darwin.json.gz)包含命令、工具摘要、
全部预热和正式样本、阶段时间、worker CPU、RSS／映射量及进程吞吐。

| 图 | 搬迁数范围 | update 中位／p95／最大（ms） | 请求到 RUNNING 中位／p95／最大（ms） | 进程耗时中位（ms） |
| --- | --- | --- | --- | --- |
| wide | 0 | 3.442／3.815／3.938 | 5.311／5.927／5.938 | 51.148 |
| array | 0 | 6.607／7.095／7.100 | 9.207／9.933／9.942 | 73.343 |
| chain | 0 | 3.289／3.513／3.528 | 7.040／7.281／7.301 | 56.885 |

这里的零搬迁由实际 moved_objects 证明。尚未测量 candidate，不能将阶段中位数相减
作为优化收益。35 次 collection 的 p95 只描述此批样本，不能证明稳定尾延迟。
三个发布时间点的区间不包括随后广播／解锁／调度，不代表 mutator 实际恢复延迟；
测试 hook 和时钟读取有成本。阶段墙钟可能重叠，worker CPU 单独记录。

复现方法见 [基准说明](../../tests/benchmarks/m44/README.md)。固定 binary／runtime
保存于 target/m44/graphs-baseline-darwin，后续与 candidate 同机交替测量。
