# 当前执行计划

## 约束
- 以 `TODO.md` 为唯一任务顺序和完成状态来源。
- 只完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 若遇到阻塞当前任务的缺失能力或规格不符，先在 `TODO.md` 中加入最小必要前置任务并提交，不绕过问题。
- 只在阶段计划本身变化时更新 `PLAN.md`。
- 完成后必须更新 `TODO.md` 的任务标题和完成记录，并提交所有相关未提交更改。

## 步骤
1. 读取 `TODO.md`，定位第一个未完成任务，并检查其依赖、验证要求和完成记录格式。
2. 查看最新提交信息，判断是否显式提到与该任务直接相关的未完成问题。
3. 基于当前任务范围阅读必要代码和测试，避免开放式历史问题扫查。
4. 实现当前任务；如发现直接阻塞该任务的真实缺口，改为更新 `TODO.md` 记录前置任务并停止。
5. 运行任务要求的验证命令；若失败，修复并重跑相关验证。
6. 将任务标题标记为 `[DONE]`，补全完成记录；必要时更新 `memory/claude_plan.md` 的进度。
7. 检查 git diff/status，提交本次任务涉及的所有更改。
8. 停止，不继续下一个任务。

## 当前状态
- 已定位并完成当前任务：`MIR-T12R：Review MIR-T12 codegen handoff guard`。
- 最新提交为 `[MIR-T12] Add codegen routing handoff guard`，与当前 review 直接相关，但未显式声明未完成问题。
- 已审查 routing/ABI handoff 实现、fixtures、verifier 负例和 effect-lowered source classification fail-fast 路径；未发现需要归回 `MIR-T12` 的阻塞缺口。
- 已运行 `MIR-T12R` 相关验证命令、最终 clippy，并更新 `TODO.md` 完成记录。
- 下一步：提交本次 review 记录。
