# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Complete exactly the first task whose title is not prefixed with `[DONE]`, then stop.
- Do not perform broad triage before identifying that task.
- If the task is blocked by an unscheduled prerequisite or a failing unscheduled test/fixture, update `TODO.md`, commit that scheduling change, and stop.

## Planned Steps

1. Read `TODO.md` and identify the first incomplete task.
2. Inspect the latest commit only for directly relevant unfinished work or regressions tied to that task.
3. Read the task details, dependencies, validation requirements, and nearby completion records.
4. Inspect the relevant implementation and tests for that task.
5. Implement the smallest spec-correct change that fully satisfies the task.
6. Add or update targeted tests/fixtures required by the task.
7. Run targeted validation first, then broader validation required by the task or affected area.
8. If any observed failing test/fixture is not already explicitly scheduled, either fix it or add the minimum prerequisite/follow-up task before marking the current task done.
9. Update `TODO.md` by prefixing the completed task title with `[DONE]` and filling in its completion record.
10. Update this file after major milestones or plan changes.
11. Inspect git status/diff/log, then commit all intended changes with a task-tagged message.
12. Stop without starting the next task.

## Current Status

- Identified first incomplete task: `P7-T04-b-2R` in `TODO-6.md`.
- Review found a blocking issue in the reviewed task: `GenericClassDecl` and `MonoClassInit` were type aliases over `ClassInitImpl<T>`, not independent nominal Rust types.
- Applied the review fix by replacing the aliases with distinct `GenericClassDecl` and `MonoClassInit` structs and updating the HIR completeness verifier to avoid the removed shared `ClassInitImpl<T>` API.
- Validation completed: targeted searches, `cargo fmt`, `cargo test -p scoopc_types`, `cargo test -p scoopc --no-default-features --lib hir`, `cargo test -p scoopc --no-default-features --lib`, filtered LLVM layout test, effect_lowered fixtures, run-pass fixtures, and `cargo clippy --all-targets -- -D warnings` all passed.
- Updated `TODO-6.md` and `TODO.md` to mark `P7-T04-b-2R` as `[DONE]` with the review/fix completion record.
- `git diff --check` passed; git status/diff/log inspected before commit.
- Next: commit intended changes for `P7-T04-b-2R` and stop.
