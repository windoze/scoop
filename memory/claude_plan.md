# Claude 执行计划

我不能记录私密逐步推理链；本文件记录可审计的执行计划、关键决策和进度。

## 当前目标

- 按 `TODO.md` 的权威顺序，只完成第一个标题未带 `[DONE]` 的任务，完成后更新记录、验证并提交，然后停止。
- 当前任务：`P3-T03`，实现 `syslib` path trust gate 与 intrinsic privilege gate。
- 范围约束：不跳过 review 任务，不为方便拆分任务，不用 fixture-only hack 或弱化规格的方式绕过缺失实现。

## 执行步骤

1. 读取 `TODO.md`，确认首个未完成任务及其验证要求。
2. 查看最近提交，只处理与当前任务直接相关的未完成问题。
3. 检查 `source.rs`、sysroot loading、annotation gate、sealed marker gate、cone package loader 和相关 fixtures。
4. 实现显式 trusted `syslib` source trust，移除 `SourceOrigin::Sysroot` 对语言特权的直接授权。
5. 增加 `kind = "syslib"` 的 `sysroot/lib/<cone.fqn>/` path gate，并覆盖用户路径自声明 `syslib` 的拒绝行为。
6. 更新 intrinsic / `@file:AllowIntrinsic` / sealed marker fixtures，使 trusted syslib surface 由 `.sysroot` overlay 或 sysroot source 承载，用户 source 只消费或触发未授权诊断。
7. 运行定向测试、全量 Rust 测试、clippy 和完整 fixture suite。
8. 在 `TODO.md` 标记当前任务 `[DONE]` 并填写完成记录；不更新 `PLAN.md`，因为阶段级计划未变化。
9. 检查最终 diff/status/log，提交本次任务所有相关变更后停止。

## 进度记录

- 已确认首个未完成任务为 `P3-T03`。
- 最近提交为 `[P3-T02] Allow lib cones without main`，未发现改变当前任务范围的未完成事项。
- 已实现 `SourceTrust::TrustedSyslib`，sysroot/frontend support source loading 显式赋予 trusted syslib trust。
- 已将 `@Intrinsic`、`@file:AllowIntrinsic` 和 sealed marker gate 改为查询 `SourceFile::is_trusted_syslib()`，不再查询 `SourceOrigin::Sysroot` 或路径片段作为语言特权。
- 已为 `load_cone_source_package` 添加 `syslib` path gate：用户/外部路径下的 `kind = "syslib"` 会被稳定拒绝，trusted sysroot lib path 在单测中保留无 main 加载能力。
- 已更新 typecheck/run-pass/UMB intrinsic fixtures：trusted syslib declarations 移到 companion `.sysroot` overlay，用户 fixture 不再直接通过 `@file:AllowIntrinsic` 自行开权。
- 已更新 HIR golden 中受 sysroot 注释收口影响的 `target_decl_span`。
- 已更新 `TODO.md`：任务索引和标题均将 `P3-T03` 标记为 `[DONE]`，并写入完成记录。
- 验证已完成：`cargo fmt`、`cargo build`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、定向 fixture suites 和完整 `cargo run -p scoop -- test` 均通过。
- 下一步：提交本次任务变更，然后停止。
