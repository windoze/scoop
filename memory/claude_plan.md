# 当前执行计划

## 目标

完成 `TODO.md` 中第一个标题未带 `[DONE]` 的任务，完成后更新任务记录、验证实现并提交一次 Git commit，然后停止。

## 步骤

1. 读取 `TODO.md`，只定位第一个未完成任务，不做开放式历史问题排查。
2. 检查该任务的依赖、验证要求和完成记录，必要时查看 `PLAN.md` 与最近提交中是否有直接相关的未完成说明。
3. 基于任务内容检查相关代码、测试和 fixtures，确认应修改的最小范围。
4. 按任务要求实现完整修复或功能；如果遇到阻塞当前任务的真实规格/实现缺口，更新 `TODO.md` 加入最小 prerequisite 任务并停止。
5. 运行任务指定验证以及相关测试；若失败，修复后重新验证。
6. 在 `TODO.md` 中将当前任务标题加 `[DONE]`，补充完成记录；仅当阶段计划确实变化时更新 `PLAN.md`。
7. 检查 Git diff，提交所有与本任务相关且当前未提交的更改。

## 进度

- 已读取 `TODO.md`，首个未完成任务是 `HIR-T04：收口 splice field value.[field]`。
- 下一步检查该任务相关的计划说明、最近提交和现有 splice field 实现/测试，确认最小修改范围。
- 已复现 `HIR-T04` 指定 `dump-hir` 验证路径：当前先因顶层 `const val P` 缺类型注解失败；同类本地 `struct` splice fixture 在 refactor HIR 下会被 `Item::Todo(type)` no-Todo gate 拒绝。
- 结论：`HIR-T04` 的本地类型 splice 验证依赖 `HIR-T05` 的 declaration graph；这是当前任务的具体 prerequisite，不应通过削弱 fixture 或保留 placeholder 继续推进。
- 已更新 `TODO.md`：将 `HIR-T05` 排到 `HIR-T04` 前，调整 `HIR-T03`/`HIR-T04`/`HIR-T05`/`HIR-T06` 的进入条件与依赖，并记录 `HIR-T04` 阻塞原因。
- 已更新 `PLAN.md`：记录本地类型 splice field HIR 验证依赖 H4 declaration graph。
- 下一步检查 diff 并提交该 prerequisite 调整，然后停止。
