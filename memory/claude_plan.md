# Claude Execution Plan

## Current Invocation

Goal: complete exactly the first incomplete task in `TODO.md`, validate it, update task records, commit the result, then stop. This file records the actionable execution plan, decisions, and progress log for the invocation.

## Execution Plan

1. Read `TODO.md` and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check recent git history only for unfinished work that is directly relevant to the selected task.
3. Inspect the task body, referenced files, dependencies, and validation requirements.
4. If a concrete prerequisite blocks correct execution, update `TODO.md` with the minimum prerequisite task, record the blocker here, commit the bookkeeping, and stop.
5. If the task is implementable as written, make the smallest correct code, fixture, and documentation changes needed without weakening the task or using workarounds.
6. Run targeted validation first, then the task-required checks and any broader checks warranted by the touched area.
7. Fix any regression caused by the task before marking it complete.
8. Mark the completed task heading with `[DONE]` in `TODO.md` and update its completion record with concrete change and validation notes.
9. Update `PLAN.md` only if phase-level sequencing, dependencies, assumptions, or completion criteria changed.
10. Inspect git status, diff, and recent log; commit all relevant changes for this task with a task-tagged message.
11. Stop without starting the next task.

## Progress Log

- Initialized this invocation plan before repository task selection and implementation work.
- Selected first incomplete task: `P5-T02` in `TODO-5.md`, covering LIR callable, dynamic invoke, dispatch, and resume contracts.
- Recent commits only show completed `P5-T01/P5-T01R`; no directly relevant unfinished issue was identified from the latest commit history.
- Implementation direction: extend the data-only `scoopc_lir_facts` model, publish facts from the production `LirStageOutput` builder using the existing `LateLoweredProgram`, add verifier/dump coverage, run the required P5-T02 validations, then update task records and commit.
- Edited `scoopc_lir_facts` to add structured callable, step, dynamic invoke, dispatch, resume packing, continuation object, and surface-resume dispatch contracts with verifier/dump support.
- Added a production LIR facts builder under `crates/scoopc/src/pipeline/` and wired `LirStageOutput` construction to publish the richer facts from post-opt LIR plus explicit MIR/effect-facts stage inputs.
- Validation passed after updating effect-lowered goldens: `cargo fmt`, `cargo check -p scoopc_lir_facts`, `cargo test -p scoopc_lir_facts`, `cargo test -p scoopc --no-default-features effect_lowered`, `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`, `cargo run -p scoop_tools -- dependency-gate`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check`.
- Marked `P5-T02` complete in `TODO.md` and `TODO-5.md`; no `PLAN.md` update was needed because the phase-level plan did not change.
