# 当前执行计划

说明：本文件记录可审计的执行计划与进度；不会记录隐藏推理链。执行中若计划变化或关键步骤完成，将及时更新。

## 目标

完成 `TODO.md` 中第一个标题未带 `[DONE]` 的任务，然后停止。完成标准包括实现、验证、更新任务记录、提交 Git commit。

## 步骤

1. 读取 `TODO.md`，按文件顺序识别第一个未完成任务，并确认任务正文、依赖、验证要求与完成记录格式。
2. 检查最新提交是否明确提到与该任务直接相关的未完成问题；若存在，将其纳入当前任务或作为必要前置项记录到 `TODO.md`。
3. 只针对当前任务收集代码上下文，避免无关历史问题扫查。
4. 如果当前任务可直接完成，实施最小正确修改；如果发现规范级阻塞问题，则在 `TODO.md` 插入最小必要前置任务并停止。
5. 运行任务要求的验证命令及必要的相关测试；修复由当前任务引入或暴露且阻塞当前任务的问题。
6. 将任务标题加 `[DONE]`，更新 `TODO.md` 完成记录；仅当阶段级计划变化时才更新 `PLAN.md`。
7. 检查工作区差异，确认没有误改无关内容。
8. 使用清晰任务编号提交所有本次任务相关改动。
9. 停止，不继续下一个任务。

## 进度

- 已创建本执行计划。
- 已读取 `TODO.md`，确认首个未完成任务为 `TODO-4-INIT`：分析 P3 需求，生成 `TODO-4.md` 详细任务列表并更新索引。
- 最新提交为 `[P2-T07R] Review P2 completion`，未显示与 `TODO-4-INIT` 直接相关的未完成问题。
- 已读取 `TODO-4.md`、`PLAN.md`、`PIPELINE_REFACTOR.md` 与 `PIPELINE-CLEANUP.md` 的 P3 相关内容。
- P3 任务包必须覆盖：`MirStageOutput = { mir, mir_facts }` 收口、MIR-owned root inventories / snapshot binding / pass artifacts、`mir_facts` 查询面、显式 MIR pass pipeline，以及 HIR 层去虚化删除。
- 已盘点代码中的 `MirStageOutput` / `LoweredMir` / `MaterializedMir` 字段、构造点、下游读取点和现有 pass 入口/顺序。
- 当前事实：`MirStageOutput` 字段在 `pipeline/mir_stage.rs`，`MaterializedMir` 与 pass artifacts 在 `mir/materialize/mod.rs` 与 `mir/pass_view.rs`，现有 pass 顺序在 `mir/materialize/run.rs` 尾部内联执行。
- 已编辑 `TODO-4.md` 与 `TODO.md`：将 `TODO-4-INIT` 标记 `[DONE]`，写入 P3 任务列表与索引，并把 `TODO-4.md` 状态改为已细化。
- 已运行 `git diff --check`，通过，无空白错误。
- 已检查 `git status`、`git diff`、`git diff --stat` 与最近 10 个提交；当前仅 `TODO.md`、`TODO-4.md`、`memory/claude_plan.md` 为本任务相关改动。
- 下一步提交本次 `TODO-4-INIT` 文档细化任务，然后停止。
