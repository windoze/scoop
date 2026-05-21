# 执行计划

## 当前约束

- 以 `TODO.md` 为唯一任务顺序和完成状态来源。
- 只处理第一个标题未带 `[DONE]` 的任务，完成后停止。
- 若遇到阻塞当前任务的规格缺口或实现缺口，先把最小必要前置任务写入 `TODO.md`，提交后停止。
- 仅在阶段级计划、依赖或完成标准变化时更新 `PLAN.md`。
- 完成任务后必须更新 `TODO.md` 的标题和完成记录，并提交所有相关变更。

## 步骤

1. 读取 `TODO.md`，定位第一个未完成任务，并记录任务编号、范围、依赖和验证要求。
2. 检查最近提交是否提到与该任务直接相关的未完成事项；若有，将其纳入当前任务或作为前置项记录到 `TODO.md`。
3. 按任务要求检查相关代码、规格和测试夹具，确认最小实现范围。
4. 实现任务；若发现阻塞性缺口，不绕行，改为更新 `TODO.md` 并提交后停止。
5. 运行任务要求的验证命令和必要的针对性测试；修复由当前任务引入或暴露且阻塞当前任务的问题。
6. 更新 `TODO.md`：给任务标题加 `[DONE]`，补充完成记录和验证结果。
7. 检查 `git status`、`git diff`、最近提交；提交本次任务相关全部变更。
8. 停止，不继续处理下一个任务。

## 进度记录

- 已创建初始执行计划。下一步读取 `TODO.md` 定位第一个未完成任务。
- 已定位第一个未完成任务：`P5-T03`（切换 codegen-neutral ABI/query surface 到 `LIR + lir_facts`）。
- 最近提交为 `9271f7d2 [P5-T02R] Review LIR contract facts`，是当前任务的直接前置 review，未发现需要先插入的新前置项。
- 下一步检查 `LirStageOutput`、LLVM emit、effect-lowered layout/body lowering 的当前读取点，优先删除 P5-owned contract 对 raw MIR/effect facts 的依赖。
- 当前实现重点：修改 `ProgramAbiMaterializer` / `materialize_program_abi` 只接收 `LateLoweredProgram + LirFacts + TypeStore`，用 `LirFacts` 替代 dynamic invoke、dispatch slot、plain callable ABI 等查询；`LirStageOutput` 将保留 LIR、LIR facts 和必要 type/base context，删除 `materialized_pass_view()`、`effect_facts()`、`mir_facts()` 等公开上游 accessor。
- 已完成实现：ABI materializer 改为消费 `LateLoweredProgram + LirFacts + TypeStore`；dynamic invoke、dispatch、plain callable ABI、surface-resume ABI 的 P5-owned 查询已切到 LIR/LIR facts；`LirStageOutput` 的上游公开 accessor 已删除。
- 已通过验证：`cargo check -p scoopc --features llvm`、`cargo test -p scoopc --features llvm effect_lowered`、`cargo test -p scoopc --features llvm llvm::tests::late_lower`、`cargo clippy --all-targets -- -D warnings`、`git diff --check`。
- 完整 run-pass fixture 命令仍失败 7/415，剩余失败为 array/string generic println materialization、runtime test import、process/atomic/string runtime residual；已记录到 `TODO-5.md` 的 P5-T03 完成记录，未把它们归入 P5-owned contract 完成范围。
- 下一步：完成最终状态检查，提交 P5-T03 变更后停止。
