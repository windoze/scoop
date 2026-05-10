# Claude Plan

## Note

The user asked for a complete thought process. I cannot provide private internal reasoning verbatim, but I will keep a clear, high-level rationale summary and execution log here so progress is inspectable.

## Initial Plan

1. Read `TODO.md` first and identify the first task whose title is not prefixed with `[DONE]`.
2. Read any directly relevant task details, dependencies, and completion-record expectations in `TODO.md`.
3. Check the latest commit message to see whether it mentions unfinished work directly relevant to that task.
4. Inspect only the code and documents needed for that specific task; avoid broad unrelated triage.
5. Implement the task completely, keeping changes minimal and spec-correct.
6. Run the required validation for the task plus relevant repo-wide checks when appropriate, including fixing any issues caused by the task.
7. Update this file as key steps complete or if the plan changes.
8. Mark the task `[DONE]` in `TODO.md` with an updated completion record, or if blocked, add the minimum prerequisite task(s) in `TODO.md` at the correct position.
9. Update `PLAN.md` only if phase/stage sequencing or dependency structure changes.
10. Commit all current uncommitted changes required by this task with a descriptive message, then stop.

## Progress Log

- Plan file created before repository inspection.
- Read `TODO.md` and identified the first incomplete task as `G0-T01R`.
- Next steps: inspect the latest commit for directly relevant unfinished notes, review the required files, run the mandated grep/check commands, then decide whether `G0-T01R` can be marked done or whether a blocking prerequisite/fix is required.
- Review found a concrete issue directly relevant to `G0-T01R`: several LLVM tests in `crates/scoopc/src/llvm/tests.rs` had their legacy-name assertions removed but were not replaced with new positive checks, leaving them effectively as empty tests.
- Updated plan: repair those tests in-place as part of the review, then rerun the required validation and only mark `G0-T01R` done if the review surface is again meaningful.
- While validating the review, an additional direct regression surfaced in `runtime/c/scoop_runtime.c`: `scoop_runtime_init` / `scoop_alloc` (and related neutral substrate helpers) had been physically deleted, so runtime integration tests could not link.
- Fixed the review findings by:
  - replacing the emptied LLVM tests with positive refactor-surface assertions;
  - restoring the accidentally deleted neutral runtime substrate pieces only (`stdatomic` include, GC-stress globals/parser, Immix allocation helpers, `trimIndent`, `scoop_runtime_init`, `scoop_alloc`, `scoop_gc_collect_safepoint`).
- Validation completed so far:
  - banned-symbol grep across `crates/scoopc/src`, `runtime/c`, `sysroot`: no hits;
  - `cargo check -p scoop_runtime`: passed;
  - `cargo test -p scoop_runtime --tests --no-run`: passed;
  - `cargo test -p scoop_runtime --test runtime_init runtime_init_is_callable_and_observable -- --exact --nocapture`: passed;
  - `cargo test -p scoop_runtime --test alloc scoop_alloc_returns_non_null_and_can_be_called_repeatedly -- --exact --nocapture`: passed;
  - `cargo test -p scoop_runtime --test explicit_root_frame explicit_root_frame_tls_top_and_descriptor_walk_smoke -- --exact --nocapture`: passed;
  - `cargo clippy -p scoop_runtime --all-targets -- -D warnings`: passed;
  - `cargo check -p scoopc`: still fails, but the front errors remain structural target-shape gaps (missing hidden-ABI / outcome / ordinary-callee helpers), matching the intended next tasks.
- Remaining steps: update `TODO.md` completion record for `G0-T01R`, then create the requested git commit and stop.
