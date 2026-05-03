# 本次执行计划

## 约束说明
- 不在确认当前任务之前做开放式问题排查。
- 以 `TODO.md` 作为索引，以对应的 `TODO-Px.md` 作为任务细节与完成状态的唯一事实来源。
- 本次只完成第一个未完成的详细任务；如果遇到阻塞，只补充最小前置任务并同步索引，然后提交并停止。

## 执行步骤
1. 读取 `TODO.md`，按索引顺序定位对应的 `TODO-Px.md` 文件。
2. 检查详细任务标题是否带有 `[DONE]`，识别第一个未完成的详细任务。
3. 查看最近提交，确认是否存在与该任务直接相关且未完成的遗留工作；若有，将其视为当前任务的一部分或前置条件。
4. 阅读当前任务的详细要求、约束、验证条件与完成记录。
5. 审查与当前任务直接相关的代码、测试、文档与现有实现边界，确认实现路径。
6. 实现当前任务，避免绕过规范或使用临时性变通方案。
7. 运行与任务直接相关的验证；必要时补充或修复测试，直到相关检查通过。
8. 更新任务记录：在对应 `TODO-Px.md` 中将任务标题标记为 `[DONE]` 并填写完成记录；若任务索引状态、标题、顺序或依赖发生变化，同步更新 `TODO.md`；仅在阶段计划确实变化时更新 `PLAN.md`。
9. 检查工作区差异，按要求提交本次任务涉及的全部改动，并停止，不继续下一个任务。

## 进度记录
- 已写入初始计划。
- 已读取 `TODO.md` 并确认首个未完成的详细任务为 `TODO-P6.md` 中的 `P6-T03`。
- 已检查最新提交正文：仅包含 `[P6-T02kR] Review handle-arm binder contract`，未发现需要并入当前任务的额外未完事项说明。
- 当前工作区仅有 `memory/claude_plan.md` 的计划更新，未发现需要合并处理的遗留代码改动。
- 已审查 `P6-T03` 直接相关的 refactor LLVM codegen 入口、`effect_refactor` ABI/query 层、`LateLoweredProgram` 状态图/边界合同，以及现有 legacy body lowering 的调用边界。
- 新发现 blocker：当前 handoff 已发布 `HandleDispatch` 的 `body_state` / `arm_states` / `finally_state` / `handled case -> arm` / pending completion 等合同，但没有 authoritative 地发布“哪些 late-lowered state / boundary 属于该 handle 的 body / arm / finally region，以及某个 boundary 的 outward case 是否应被当前 handle 本地消费”的稳定查询面。若继续实现 `P6-T03`，backend 将被迫在 P6 现场通过 state-graph 遍历重建 handle 子图归属与 boundary 消费路由，等价于重新做高层控制流分析，违反任务约束。
- 已把该 blocker 记录到 `TODO-P6.md`，新增最小前置任务 `P6-T02l`，并同步更新 `TODO.md` 索引与 `P6-T03` 依赖/阻塞记录。
- 下一步：复核工作区差异，使用 `P6-T02l` 相关提交信息提交这些任务编排更新并停止，等待下次 invocation 先完成前置合同任务。
