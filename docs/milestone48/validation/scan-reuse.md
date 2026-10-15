# Codegen 复用 callable runtime scan 计划

逐函数对象发射直接借用 production 保存的 `StrongCallableRuntimeScanPlanSetV1`，按 body identity 查询本函数计划；不再在每个 LLVM module 中重新扫描整个 LIR Module。非 callable 的 metadata module 同样不构造额外计划。Context cell 与 runtime scan atom 均使用已有 `ObjectSymbolSurfaceV1::atom_boundary` 索引，消除每次查询对整个 Cone 的线性遍历。没有新增索引、全局缓存、wire 字段或 ABI。

初始化注册的声明与 unit identity 映射抽到独立 `emission/initialization.rs`，职责保持；主发射文件为 544 行，初始化模块 100 行，runtime scan emitter 为 187 行。

- Darwin：workspace clippy、374 项 codegen 测试、release 构建通过。7 项 CLI 组合全部通过，共 10 variants、58 processes、3 项必要产物／链接断言。
- Linux：fmt、workspace clippy、389 项 codegen 测试、release 构建通过。GNU和 musl各通过同一组 7 项、10 variants、58 processes、3 项产物／链接断言。
- CLI 覆盖 Context、协程退出、跨 Cone Context、SSA roots、root 类型、scoped data borrowing 与 M46 序列化，保留正常、moving GC／minor GC 运行及实际诊断。源码、工具和原产物始终存在。
- 初始化 helper 的职责拆分后，补做 Darwin clippy及8 项初始化测试，以及 Linux fmt、clippy及同组 8 项测试，全部通过；不为只移动同一实现重复完整 CLI 验收。

确认没有本机 cargo／rustc 进程后，清理 `target/debug/incremental` 756.4 MiB。Linux 使用独立 reader 工具目录，不改写历史隔离迁移回归使用的工具。

本批完成实际 codegen 对已有扫描计划和 atom 索引的复用；不据此声称 Compile／Link 按需读取、消费 replay／双份对象／证明链清理已完成。最终全量与至少五轮性能复测仍待 M48 其余实现完成后进行。
