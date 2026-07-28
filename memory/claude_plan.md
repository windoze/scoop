# Claude Execution Plan

## Scope

This invocation will complete exactly the first incomplete task listed in `TODO.md`, then stop after documenting and committing the result. `TODO.md` is the source of truth for task ordering and completion status.

## Reasoning Summary

- Read `TODO.md` first to identify the first task whose heading is not prefixed with `[DONE]`.
- Check the latest commit only for unfinished work directly relevant to that first incomplete task.
- Avoid broad triage or unrelated historical cleanup.
- If the task can be completed as written, implement it fully, validate it, mark it `[DONE]`, update its completion record, and commit.
- If a concrete blocker prevents spec-correct completion, add the minimum prerequisite task in `TODO.md`, leave the current task incomplete, commit the scheduling change, and stop.

## Step-By-Step Plan

1. Read `TODO.md` and identify the first incomplete task by heading prefix.
2. Inspect only the task-relevant context, including the latest commit message if it points to unfinished work for that task.
3. Read the relevant source, tests, fixtures, and specification sections needed for the selected task.
4. Implement the smallest correct change that satisfies the selected task without workarounds or spec deviations.
5. Add or update tests/fixtures required by the task.
6. Run validation in the required order: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, relevant tests, then full tests and fixtures when required.
7. If validation exposes unscheduled failures, fix them if in scope or add the minimum prerequisite/follow-up task before marking completion.
8. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling in the completion record.
9. Update this file whenever the plan changes or a key step completes.
10. Inspect git status/diff/log, stage only intended files, commit with a task-tagged message, and stop.

## Progress Log

- Initial plan written before reading `TODO.md` or running commands.
- Identified the first incomplete task as `TC-04-FIX3: 清除 source-callable/direct-call 残留 FQN live lookup`.
- Checked the latest commit: `[TC-04-R] Schedule source callable FQN lookup fix`; it is directly relevant and already represented by `TC-04-FIX3`, so no extra prerequisite is needed before implementation.
- Added handle-native ABI/signature entry points for `LirCallableRef` / exact callee bindings and began migrating direct-call, dispatch, closure-body, release-hook, and layout query call sites away from the old FQN-named helpers.
