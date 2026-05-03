# Claude Plan

## Current Invocation Plan

1. Inspect `TODO.md` as the task index.
2. Open the referenced detailed task files in order (`TODO-P0.md`, `TODO-P1.md`, `TODO-P2.md`, etc.) and identify the first task whose heading is not prefixed with `[DONE]`.
3. Inspect the latest commit message to determine whether it mentions unfinished work that is directly relevant to that first incomplete detailed task.
4. Read the detailed task body for the selected task, including constraints, dependencies, validation requirements, and completion-record expectations.
5. Inspect the relevant code, tests, and fixtures needed to implement that task without changing scope or introducing workarounds.
6. Implement the task completely, or, if blocked by a concrete prerequisite not already tracked, add the minimum prerequisite task(s) in the appropriate detailed TODO file and sync `TODO.md`.
7. Run the relevant validation commands, including targeted tests and broader required checks such as formatting/lint/test commands when they are relevant to the touched code.
8. Update this plan file with progress as key steps complete or if the execution plan changes.
9. Update the appropriate `TODO-Px.md` completion record and prefix the completed task title with `[DONE]` if the task is fully finished; sync `TODO.md` if task state, titles, ordering, or file references changed.
10. Update `PLAN.md` only if phase-level sequencing, dependencies, assumptions, or completion criteria changed.
11. Commit the resulting changes with a task-specific message and stop after this single detailed task (or after recording a blocking prerequisite task if the task cannot yet be completed).

## Progress Log

- Plan initialized before repository inspection.
- Read `TODO.md` and confirmed the first incomplete indexed task is `P6-T03` in `TODO-P6.md`.
- Checked the latest commit: `[P6-T02e] Publish pure caller runtime-error lowering contract`. It is directly relevant prerequisite work, but it does not add a newer unfinished task beyond the blockers already recorded under `P6-T03`.
- Read the `P6-T03` task body and confirmed the task scope: implement refactor LLVM body lowering directly from the P5 late-lowered state graph and boundary contracts, without falling back to legacy effect lowering.
- Next step: inspect the current refactor LLVM codegen modules, the P5 late-lowered representation, and existing tests to determine the smallest correct implementation path for `P6-T03`.
- Inspected `effect_refactor/{layout,types}.rs`, `llvm/emit.rs`, `mir_body.rs`, and representative `dump-effect-lowered` / `dump-mir` outputs for `effect_resume_if_else_branch_single_perform.scoop`.
- Identified a new blocker while mapping P6-T03 requirements to the current authoritative handoff: `RefactorAbiMaterializer::materialize_dynamic_invoke_layouts(...)` only publishes canonical dynamic-invoke LLVM query data for `boundary_map` call boundaries, but P6-T03 must lower whole-body state slices. Straight-line slices can still contain non-boundary `CallKind::{Closure, FunValue, Virtual, Interface}` call rvalues, and the current handoff does not publish an authoritative callable-object ABI/query for those sites.
- Because of that gap, proceeding with P6-T03 right now would require one of the forbidden behaviors: inventing a new callable ABI in the backend, falling back to legacy closure/wrapper lowering, or narrowing the implementation to an easier subset. The correct next action is to add the minimum new prerequisite task ahead of `P6-T03`, sync the TODO index, commit, and stop.
- Added the new prerequisite task `P6-T02f` to `TODO-P6.md`, updated `P6-T03` to depend on it, and synced `TODO.md` with the new index entry.
- Final step for this invocation: review the worktree, commit the blocker/task-order update, and stop so the next invocation can pick up `P6-T02f`.
