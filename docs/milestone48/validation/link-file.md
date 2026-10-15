# ProgramLink 共用文件输入

普通 build 后的链接、显式 `scoop link` 和独立 `scoop-link` 都通过 `ArtifactInput` 使用同一个文件读取入口。构建结果保留路径与生产摘要，在真正链接时首次打开文件；显式链接发现与成员消费共享已解析 manifest 和成员缓冲区。重复 Cone 候选比较已有 Artifact 指纹，删除整包字节比较。`ArtifactSnapshot` 已无生产调用，模块及专用测试一并退役。

显式链接不再通过 `ImmutableInputSnapshot` 捕获／哈希整个 `.slib`，不作读取前后状态检查。缺失文件和目录输入返回实际 I/O 错误，artifact origin 保留路径；相关两份精确诊断期望同步更新。manifest 摘要的公共读取适配器继续把 I/O 错误交给既有 locator 的 Read 分类。

## 本机验证

- workspace clippy、87 项 scoop、4 项 linker及正式产物 ProgramLink通过。linker 真实 native 用例耗时 115.85 秒，ProgramLink 用例耗时 71.50 秒，均确认进程后继续等待。首轮 scoop 发现摘要 I/O 分类回归，修复后通过，首轮结果保留。
- release通过；CLI 首轮8 项成功，另测试入口用例暴露旧夹具在两个不同源目录间要求同一缓存路径的问题。改为 production／test 从开始就使用同一工作目录，原来的产物路径和零编译断言保留；只重跑该项，debug／release 两个 variants 均通过。最终覆盖 9 个不同用例、12 variants、125 次成功用例进程，包含三种链接入口、重复依赖与实际图错误、ODR、native、moving／minor GC、M46 序列化、生成测试入口及所需对象补产。
- 最后移除无人使用的 `ArtifactSnapshot` 后，通过 clippy、1 项共享数据测试、2 项文件入口测试及最终 release 构建，未重复无关的 native 大测试。

## Linux 验证与读取采样

独立 `target/m48-summary-reader` 下的 fmt、clippy、90 项 scoop、release 构建及正式产物 ProgramLink通过。ProgramLink 用例实际运行 124.32 秒。

linker 首次执行13 项通过，4 项因独立 target 没有默认查找的 debug `scoopc` 而失败。构建当前 release 并通过 `SCOOP_TEST_PAIRED_SCOOPC` 指定后，只补跑 native alias、DSO 更新、C++／musl和archive 选择，全部通过，17 项结果齐全。保留初始脚本、补跑脚本及完整正确顺序。

GNU 和 musl各通过 9 个不同 CLI 用例、12 variants、125 processes，与本机最终成功选择一致。所有源码、工具及原产物在正常消费中保持存在。

显式 `scoop link` 的 GNU 采样确认三个 `.slib` 各打开一次，manifest 只读取一次，成员读取区间没有重叠；重新链接的程序输出 `42`。准备构建没有 compiler child。保留命令及逐文件统计、采样脚本、准备输出和链接输出。当前 Link replay 仍请求所有机器对象，因此这里 core 的约 41.6 MB 仍几乎全读；本批只证明入口复用与重复读取退役，不宣称对象读取已消失。

确认本机没有 cargo／rustc／clippy 后，清理 incremental 4,020.7 MiB。

## 范围与后续

本批删除链接调用方的整包快照与重复 manifest 读取；旧 Link 对象／注册数据 replay 仍会请求机器对象，不能据此声称机器对象零读取或验证链已退役。Compile 的语义／布局重放、ProgramLink 重放与 final／provisional 数据、dump 输入快照和最小阶段复用、历史迁移及最终全量／五轮性能验收继续属于 M48。M38 未启动。
