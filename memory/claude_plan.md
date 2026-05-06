# 执行计划

## 当前目标

完成 `TODO-P7.md` 中的 `P7-T02Zd：闭合 resumed-body raise 穿过 finally pending completion 的 composition/origin contract，解除 P7-T02Z run-pass 阻塞`，并在完成后提交变更并停止。

## 步骤

1. 读取 `TODO.md`，仅作为任务索引使用。
2. 按索引顺序读取对应的 `TODO-Px.md` 详细任务文件，找到第一个标题未带 `[DONE]` 的任务。
3. 检查最新提交是否明确提到与该任务直接相关的未完成问题；如相关，将其纳入当前任务或添加为前置任务。
4. 理解该详细任务的要求、依赖、约束和验证方式。
5. 最小范围实现任务，不引入规避 spec 的 workaround。
6. 运行相关测试；如发现与当前任务直接相关的失败，修复后重新验证。
7. 更新对应 `TODO-Px.md` 的任务标题为 `[DONE]`，补全完成记录。
8. 同步 `TODO.md` 中对应索引项的 `[DONE]` 状态；仅在阶段计划变化时更新 `PLAN.md`。
9. 更新本文件记录关键进度。
10. 检查工作区变更，提交所有与本次任务相关的变更。
11. 停止，不继续下一个任务。

## 进度

- 已写入初始执行计划。
- 已读取 `TODO.md` 与 `TODO-P7.md`，确认首个未完成详细任务为 `P7-T02Zd`。
- 已检查最新提交 `584525af [P7-T02Z] Fix run-pass blockers and add finally prerequisite`，其 unfinished issue 正是当前 `P7-T02Zd` 前置任务，纳入本轮范围。
- 已复现当前阻塞：`effect_resume_finally_body_raise_after_resume.scoop` 在默认 refactor run 下因 pending outward completion `c1` 同时对应 `st11` 与 `st12` fail-fast。
- 正在实现 origin-aware pending completion handoff：新增 pending completion origin 表，并将 finally dispatch 改为使用 origin-specific tag 与 resume state。
- 已让目标 fixture 越过 origin ambiguity，并修复同一 resume state 的多 outward-case composed dispatch。
- 已修正 composed resume 已在 callee surface-resume 中执行当前 handle finally 时，原 resume boundary 继续向外层 handle 路由，避免 finally 重复执行或直接返回未处理 Step。
- 目标 fixture `run` 与 `test --fixtures` 已输出/匹配 golden；相邻 finally fixtures 已通过。
- 已运行相关 Rust 单测与 `cargo clippy --all-targets -- -D warnings`，均通过。
- 已将 `TODO-P7.md` 的 `P7-T02Zd` 与 `TODO.md` 索引同步标记为 `[DONE]`，并写入完成记录。

## 当前任务执行计划

1. 复现 `tests/fixtures/run-pass/effect_resume_finally_body_raise_after_resume.scoop` 在默认 refactor 路径上的失败，记录 fail-fast 或输出差异。
2. 查找 `HandleDispatch` pending completion、resume state、continuation composition 与 finally lowering 的 P5/P6 handoff 数据结构和校验位置。
3. 最小修改 handoff，使 pending outward completion 带有可区分 origin/resume-state/composed-resume 的 authoritative contract，并让 LLVM lowering 消费该 contract。
4. 增加或更新定向测试，覆盖 resumed body 在 finally-protected handle 中再次 raise 的路径。
5. 运行当前任务要求的 run/test fixture 与相邻 finally/continuation 回归。
6. 运行相关 Rust 单测、格式化与 lint；如时间允许扩大到相关模块测试。
7. 更新 `TODO-P7.md` 与 `TODO.md` 的 `[DONE]` 状态和完成记录。
8. 提交本轮所有相关变更。
