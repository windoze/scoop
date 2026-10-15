# 冷加载显式产物时保留同一文件入口

本批基于 `d9ae49dc`。显式 `.slib` 首次通过 locator 读取 manifest 摘要后，原实现丢弃文件入口，构建完成阶段只用路径和摘要新建 BuildArtifact，真正链接时再次打开产物并重读 manifest。

现在摘要缓存未命中时创建 `ArtifactFile`，读出的摘要和已打开文件一起保存在 `BuildArtifact`；locator 候选、prepared 候选和 completed node 共享同一个 `Rc<BuildArtifact>`。命中已有摘要记录时继续只持有摘要／路径，首次需要文件内容时才打开。构建数据的相等比较仍依据路径与摘要，不比较按需读取状态。没有新 wire 格式、兼容分支或 runtime ABI 变化。

缓存实际输入变化、target 不兼容与缺失输入的原测试继续保留；实际 I/O 错误仍按 locator 的读取／打开错误报告。主要数据模块为 61 行，摘要缓存模块 112 行；现有 locator 从 547 行减至 544 行。

## 验收

本机 [workspace clippy](cold-entry-darwin-clippy.log.gz)、[87 项 scoop 测试](cold-entry-darwin-unit.log.gz)、[release](cold-entry-darwin-release.log.gz)通过；[3 项 CLI](cold-entry-darwin-cli.json.gz)为 3 variants、33 processes。Linux [fmt](cold-entry-linux-fmt.log.gz)、[clippy](cold-entry-linux-clippy.log.gz)、[release](cold-entry-linux-release.log.gz)、[90 项 scoop 测试](cold-entry-linux-unit.log.gz)通过；[GNU](cold-entry-gnu-cli.json.gz)和 [musl](cold-entry-musl-cli.json.gz)各 3 项 CLI 通过、3 variants、33 processes，覆盖显式产物摘要更新／复用、多 Cone 独立链接和真实输入错误。命令见[Linux 脚本](cold-entry-linux.py.gz)。

[采样脚本](cold-entry-trace.py.gz)在已有 `m48-artifact-summary` 用例基础上正常更新 consumer 源码并 touch 显式 provider 产物，所有源码、工具、原产物仍存在。[冷加载 trace](cold-entry-gnu-cold.trace.gz)与 [warm trace](cold-entry-gnu-warm.trace.gz)按父进程区分必要的编译子进程读取。[结果](cold-entry-gnu-io.json.gz)：

| 同一父进程 | provider 打开次数 | 读取字节 | manifest 读取字节 | compiler child | 运行退出码 |
| --- | --- | --- | --- | --- | --- |
| 更新后的冷摘要与实际链接 | 1 | 26,444 | 1,814，恰好一次 | 1 | 50 |
| 后续全命中 | 0 | 0 | 0 | 0 | 50 |

冷加载的读取范围没有重叠，说明 discovery 和实际链接共享已读取的 manifest／成员；独立编译子进程按自身需求首次读取依赖仍允许。这项采样不替代最终五轮性能验收。

## 清理与剩余工作

本机 Cargo 结束后清理 [790,932,155 字节 incremental](cold-entry-darwin-cleanup.json.gz)。历史迁移的 GNU／musl 清单均已闭合且所有相关进程结束后，清理 [16 个历史工作目录](cold-entry-historical-cleanup.json.gz)，原占用 122,021,268 KiB，约 116.37 GiB；日志与冻结 release 工具保留。

Compile／ProgramLink 的语义与布局重放、writer 其余复核、dump 最小阶段生成与复用、最终当前代码全量回归和五轮性能验收仍待完成。M48 尚未完成，M38 未启动。
