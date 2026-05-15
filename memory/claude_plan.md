# Claude Execution Plan

## Scope

- Execute exactly the first incomplete task in `TODO.md`.
- Treat `TODO.md` as the authoritative task list and only update `PLAN.md` if phase-level sequencing or dependencies change.
- Avoid unrelated bug sweeps or opportunistic work.
- Do not use workarounds for spec or implementation gaps; add a prerequisite task and stop if a blocker prevents correct execution.

## Steps

1. Read `TODO.md` and identify the first task whose title is not prefixed with `[DONE]`.
2. Check the latest commit message only for an explicitly unfinished issue that is directly relevant to the selected task.
3. Read the selected task details, dependencies, validation requirements, and relevant source/test files.
4. Implement the task as written, making the smallest correct change that satisfies the task without narrowing scope.
5. Add or update focused tests/fixtures required by the task.
6. Run the task-specified validation commands and any additional relevant checks needed for confidence.
7. If validation fails, fix the root cause and rerun the relevant checks.
8. Mark the task title in `TODO.md` with `[DONE]` and update its completion record with implementation and validation notes.
9. Update this plan file at key milestones and if the approach changes.
10. Review the worktree, stage all files required for this completed task, and create one descriptive git commit.
11. Stop after the commit without starting the next task.

## Current Status

- `TODO.md` 已读取。
- `P4-T01` 的正式任务标题已标 `[DONE]`，但任务索引漏标 `[DONE]`。
- 工作区已有大量未提交改动，内容与 `P4-T01` 完成记录对应，说明上一轮 `P4-T01` 已实现但未完成提交步骤。
- 当前执行目标调整为闭合 `P4-T01` 的遗漏提交步骤，并在提交中同步修正 `TODO.md` 顶部任务索引的 `[DONE]` 漏标。
- 已复跑 `P4-T01` 关键验证：`cargo fmt --all -- --check`、`cargo check -p scoopc`、相关 `scoopc` targeted tests、`tests/fixtures/build`、两个 scalar toString run-pass fixture、GC stress variants、`cargo clippy --all-targets -- -D warnings` 均通过。
- 本轮提交完成后停止，不进入 `P4-T02`。
