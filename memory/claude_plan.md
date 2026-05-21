# Claude Execution Plan

## Scope
- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that one task, then stop.

## Execution Plan
1. Read `TODO.md` and the latest commit summary to identify the first incomplete task and any directly relevant unfinished issue.
2. Inspect the task body for requirements, dependencies, validation commands, and completion-record format.
3. Explore only the code paths needed for that task.
4. Implement the smallest spec-correct change without fixture-only hacks or workarounds.
5. Add or update tests/fixtures required by the task.
6. Run the task-specified validation and any focused tests needed for confidence.
7. Update this file after key milestones or plan changes.
8. Mark the completed task title in `TODO.md` with `[DONE]` and update its completion record.
9. Review git status/diff/log, then commit all intended changes for this task with a task-tagged message.
10. Stop without starting the next task.

## Progress
- Initial plan written before running repository commands.
- Identified first incomplete task: `P4-T04` in `TODO-5.md`.
- Task scope: P4 cleanup/documentation/dependency audit only; do not start P5 LIR facts or P7 backend cleanup.
- Required validation: `cargo fmt`, `cargo run -p scoop_tools -- dependency-gate`, `cargo test -p scoopc_effect_facts`, `cargo test -p scoopc --no-default-features effect_facts_stage`, `cargo clippy --all-targets -- -D warnings`, `git diff --check`.
- Audited P4 code paths: no `canonical_snapshot_mut()` remains; `EffectFactsStageOutput` contains only `MaterializedEffectFacts`; P5 consumes MIR and effect facts via explicit `EffectLoweringStageInput`.
- Updated `README.md`, `crates/scoopc_effect_facts` crate docs, `PIPELINE-CLEANUP.md`, and `PIPELINE_REFACTOR.md` to reflect P4 completion and P5/P7 residual boundaries.
- Validation completed: `cargo fmt`, `cargo run -p scoop_tools -- dependency-gate`, `cargo test -p scoopc_effect_facts`, `cargo test -p scoopc --no-default-features effect_facts_stage`, `cargo clippy --all-targets -- -D warnings`, `git diff --check`.
- `P4-T04` is marked `[DONE]` in `TODO.md` and `TODO-5.md`; completion record has been filled.
- Next step: inspect git status/diff/log and commit the P4-T04 changes.
