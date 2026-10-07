# M31 早期 Linux 容器测量

这些原始报告来自独立 Debian 13/amd64 容器，在 Apple M3 Ultra 上由 OrbStack 模拟执行。环境见 [ENVIRONMENT.json](ENVIRONMENT.json)。每个已完成程序保留原输入、构建记录、五次运行和实际 GC 统计，没有删去慢样本。

GNU 的 M30、M31 debug/release 以及 musl 的 M30、M31 debug 均完成八个程序。musl release 在清理运行环境时中断，只完成 aggregate、allocation、arrays、collections、old-graph-large 五项，记录在 [部分报告](M31-LINUX-MUSL-RELEASE-PARTIAL.json)；其余三项没有填造数据。

随后 Linux 测量迁至用户指定的 `nuc12:~/repos/scoop`，完整对照见[性能报告](../PERFORMANCE.md)。原生与模拟环境的时间不合并、不相除。这批记录保留为独立的早期观测，不代替 NUC 的完整对照，也不增加功能验收计数。
