# Claude Plan

## Note

I will not record private chain-of-thought. This file contains a concise execution plan, key decisions, blockers, and progress updates.

## Initial Plan

1. Read `TODO.md` as the task index.
2. Open the referenced `TODO-Px.md` files in task order.
3. Identify the first detailed task whose heading is not prefixed with `[DONE]`.
4. Check the latest commit message for any directly relevant unfinished work tied to that task.
5. Read the task body carefully, including constraints, dependencies, and validation requirements.
6. Inspect the relevant code and tests only for the selected task.
7. Implement the task completely, or if blocked by a concrete prerequisite, add the minimum prerequisite task to the correct `TODO-Px.md`, sync `TODO.md`, and stop.
8. Run targeted verification first, then broader required validation such as formatting, tests, and linting as appropriate.
9. Update `TODO-Px.md` completion records and mark the finished task title with `[DONE]`.
10. Sync `TODO.md` if task state, title, ordering, or file references changed.
11. Update this file with progress and any plan adjustments.
12. Commit all relevant uncommitted changes for this task with a task-specific message, then stop.

## Progress Log

- Plan created. Next step: inspect `TODO.md` and the detailed `TODO-Px.md` files to select the first incomplete task.
- `TODO.md` and `TODO-P5.md` inspected. The first incomplete detailed task is `P5-T03`: implement fact-driven boundary selection and whole-function segmentation to produce owner-state / resume-state skeletons.

## Task Focus: P5-T03

### Constraints to preserve

- Boundary selection must be driven only by P4 `MaterializedEffectFacts` plus canonical MIR/P3 explicit structure.
- No fallback to AST/HIR/span/name-based inference.
- No separate fast path for simple functions; `NoOutward` must still go through the same segmentation pipeline.
- No routing new logic through legacy `effect/state_machine/**` as the authoritative implementation.

### Working plan for P5-T03

1. Inspect the current `effect_lowered` IR and builder to see what shell structures already exist for state graph, boundary map, and resume-state map.
2. Inspect `effect_facts` and canonical MIR query surfaces to find the authoritative inputs needed for boundary selection.
3. Determine whether any missing upstream contract makes correct implementation impossible.
4. If no blocker exists, implement a dedicated segmentation module in the refactor path and thread the result into the late-lowered IR.
5. Add focused tests for:
   - boundary selection kinds;
   - self-contained nested handle exclusion;
   - owner/resume mapping in expression/branch/loop contexts;
   - `NoOutward` degenerating through the same entry.
6. Run the required targeted tests and clippy for the touched crate.
7. Mark `P5-T03` as `[DONE]`, update completion records and sync `TODO.md` if needed.
8. Commit exactly this task and stop.

### Current status

- No blocker identified yet. Next step: inspect `crates/scoopc/src/effect_lowered/**`, `crates/scoopc/src/effect_facts/**`, and relevant MIR pass-view structures.
- Inspection complete. No prerequisite task is needed before `P5-T03`.
- Key implementation decision:
  - use canonical MIR + P4 facts to select boundaries;
  - split states at statement-level call/resume sites and terminator-level perform/handle sites;
  - keep raw CFG successor skeleton in the late-lowered state graph;
  - record owner/resume mappings separately in `boundary_map` / `resume_state_map`;
  - represent in-block resume points explicitly so boundaries inside expression/argument/condition contexts do not collapse back to whole-block granularity.
- Next step: edit `effect_lowered/ir.rs`, add `effect_lowered/segment.rs`, and wire the builder to publish real segmentation skeletons.
- Core implementation complete:
  - added `crates/scoopc/src/effect_lowered/segment.rs`;
  - wired `effect_lowered/builder.rs` to emit real segmentation results for callables with bodies;
  - extended `LateLoweredState` with MIR slice coverage and successor skeletons;
  - added fact-driven boundary selection for call / perform / resume / runtime error / nested outward handle;
  - fixed a blocking builder contract bug by skipping declaration-only pass-view families that have no callable facts.
- Verification complete:
  - `cargo test -p scoopc --no-default-features refactor_late_boundary_selection`
  - `cargo test -p scoopc --no-default-features refactor_late_segmentation`
  - `cargo test -p scoopc --no-default-features refactor_owner_resume_state`
  - `cargo test -p scoopc --no-default-features refactor_late_lowered_ir`
  - `cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`
  - `cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`
- Documentation updated:
  - marked `P5-T03` as `[DONE]` in `TODO-P5.md`;
  - synced `TODO.md` index;
  - `PLAN.md` unchanged because phase sequencing did not change.
- Next step: review the final diff, commit the task, and stop.
