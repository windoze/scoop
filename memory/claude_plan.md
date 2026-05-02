## 当前执行计划

说明：按安全与协作要求，这里记录可执行计划、关键判断依据、进度与变更，不记录逐字内部推理。

1. 按要求先读取 `TODO.md` 作为索引，再按引用顺序检查对应 `TODO-Px.md`，定位第一个标题未带 `[DONE]` 的详细任务。
2. 检查最近一次提交是否直接提到与该任务相关且尚未收尾的问题；若这是当前任务的直接未完成部分或前置依赖，则一并纳入当前处理，或在详细 TODO 中补充为前置任务。
3. 阅读当前任务涉及的代码、测试、规范与约束，确认实现边界，不做超范围历史问题排查。
4. 实施当前任务要求的最小正确修改；若遇到阻塞当前任务的真实缺口或规格不匹配，则先修复，或在对应 `TODO-Px.md` / `TODO.md` 中补充最小前置任务并停止。
5. 运行与当前任务直接相关的验证，再运行要求的质量检查，至少覆盖：相关测试、`cargo fmt`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`（若当前任务范围允许且在合理时间内可执行）。
6. 完成后更新对应 `TODO-Px.md` 的完成记录，并将任务标题前缀改为 `[DONE]`；如索引有变化，同步更新 `TODO.md`。仅当阶段计划发生变化时才更新 `PLAN.md`。
7. 检查工作区中与本次任务相关的未提交修改，按要求创建一次原子提交，然后停止，不继续下一个任务。

## 进度记录

- 已创建本文件，下一步开始读取任务索引并定位首个未完成详细任务。
- 已读取 `TODO.md` 与 `TODO-P4.md`，确认当前首个未完成详细任务为 `P4-T04R`：Review solver / widening / `impl_plan`，确认求解结果完全由 facts 驱动。
- 已检查最新提交：`[P4-T04] Solve outward cases and finalize effect facts`。该提交与 `P4-T04R` 直接相关，提交信息本身未显式声明新的未完成事项；后续仍需以代码、搜索与测试结果复核是否存在必须先修的缺口。
- 下一步：检查 `effect_facts` 相关实现与当前工作区状态，确认是否存在需要在本 review 中直接修复的问题；随后运行任务要求的搜索与定向测试/静态检查。
- 已完成 `P4-T04R` 要求的定向测试与 clippy 复验，当前命令均通过；并确认 `effect_facts` / `effect_refactor_pipeline` 主线中未直接命中 `may_outward_effect`。
- 新发现的可疑点：`HandleSiteEffectFacts` 的 `body_outward_cases` / `arm_outward_cases` / `finally_outward_cases` 由 builder 基于保守 seed 预计算，而 solver 当前只 finalizes `Call` site，未重算 `Handle` site；这可能导致外层 handle/block facts 保留过宽结果。
- 当前计划调整：先为“handle body 内调用的实际 outward 是 schema 子集”补一个最小回归测试；若测试确认问题存在，则在 `BodyEffectSolverFacts` 中补足 handle region 求解输入，并让 solver 在最终站点回填时重算 `Handle` site outward/classification，然后重新跑定向验证。
- 已补两条 solver 回归测试：其中 `refactor_effect_solver_keeps_handle_body_outward_for_plain_call_effects` 在修复前失败，确认 `HandleSiteEffectFacts.body_outward_cases` 会漏掉 handle body 内普通 `Call` 带来的 outward effect；这是当前 review 直接相关的真实缺口。
- 已实施修复：`BodyEffectSolverFacts` 现增加 cleanup-block 与 handle-region metadata；solver 在 final site 回填时会用 finalized site map + region traversal 重新计算 `Handle` site 的 `body_outward_cases` / `arm_outward_cases` / `finally_outward_cases` / `nested_handle_classification`，不再停留在 builder 阶段的半成品。
- 已重跑 `P4-T04R` 的全部定向测试与 clippy，结果通过；并已更新 `TODO-P4.md` / `TODO.md`，将 `P4-T04R` 标记为 `[DONE]`，完成记录中明确写入本次 review 内直接修复的 handle-site finalization 缺口。
- 收尾步骤：检查工作区差异与提交内容，按任务要求创建一次原子 git 提交，然后停止。
