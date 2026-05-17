# Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose title is not prefixed with `[DONE]`.
- Complete exactly that task, then stop after committing the result.

## Steps

1. Read `TODO.md` and identify the first incomplete task.
2. Check whether the latest commit mentions an unfinished issue directly relevant to that task.
3. Inspect only the files and tests needed for the selected task.
4. Implement the task without workarounds or spec deviations.
5. Run the task's required validation and relevant tests.
6. Update `TODO.md` by prefixing the completed task title with `[DONE]` and adding a completion record.
7. Update this file when key milestones complete or if the plan changes.
8. Commit all relevant changes with a task-specific message.
9. Stop without starting the next task.

## Current Status

- First incomplete task identified: `P8-T01` (`TODO-4.md`), a documentation baseline for current scalar operator behavior.
- Latest commit checked: `4d6d4cdf [P7-T03] Align runtime GC collect symbol`; no directly relevant unfinished issue found.
- Inspected current operator codegen and related type/lowering paths.
- Added `docs/reshape-baseline/operator-behavioral-baseline.md` and validated it with the required `cat` command.
- Updated `TODO.md` and `TODO-4.md` to mark `P8-T01` as done with a completion record.
- Additional checks passed: `git diff --check`; `cargo clippy --all-targets -- -D warnings`.
- Next step: commit the task changes, then stop.
