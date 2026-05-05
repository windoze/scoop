# Claude Execution Plan

## Scope

- Work through exactly one detailed TODO task in repository order.
- Treat `TODO.md` as the index and `TODO-Px.md` files as the source of truth.
- Stop after completing the first incomplete detailed task, or after recording and committing a concrete prerequisite/blocker if the task cannot be completed as written.

## Constraints

- Do not use workarounds, weakened fixtures, or spec deviations.
- Do not change `PLAN.md` unless phase-level sequencing, dependencies, assumptions, or completion criteria actually change.
- Keep `TODO.md` synchronized with the detailed TODO file if task completion markers, task ids, titles, ordering, or dependencies change.
- Mark a task complete only by prefixing its detailed heading with `[DONE]` and updating its completion record.
- Commit the completed task or blocker/prerequisite update before stopping.

## Step-By-Step Plan

1. Read `TODO.md` to determine the indexed task order and referenced detailed files.
2. Inspect the detailed `TODO-Px.md` files in order until the first task heading without `[DONE]` is found.
3. Check the latest commit message for a directly relevant unfinished issue before implementing the selected task.
4. Read the selected task requirements, dependencies, constraints, and validation instructions.
5. Inspect only the relevant source, fixture, and test files needed to implement that task correctly.
6. Implement the smallest spec-correct change that satisfies the task.
7. Add or update tests/fixtures required by the task.
8. Run the relevant validation commands first, then broader checks if practical and required by the task.
9. Fix any issues introduced by the implementation; if a concrete prerequisite blocks spec-correct completion, update the relevant TODO files instead and stop after committing that bookkeeping change.
10. Update the detailed TODO completion record and prefix the completed task heading with `[DONE]`; synchronize `TODO.md` if needed.
11. Commit all uncommitted files that belong to this task with a descriptive task-tagged message.
12. Stop without starting the next task.

## Progress Log

- Initialized execution plan before repository inspection.
- Selected first incomplete detailed task: `P7-T01` in `TODO-P7.md`.
- Latest commit is `[P6-T05R] Record final plan status`; it does not state an unfinished issue directly blocking `P7-T01`.
- Current implementation target: flip omission/default selector behavior to `refactor` while preserving explicit `legacy` and `refactor` CLI modes.
- Relevant implementation points found: `crates/scoopc/src/session/mod.rs`, `crates/scoopc/src/driver_cli.rs`, `crates/scoop/src/cli.rs`, and fixture subprocess selector helpers in `crates/scoop/src/fixtures/**`.
- Required edit: make `Refactor` the true default everywhere, and invert fixture subprocess propagation so only explicit `legacy` must be passed after the default flip.
- Implemented default flip in session, `scoop` CLI, `scoopc` driver CLI, and fixture subprocess selector propagation; updated targeted tests and dispatcher/session comments.
- Next step: run formatting, targeted no-default-features tests, then required smoke commands for default and explicit legacy paths.
- Verification progress: `cargo fmt --all`, targeted `scoop`/`scoopc` no-default-features tests, fixture helper tests, and required default/legacy dump/build/test smoke commands have passed.
- Next step: run clippy before marking `P7-T01` complete.
- `cargo clippy --all-targets -- -D warnings` passed.
- Next step: mark `P7-T01` complete in `TODO-P7.md`, sync `TODO.md`, then commit the task changes.
- Marked `P7-T01` as `[DONE]` in `TODO-P7.md`, added completion record, and synchronized the `TODO.md` index.
- Next step: review git diff/status and create the required task commit.
