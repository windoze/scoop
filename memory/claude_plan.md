## Execution Plan

I cannot provide private chain-of-thought notes, but this file will contain a concise execution plan, key decisions, blockers, and progress updates.

1. Read `TODO.md` as the index.
2. Open the referenced `TODO-Px.md` files in task order.
3. Identify the first detailed task that is not clearly recorded as completed.
4. Check the latest commit for unfinished work directly relevant to that task.
5. Inspect the implementation and tests needed for that task.
6. Implement the task completely, or if blocked, add the minimum prerequisite task(s) to the correct detailed TODO file and sync `TODO.md`.
7. Run the relevant formatting, linting, and test commands, fixing issues that are in scope.
8. Update the detailed TODO file with a completion record if the task is finished.
9. Update `TODO.md` if task ids, titles, ordering, or file references changed.
10. Update this file with progress notes and any plan adjustments.
11. Create one git commit with a task-specific message, then stop.

## Progress Log

- Initial plan written before repository inspection.
- Read `TODO.md` as index, then inspected `TODO-P0.md` through `TODO-P4.md` in order.
- Confirmed `P0` through `P4-T01` have explicit completion records; the first incomplete detailed task is `P4-T01R` in `TODO-P4.md`.
- Checked the latest commit: `[P4-T01] Add refactor effect-facts stage boundary`. It is directly relevant to `P4-T01R`, but the commit message/body does not record an unfinished prerequisite that must be inserted ahead of the review.
- Execution focus for this invocation: review the P4 facts-stage boundary by auditing `crates/scoopc/src/effect_facts/**`, `crates/scoopc/src/effect_refactor_pipeline/effect_facts_stage.rs`, `crates/scoopc/src/lib.rs`, `crates/scoopc/src/program_facts.rs`, `crates/scoopc/src/mir/summary.rs`, and `crates/scoopc/src/effect/analysis.rs`; rerun the required targeted tests/searches; then either (a) mark `P4-T01R` complete if the review passes, or (b) record a concrete prerequisite/fix if a blocking issue is found.
- Completed the audit for `P4-T01R`: the new facts terms remain isolated to `crates/scoopc/src/effect_facts/**`, `crates/scoopc/src/effect_refactor_pipeline/effect_facts_stage.rs`, and a P3 handoff comment in `mir_stage.rs`; no leakage was found in `program_facts.rs`, `mir/summary.rs`, or `effect/analysis.rs`.
- Validation completed successfully:
  - `cargo test -p scoopc --no-default-features refactor_effect_facts_stage`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/dispatch_and_resume_call.scoop`
  - `cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`
- Recorded the review result in `TODO-P4.md`; no new prerequisite task or plan change was required.
