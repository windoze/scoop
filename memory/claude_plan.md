# Claude Execution Plan

本文件记录本次调用的可审计执行计划、关键进展和计划变更。说明：我会记录决策依据和步骤，但不会写入不可公开的逐字内部推理链。

## 初始计划

1. 读取 `TODO.md`，按文件顺序找到第一个标题未以 `[DONE]` 开头的任务。
2. 在开始实现前，只围绕该任务读取必要上下文，包括任务正文、相关代码、测试和最近提交中与该任务直接相关的信息。
3. 如当前任务被具体缺陷、缺失语言特性或规格不匹配阻塞，先判断该问题是否直接阻塞当前任务；若阻塞且不能在本次完整修复，则在 `TODO.md` 中插入最小前置任务，提交后停止。
4. 若未被阻塞，按任务要求做最小正确实现，不引入规避性改法或 fixture-only hack。
5. 添加或更新覆盖当前行为的测试/fixture，并运行任务要求的验证命令；若失败，定位并修复。
6. 完成后将该任务标题前缀改为 `[DONE]`，更新完成记录；仅当阶段级计划确实改变时才更新 `PLAN.md`。
7. 检查工作区差异，提交本次任务相关所有变更，提交信息使用任务编号开头。
8. 完成一个任务后停止，不继续下一个任务。

## 进展记录

- 已确定第一个未完成任务：`P7-C5：B-10 Effect-typed callable adapter / ABI routing 实现`。
- 当前执行重点：确认最近提交是否包含与 B-10 直接相关的未完成事项，列出 B-10 active IDs，并梳理需要退场的 production fallback、fixtures、audit/stale count 文件。
- 已确认最近提交仅为 P7-C4 完成提交；P7-C4 completion record 中提到的 effect run-pass 历史失败与当前 B-10 effect callable/routing task 相关，纳入本任务验证范围。
- `umb-audit list --bucket B-10` 显示 109 条 active，全部为 `RealImpl`；`tests/fixtures/effect_lowered/` 当前通过，B-10 umbrella fixtures 仍因 `IGNORE-UNTIL-FIX:B-10` 被跳过。
- 代码阅读结论：多数 B-10 `UnsupportedMainBody` 站点已有真实 lowering，可迁移为 verifier-backed internal invariant；少数路径需要补 effect-typed callable/continuation/named-arg routing 或确认由上游 contract 保证。
- 已完成第一批 production 修改：`crates/scoopc/src/llvm/codegen/**` 中的 B-10 `UnsupportedMainBody` constructor 已清空；`UnsupportedMainBody` 只剩 enum/diagnostic 与 audit helper 文本。同步修复了 materialized MIR pattern extract result type 的等价性判断，避免 continuation/Option pattern 绑定因等价 TypeId 漂移被误拒。
- 已同步 B-10 retired ledger 与 active inventory：`umb-audit stats` 目前显示 active=0、retired=1284、initial=1284；B-10 fixtures 已激活并通过 targeted run。下一步运行完整验证并补 TODO 完成记录。
- 已完成最终验证并更新 `TODO.md`：`cargo run -p scoop -- test`、`cargo test --all --all-targets`、`cargo clippy --all-targets -- -D warnings` 均通过；P7-C5 标记为 `[DONE]`，active=0、retired=1284。
