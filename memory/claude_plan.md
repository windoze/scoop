# Claude Plan

## Notes

- This file records an actionable execution plan and progress log for the current invocation.
- It does not contain private internal reasoning; it captures decisions, next steps, blockers, and verification status.

## Initial Plan

1. Read `TODO.md` as the task index.
2. Open the referenced detailed task files in order (`TODO-P0.md`, `TODO-P1.md`, `TODO-P2.md`, etc.) and identify the first task whose title is not prefixed with `[DONE]`.
3. Check the latest commit message for any directly relevant unfinished issue tied to that task.
4. Read the current task requirements carefully, then inspect only the code and tests needed to implement that task.
5. Implement the task completely with the smallest correct change set.
6. Run the required validation commands, including targeted tests first and broader checks as needed.
7. If a concrete blocker prevents spec-correct completion, add the minimum prerequisite task(s) in the appropriate detailed TODO file, sync `TODO.md`, and stop.
8. If the task is completed, mark it `[DONE]` in the relevant `TODO-Px.md`, sync `TODO.md` if needed, and update completion records.
9. Commit all required changes with a task-specific message, then stop.

## Progress Log

- Created the initial execution plan before repository inspection.
- Read `TODO.md` and identified `P5-T01` in `TODO-P5.md` as the first incomplete detailed task.
- Checked the latest commit message (`[P4-T05R] Verify P5 can consume MIR plus facts`) and found no separately tracked unfinished prerequisite directly attached to `P5-T01`.
- Read the `P5-T01` task body and validation requirements in `TODO-P5.md`.

## Current Working Plan

1. Inspect the existing refactor pipeline stage outputs around MIR/effect-facts to determine where the new P5 stage should attach.
2. Inspect `crates/scoopc/src/lib.rs` and current module layout to add a public `effect_lowered` subsystem without touching legacy state-machine logic.
3. Implement the smallest complete P5 stage boundary:
   - add the new `effect_lowered` module tree,
   - define the late-lowered IR container and stage output type,
   - add a shared stage entry in the refactor pipeline that consumes the P4 stage output.
4. Add targeted tests named around `refactor_effect_lowered_stage_*` to prove construction, dependency on P4 stage output, and isolation from LLVM/legacy state-machine entrypoints.
5. Run the required targeted tests and any necessary formatting/linting for touched code.
6. If successful, update `TODO-P5.md`, sync `TODO.md` if needed, update this plan log, and create a single task commit.

## Implementation Progress

- Added a new public `crates/scoopc/src/effect_lowered/` subsystem and exposed it from `crates/scoopc/src/lib.rs`.
- Added `effect_lowered::LateLoweredProgram` / `LateLoweredCallable` as the initial standalone late-lowered representation container.
- Added `effect_lowered::LateLoweredProgramBuilder` to build the initial late-lowered representation from canonical MIR pass-view plus `MaterializedEffectFacts`.
- Added `crates/scoopc/src/effect_refactor_pipeline/effect_lowering_stage.rs` with `RefactorEffectLoweredStageOutput` as the explicit P5 stage output type.
- Added shared refactor-pipeline entrypoints so tests, future dump CLI, and future P6 lowering can all reuse the same stage helper.

## Verification Progress

- Ran `cargo fmt`.
- Ran `cargo test -p scoopc --no-default-features refactor_effect_lowered_stage` successfully.
- Ran `cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings` successfully.

## Remaining Step

1. Create the task commit that captures the completed `P5-T01` implementation, verification, and TODO synchronization.

## Documentation Progress

- Marked `P5-T01` as `[DONE]` in `TODO-P5.md` and recorded the implementation summary, module mapping, shared stage entrypoints, boundary notes, and verification commands.
- Synced the `[DONE]` marker for `P5-T01` in `TODO.md`.
