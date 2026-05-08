# Claude Plan

## Initial execution plan

1. Read `TODO.md` and identify the first task whose title is not prefixed with `[DONE]`.
2. Check the latest commit message for any directly relevant unfinished work tied to that task.
3. Inspect only the code and documents needed for the selected task and its dependencies.
4. Implement the task completely, avoiding workarounds and stopping only if a concrete prerequisite blocker is discovered.
5. Run the task-required validation, then broader required checks as needed.
6. Update `memory/claude_plan.md` with progress, update `TODO.md` completion state and records, and update `PLAN.md` only if phase-level planning changes.
7. Create one git commit covering the task work, then stop.

## Progress log

- Wrote the initial execution plan before repository inspection.
- Read `TODO.md` and identified `CG-T08` as the first task whose heading is not prefixed with `[DONE]`.
- Checked the latest commit message: `[CG-T07S] Complete cross-fixture transport drift audit`. It is directly relevant as the prerequisite unblocker for `CG-T08`, but it does not introduce a newer unfinished prerequisite beyond what `TODO.md` already records.

## Task-specific plan for CG-T08

1. Inspect the existing `CG-T08` matrix test, task notes, and `PIPELINE_GAPS.md` audit state.
2. Run the required validation for `CG-T08`:
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`
3. If validation exposes a blocker that belongs to the current task, fix it immediately; if it reveals a concrete prerequisite outside `CG-T08`, add that prerequisite to `TODO.md` ahead of `CG-T08`, keep `CG-T08` incomplete, and stop.
4. If validation passes, update `PIPELINE_GAPS.md` with a codegen-stage exit audit addendum, then mark `CG-T08` as `[DONE]` in `TODO.md` and extend the completion record with the final validation summary.
5. Commit all task changes in one git commit and stop.

## Execution progress

- Inspected the existing `CG-T08` matrix test in `crates/scoop/tests/cg8_codegen_regression_matrix.rs` and confirmed that the representative fixture coverage for `CG-T01` through `CG-T07` and `P7-T02Z` is already present in the repository.
- Verified the required `CG-T08` commands successfully:
  - `cargo test --all`
  - `cargo run -p scoop -- test` -> `fixtures: ok (1270)`
  - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc` -> `fixtures: ok (25)`
- Updated `PIPELINE_GAPS.md` with a 2026-05-09 codegen-stage exit audit addendum and converted the historical `§5.7` blocker note into a resolved historical record.
- Marked `CG-T08` as `[DONE]` in `TODO.md` and appended the final completion record.
