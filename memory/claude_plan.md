# Current Invocation Plan

This file records the execution plan and progress for the current autonomous task invocation. It intentionally contains a concise, reviewable plan rather than private reasoning.

## Plan

1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit only for unfinished work directly relevant to that selected task.
3. Inspect the task requirements, dependencies, and validation instructions from `TODO.md`.
4. Implement the selected task completely without changing scope or relying on workarounds.
5. If a concrete blocker or missing prerequisite is found, update `TODO.md` with the minimum required prerequisite task, commit that bookkeeping change, and stop.
6. Run the relevant tests and quality checks required by the task, fixing issues that are in scope.
7. Mark the completed task title in `TODO.md` with `[DONE]` and update its completion record.
8. Update this file when key steps complete or if the plan changes.
9. Commit all changes for this task with a clear task-tagged message.
10. Stop after completing exactly one task.

## Progress

- Plan initialized before reading task details or running commands.
- Identified first incomplete task: `HIR-T02` (`在 parser 拒绝纯语法延期/非法 surface`).
- Implemented parser-side gates for deferred `spawn`/`join`, assignment expressions, spread args outside calls, and named args outside calls.
- Added parse fixtures for the five HIR surface gate cases and updated existing spawn/join negative fixture expectations to parser diagnostics.
- Fixed legal annotation `name = value` parsing so annotation arguments no longer rely on assignment-expression AST.
- Verified the task with the targeted parser unit test, refactor parse fixtures, affected typecheck fixtures, and strict clippy.
- Marked `HIR-T02` as `[DONE]` in `TODO.md` with a completion record.
