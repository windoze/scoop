# Current Invocation Plan

## Objective

Complete exactly the first incomplete task in `TODO.md`, update task bookkeeping, validate the work, commit the result, then stop.

## Execution Plan

1. Read `TODO.md` and identify the first task whose heading is not prefixed with `[DONE]`.
2. Review the selected task details, dependencies, validation requirements, and any completion notes.
3. Inspect the latest commit only for directly relevant unfinished work if needed for the selected task.
4. Examine the smallest relevant part of the codebase and tests for the selected task.
5. Implement the task as specified without narrowing scope or introducing workarounds.
6. Add or update tests and fixtures required by the task.
7. Run the task-specified validation and any directly relevant test commands.
8. If validation reveals an unscheduled failure, either fix it or add the minimum prerequisite/follow-up task in `TODO.md` before marking the current task done.
9. Mark the task title `[DONE]` in `TODO.md` and update its completion record only after implementation and validation are complete.
10. Update `PLAN.md` only if phase-level sequencing, dependencies, assumptions, or completion criteria actually change.
11. Inspect the git diff, then commit all intended changes with a task-scoped message.
12. Stop after the commit and do not start the next task.

## Progress Log

- Plan initialized before task selection.
- Identified first incomplete task: `P7-T02R` in `TODO-6.md`.
- Latest commit is `[P7-T02-a] Fix run-pass fixture baseline`; it does not advertise an unfinished issue, and it is directly relevant only as the declared dependency for this review.
- Current review focus: verify reachability reads only LIR/LIR facts/base context, confirm devirtualization residuals are gone, run required validations, then update TODO records and commit.
- Static review result so far: `llvm/reachability.rs` only imports and traverses LIR fact types; targeted searches found no `hir::`, `mir::`, `MaterializedMirPassView`, or `devirtual` in that file, and no devirtualization-named residual under `crates/scoopc/src/llvm`.
- Next step: run P7-T02R validation commands, including full run-pass fixture suite with an extended timeout.
- Validation complete: `cargo fmt`; `cargo test -p scoopc --no-default-features llvm::reachability`; `cargo test -p scoopc llvm::reachability`; `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`; `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`; `cargo clippy --all-targets -- -D warnings`; `git diff --check` all passed.
- Full run-pass fixture suite passed 421/421. No unscheduled test or fixture failure was observed.
- `TODO.md` and `TODO-6.md` now mark `P7-T02R` as `[DONE]` with the review conclusion and validation record.
- Final step: commit `TODO.md`, `TODO-6.md`, and this progress file with a P7-T02R-scoped message.
