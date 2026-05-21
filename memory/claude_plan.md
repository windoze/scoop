# Claude Execution Plan

## Current Invocation

Goal: complete exactly the first incomplete task in `TODO.md`, validate it, update task records, commit the result, then stop.

## Execution Plan

1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Inspect only the files and context needed for that task, plus recent git history if it explicitly mentions an unfinished issue directly relevant to the selected task.
3. Determine whether the selected task can be implemented directly as written.
4. If a concrete blocker or missing prerequisite prevents correct implementation, update `TODO.md` with the minimum prerequisite task in the correct order, record the blocker here, commit that bookkeeping, and stop.
5. If implementation is possible, make the smallest correct code and fixture changes needed to satisfy the task without workarounds or spec deviations.
6. Run targeted tests first, then any validation commands required by the task. Run broader tests if the touched area warrants it.
7. Fix any regressions directly caused by the task before proceeding.
8. Mark the task heading in `TODO.md` with `[DONE]` and update its completion record with implementation and validation notes.
9. Update `PLAN.md` only if phase-level sequencing, dependencies, assumptions, or completion criteria changed.
10. Inspect git status, diff, and recent log; commit all relevant changes for this task with a descriptive task-tagged message.
11. Stop without starting the next task.

## Progress Log

- Initialized invocation plan before running repository inspection commands.
- Selected first incomplete task: `P5-T01R` in `TODO-5.md`, a review task for `scoopc_lir_facts` and the `LirStageOutput` shell.
- Review scope: verify fact-crate dependency boundaries, confirm `LirStageOutput` does not nest P3/P4 outputs, confirm `LateLoweredProgram` is published as the stable LIR body, run P5-T01 validation plus `cargo tree -p scoopc_lir_facts`, then update task records and commit.
- Initial review check: `scoopc_lir_facts` contains a data-only facts crate; `LirStageOutput` stores `lir`, `lir_facts`, and explicit temporary context, not nested upstream stage output wrappers. Proceeding to required validation.
- Validation passed: `cargo fmt`, `cargo check -p scoopc_lir_facts`, `cargo test -p scoopc_lir_facts`, `cargo test -p scoopc --no-default-features effect_lowering_stage`, `cargo run -p scoop_tools -- dependency-gate`, `cargo tree -p scoopc_lir_facts`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check`.
- Marked `P5-T01R` complete in `TODO.md` and `TODO-5.md`; no `PLAN.md` update was needed because the phase-level plan did not change.
