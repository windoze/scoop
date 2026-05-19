# 当前执行计划

## 约束说明

- 我会记录可审阅的执行计划、关键决策、进度和验证结果。
- 不会记录不可公开的逐字内部推理；如遇计划变更，会在本文件更新可检查的原因和下一步。

## 初始步骤

1. 读取 `TODO.md`，按文件顺序找到第一个标题未带 `[DONE]` 的任务。
2. 检查最近提交信息，仅在其明确提到与当前任务直接相关的未完成问题时，把它纳入当前任务或作为前置项记录到 `TODO.md`。
3. 阅读当前任务的要求、依赖和验证标准，必要时查看相关代码、测试和规格。
4. 若任务可直接完成，则实现最小正确改动，并补充或更新相关测试/fixture。
5. 运行任务要求的验证命令；若失败，修复相关问题并重新验证。
6. 完成后更新 `TODO.md`：在任务标题前加 `[DONE]`，并填写完成记录。
7. 仅当阶段级计划、依赖或完成标准发生变化时更新 `PLAN.md`。
8. 提交本次任务涉及的所有变更，提交信息包含任务编号和简要说明。
9. 完成首个未完成任务后停止，不继续处理后续任务。

## 当前状态

- 已读取 `TODO.md`，首个未完成任务为 `P0-T01：冻结 R2 baseline 与迁移清单`。
- 已对照 `PLAN.md`，该任务属于基线冻结和迁移清单记录，不应拆分。
- 最近提交为 `f8a6260a Update plan`，未发现明确提到与 `P0-T01` 直接相关的未完成事项。
- 已完成 P0 审计取证：
  - archive active path：`scoop package` CLI、`build/deps.rs` 的 `.cone` 搜索、`ProjectContext`/`run_frontend` 的 `ConeArchiveApi` 注入、`typecheck_cone_archive` fixture runner、`api.scoopir` export/consume。
  - sysroot privilege active path：`SourceOrigin::Sysroot`、`SourceFile::is_sysroot()`、`@file:AllowIntrinsic`、`check_intrinsic_builtin_annotation_gate`、sealed marker 的 `source.is_sysroot()` gate。
  - runtime 外溢：`scoop_thread_*`、`scoop_sync_*`、`scoop_test_*`、`scoop_once_*` 仍在 runtime C 和 allowlist 中。
  - native-build 基线：`Cone.toml[native-build]` 已解析，driver 只编译当前显式 cone 的 C/C++ sources，`c_sources_extern_call_basic` 为 C source 端到端 fixture。
- 下一步：更新 `TODO.md` 的 P0 完成记录，运行验证命令后再将任务标记为 `[DONE]` 并提交。

## 验证进度

- `cargo build` 通过。
- `cargo test --all --all-targets` 通过（904 个 `scoopc` 单测通过，完整输出由工具截断保存）。
- `cargo run -p scoop -- test` 通过（fixtures: ok，1558 checks）。
- `cargo clippy --all-targets -- -D warnings` 通过。
- 已写入 `TODO.md` 完成记录，并将 `P0-T01` 标记为 `[DONE]`。
- 已检查 `git status --short`、`git diff -- TODO.md memory/claude_plan.md` 和 `git log --oneline -10`；当前仅有 `TODO.md` 与 `memory/claude_plan.md` 两个本任务相关变更。
- 下一步：提交 `P0-T01` 基线冻结记录。
