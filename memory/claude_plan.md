# 执行计划

## 当前约束

- 以 `TODO.md` 为任务排序和完成状态的唯一依据。
- 只完成第一个标题未带 `[DONE]` 的任务，完成后停止。
- 若遇到阻塞当前任务的规格不匹配、缺失特性或实现边界，先在 `TODO.md` 中添加最小必要前置任务并提交，然后停止。
- 完成任务后更新 `TODO.md` 的 `[DONE]` 标记和完成记录；仅当阶段级计划改变时才更新 `PLAN.md`。
- 完成或阻塞记录都需要提交 Git。

## 步骤

1. 读取 `TODO.md`，按文件顺序找到第一个标题未带 `[DONE]` 的任务。
2. 查看该任务的要求、依赖、验证方式和完成记录；必要时检查最近提交是否提到与该任务直接相关的未完成问题。
3. 针对该任务读取最小必要代码和测试上下文，避免无关历史问题排查。
4. 如果任务可以直接完成，按现有架构做最小正确实现，并补充或更新相关测试/fixture。
5. 运行任务要求的验证命令，并根据失败结果修复当前任务范围内的问题。
6. 如果出现必须先修复的具体阻塞项，则更新 `TODO.md` 插入前置任务，保留当前任务未完成，提交后停止。
7. 如果验证通过，则将当前任务标题标记为 `[DONE]`，更新完成记录，并按需更新本文件进度。
8. 检查工作区变更，提交本次任务涉及的全部未提交文件，提交信息使用任务编号和简洁说明。
9. 停止，不继续处理下一个任务。

## 进度

- 已读取 `TODO.md`，确认第一个未完成任务为 `C5-T01：更新 spec、迁移说明与设计文档状态`。
- 已核对最近提交：`[C4-T02] Record execution completion`，未发现与 `C5-T01` 直接相关的未完成项。
- 已读取 `SCOOP_FULL_SPEC.md`、`CLOSURE_FIX.md`、`PLAN.md` 与 sysroot API 相关位置。
- 已更新 `SCOOP_FULL_SPEC.md`：补入 sealed interface marker 规则、closure capture by-value snapshot / per-call reset、Kotlin makeCounter 迁移说明，以及 `RefCell` / `Box` / `Atomic*` shared-state API 摘要。
- 已更新 `CLOSURE_FIX.md` 文件头部：说明实现进度跟踪已移交至 `PLAN.md` / `TODO.md`，本文档保留为历史设计记录。
- 已运行验证：`cargo run -p scoop_tools -- spec-fixtures check` 通过（`spec fixtures: ok (1)`），`cargo run -p scoop -- test` 通过（`fixtures: ok (1405)`），`cargo clippy --all-targets -- -D warnings` 通过。
- 已将 `TODO.md` 中 `C5-T01` 标记为 `[DONE]` 并填写完成记录。
- 下一步检查工作区变更并提交本次任务。
