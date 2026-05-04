# Claude Execution Plan

## Scope
- Follow the task system exactly: read `TODO.md` as the index, then inspect referenced `TODO-Px.md` files in order.
- Complete only the first incomplete detailed task whose heading is not prefixed with `[DONE]`.
- If a concrete blocker prevents correct completion, add the minimum prerequisite task in the relevant detailed TODO file, sync `TODO.md`, commit, and stop.

## Step-by-Step Plan
1. Inspect the task index and detailed TODO files to identify the first incomplete detailed task.
2. Check the latest commit message for any unfinished issue directly relevant to that task.
3. Read the selected task body, constraints, dependencies, and validation requirements.
4. Inspect only the relevant implementation, test, fixture, and documentation files needed for that task.
5. Implement the smallest spec-correct change that satisfies the task without workarounds or fixture-only behavior.
6. Add or update tests/fixtures required by the selected task.
7. Run the task-specified validation and any focused checks needed for confidence; fix failures that are in scope.
8. Mark the task `[DONE]` in its authoritative `TODO-Px.md` heading and update its completion record.
9. Sync `TODO.md` if the completed task appears in the index or if task ordering/titles changed.
10. Update this plan file when key steps complete or if the plan changes.
11. Review the worktree, commit all task-related changes with a clear task-tagged message, then stop.

## Progress
- Plan file initialized before inspecting the repository state.
- Identified `P6-T06R` in `TODO-P6-part3.md` as the first incomplete detailed task. Latest commit is `P6-T06`, directly relevant but not explicitly marked as unfinished.
- Current task is a review task: verify P4/P5/P6 NoOutward plain ABI contracts, rerun required directed validations, record findings, mark the review done if no blocking issue remains, then commit.
- Reviewed the task bodies for `P4-T06`, `P5-T08`, and `P6-T06`, plus focused implementation areas in effect facts, late-lowered handoff, and refactor LLVM ABI/body/value lowering. No blocker has been identified before running validation.
- Validation blocker found while running `cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`: two stage dump tests fail because a plain body with local effect/control lacks an owner `StepSchema` in the P4/P5 handoff. This is directly in scope for `P6-T06R`; next step is to inspect the failing tests and repair the handoff or test setup without narrowing the validation filter.
- Fixed the blocker by retaining an internal `local_control_step_schema` for plain callables that need local control lowering, without changing their public `Plain` ABI. The previously failing stage filter and `effect_resume_if_else_branch_single_perform.scoop` refactor run-pass now pass.
- Completed the `P6-T06R` review validation set, marked `P6-T06R` `[DONE]` in `TODO-P6-part3.md`, synchronized `TODO.md`, reran affected stage/dump/lint checks after final formatting, and prepared the task changes for commit.
