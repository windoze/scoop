# Claude Execution Plan

## Scope

- Follow the indexed task workflow: read `TODO.md`, then inspect referenced `TODO-Px.md` files in order.
- Select the first detailed task whose heading is not prefixed with `[DONE]`.
- Complete exactly that one detailed task, then stop after committing.
- Do not perform unrelated bug sweeps or move to later tasks.

## Current Plan

1. Read `TODO.md` as the global index.
2. Inspect the referenced detailed TODO files in task order to identify the first incomplete task.
3. Read the selected task details, dependencies, validation requirements, and completion record.
4. Inspect only the relevant code, fixtures, docs, and tests needed for that task.
5. Implement the smallest spec-correct change that fully satisfies the selected task.
6. Add or update tests/fixtures required by the task.
7. Run targeted validation first, then broader relevant validation as needed.
8. Fix any failures that block the selected task without using workarounds or weakening the required behavior.
9. Mark the selected detailed task `[DONE]`, update its completion record, and sync `TODO.md` if needed.
10. Commit all changes for this invocation with a task-specific message.
11. Stop without starting the next task.

## Roadblock Policy

- If a concrete prerequisite blocks correct implementation, keep the current task incomplete.
- Add the minimum prerequisite task in the appropriate detailed TODO file, sync `TODO.md`, commit those bookkeeping changes, and stop.
- Update `PLAN.md` only if phase-level sequencing or completion criteria change.

## Status Log

- Started invocation and recorded the initial execution plan.
- Read `TODO.md` and the referenced P6 detailed files.
- Identified the first non-`[DONE]` detailed task as archived old `P6-T03` in `TODO-P6-part2.md`, not active `P6-T03b`.
- Confirmed latest commit is `[P6-T03a] Extract refactor value primitives` and does not mention a directly relevant unfinished issue.
- Current action: repair task bookkeeping for the already migrated old `P6-T03` by marking it `[DONE] [ABANDONED]` in the detailed file and syncing `TODO.md`; do not implement `P6-T03b` in this invocation.
- Updated `TODO-P6-part2.md` and `TODO.md` so archived old `P6-T03` is visibly `[DONE] [ABANDONED]` in both the source-of-truth detail file and index.
- Next action: verify the diff, then commit only this bookkeeping change and stop.
