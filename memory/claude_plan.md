# 当前执行计划

说明：本文件记录可审计的执行计划、关键决策和进度更新；不会记录私有逐步推理内容。

## 目标

- 严格以 `TODO.md` 为任务源，完成第一条标题未带 `[DONE]` 的任务。
- 完成实现、验证、文档化任务完成记录，并提交 Git commit。
- 完成一个任务后停止，不继续处理下一项。

## 步骤

1. 读取 `TODO.md`，按顺序定位第一条未完成任务。
2. 查看最近提交信息，若明确提到与当前任务直接相关的未完成事项，将其纳入当前任务或作为前置项记录到 `TODO.md`。
3. 读取当前任务相关代码、测试、规格或文档，确认任务要求、依赖和验证命令。
4. 若发现阻塞当前任务的缺失功能、规格不符或实现边界，按要求更新 `TODO.md` 增加最小前置任务，提交并停止。
5. 若无阻塞，实施当前任务所需的最小正确代码和测试变更。
6. 运行相关测试；必要时运行更广泛的验证命令，修复当前任务引入或暴露且直接相关的问题。
7. 更新 `TODO.md`：在当前任务标题前加 `[DONE]`，补充完成记录和实际验证结果。
8. 如阶段级计划未变化，不更新 `PLAN.md`。
9. 检查工作区变更，提交所有与本次任务相关的未提交文件；若是恢复上次未完成任务，则按用户要求一并提交当前未提交文件。
10. 停止并汇报完成内容、验证结果和提交信息。

## 当前状态

- 已读取 `TODO.md`，第一条未完成任务为 `HIR-T12：建立 top-level init/storage/object metadata handoff`。
- 最近提交为 `HIR-T11`，未记录与 `HIR-T12` 直接相关的未完成事项。
- 已确认当前代码已有 `TopLevelVar` / `TopLevelConst` / `TopLevelImmutableValue` / `ObjectInit` side table，但 refactor typed HIR dump/contract 未统一展示这些 init/storage roots，且 `@Extern` 顶层变量尚无 HIR side table。
- 已新增 extern global side table、typed HIR init/storage root contract、稳定 dump 格式和定向单测/fixture 草案。
- 已运行 `cargo fmt`。
- 已运行并通过：`cargo test -p scoopc --no-default-features refactor_hir_top_level_init`。
- 已生成并修正 `tests/fixtures/hir/refactor_top_level_init.hir`，已运行并通过新增 HIR fixture。
- 已运行并通过：`cargo test -p scoopc --no-default-features refactor_hir_no_todo`、`cargo test -p scoopc --no-default-features refactor_typed_hir`、`cargo test -p scoop --no-default-features dump_hir`、`cargo clippy -p scoopc -p scoop --no-default-features --all-targets -- -D warnings`。
- 已更新 `TODO.md`，将 `HIR-T12` 标记为 `[DONE]` 并补充完成记录；`PLAN.md` 未发生阶段级变化，未更新。
- 下一步：检查工作区 diff，并提交本次任务变更。
