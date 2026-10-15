# ProgramLink 直接消费成员与符号数据

本批基于 `8e31caf9`，完成正常 ProgramLink 对象／注册／Code 重放的退役，M48 的其他清理与最终验收仍在继续。

## 实现范围

链接读取已保存的物理 member 到 typed definition 映射、defined symbol owners、undefined symbol uses、ODR ABI 和 native requirements。最终成员、ODR 和 image 选择使用这些实际数据；Code 使用 manifest 中已有值，RuntimeImage 使用上一批保存的记录。删除 `layout_link_objects`、`layout_link_symbols` 与不再被生产端使用的 terminal proof 分支，不再生成 final／provisional 对象副本或重新解析 production 来计算 Code。

`ArtifactFile` 的克隆共享同一文件句柄、manifest 和按需成员缓冲区。ProgramLink 只接受真实文件输入，输出继续持有该共享入口；链接先检查稳定对象文件，存在时直接传路径，缺少输出时才从归档读取该成员并保存。用户 native 对象仍为实际符号索引而读取，未新增缓存内容校验或另一套严格 reader。格式和 runtime ABI 未改变。

独立旧 single-cone Link 资格链、Compile／ProgramLink 的语义与布局重放仍待后续清理；本批不把移除正常对象重放等同于全部证明链已消失。dump、历史 fixture 迁移、最终全量和五轮性能验收也尚未闭合，M38 未启动。

## 本机验证

- workspace clippy、release通过。
- slib 564 项、稳定对象存储 1 项、scoop 87 项、linker 4 项通过。slib 较上一批减少的 6 项对应已删除的 terminal 证明链专测。
- 实际 ProgramLink与实际泛型机器对象各 1 项通过。泛型对象 helper 的 manifest 固定 map 长度更新为 production/7 已有的 13 个字段，并保留实际 ODR 目录与 wire 往返断言。
- 17 项正式 CLI、报告通过：22 variants、245 processes，覆盖依赖图、普通输入错误、稳定对象复用／缺失结果补产、泛型 ODR 合并与冲突、三类 runtime image、argv、M46 序列化、C/C++、GC 和 Test 用途。
- 本机 cargo 工作结束后清理 3,178.9 MiB incremental；远端活动构建与历史迁移目录保留。

## Linux 与 I/O 采样

验收使用独立的 `target/m48-summary-reader`。历史迁移仍使用原 `target/release`，运行脚本见 Linux 验证与链接 I/O 采样。

Linux fmt、workspace clippy、release、565 项 slib、稳定对象存储 1 项、90 项 scoop、17 项 linker，以及实际 ProgramLink／泛型机器对象各 1 项通过。GNU 的 17 项 CLI 通过：22 variants、245 processes；musl 16 项通过、1 项声明不适用（native C++），20 variants、231 processes。完整记录见 Linux slib、Linux scoop、Linux linker、GNU 报告与musl 报告。

GNU 显式 `scoop link` 的 strace和摘要显示，三个 `.slib` 各打开一次，读取范围无重叠，每份恰好读取 manifest 与三个 metadata 成员；4,458 个 ELF 归档成员均未读取。最大的 42,412,404 字节归档只读取 30,297,991 字节。采样同时确认无 `scoopc` execve，产出的真实程序输出 `42\n`。源码、工具和原始产物在整个过程中保留。该观察证明稳定对象文件的复用，不表示语义／布局消费已无需重放。
