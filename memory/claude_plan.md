# Execution Plan

## Scope
- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that task, or add the minimum concrete prerequisite task if the task is blocked.
- Stop after committing the completed task or committed blocker/task-list update.

## Steps
1. Inspect `TODO.md` to find the first incomplete task and its validation requirements.
2. Check the latest commit message only for unfinished work directly relevant to that task.
3. Read the files needed to understand and implement that task.
4. Make the smallest spec-correct implementation changes; avoid workaround behavior.
5. Add or update tests/fixtures required by the task.
6. Run the task-specific validation, plus broader checks when practical.
7. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and recording completion details, or record a blocker/prerequisite if implementation cannot proceed.
8. Update this file after key milestones or plan changes.
9. Commit all relevant changes with a task-specific message.
10. Stop without starting the next task.

## Progress
- Initial execution plan recorded before task inspection.
- First incomplete task identified: `P5-T03` in `TODO-2.md`.
- Task goal: move nine advanced `String` extension helpers from `sysroot/string.scoop` into `sysroot/lang_string.scoop`, preserve behavior, add/verify automatic-prelude visibility, validate, update completion records, and commit.
- Migrated the nine `fun String.*` helpers into `sysroot/lang_string.scoop`, removed them from `sysroot/string.scoop`, and added `lang_string_helpers_auto_prelude` visibility fixture.
- Targeted visibility fixture initially failed with `scoop::resolve::unresolved_member` for `scoop.core.String.substring`.
- Root cause was core internal helpers still calling migrated `substring`; replaced those internal calls with `unsafeSliceBytes` so `scoop.core` does not depend on `scoop.lang.string`. Targeted visibility/string fixtures now pass.
- Full fixture run then exposed comptime failures: the const gate still whitelisted old `scoop.core.*` helper FQNs, and the comptime support path loaded `stdlib/` as sysroot so auto prelude was skipped. Updated both; the `const_fun_string_methods` comptime fixture now passes.
- Full fixture baseline now has only the pre-existing `tests/fixtures/run-pass/mutable_array_ops_basic.scoop` failure recorded by P4-T02; all other targets pass.
- Rust workspace tests and `cargo clippy --all-targets -- -D warnings` pass after updating LLVM ABI assertions for the new `scoop.lang.string.substring` symbol.
- `TODO.md` and `TODO-2.md` updated to mark `P5-T03` as `[DONE]` with completion details. Next step: review diff/status, then commit this task only.
