# Execution Plan

I will follow `TODO.md` as the source of truth and complete exactly the first incomplete task. I cannot record private chain-of-thought, but this file contains the complete operational plan and progress log for this invocation.

## Selected Task

First incomplete task: `P3-T07R`, "Review default internal visibility".

The latest commit is `[P3-T07] Default declarations to internal visibility`; it is directly relevant and supplies the implementation under review. The only unrelated uncommitted file currently visible is `GC_PACING.md`, which I will not modify or include.

## Step-by-Step Plan

1. Re-read the `P3-T07` completion record and `P3-T07R` requirements in `TODO-3.md`.
2. Inspect the implementation surfaces named by the review task:
   - `crates/scoopc_hir/src/resolve/mod.rs`
   - `crates/scoopc_cone/src/scoopir/export.rs`
   - `crates/scoopc_cone/src/visibility.rs`
   - sysroot public API declarations under `sysroot/lib/**/src/*.scoop`
   - cone/export fixtures, especially `default_internal_export` and `public_api_filter`.
3. Verify that no-modifier declarations resolve as `Visibility::Internal`.
4. Verify that `.cone` export is still driven only by explicit `Visibility::Public` and does not re-export `internal` declarations.
5. Verify that same-cone internal visibility and downstream invisibility are covered by fixtures.
6. If review finds a direct defect in the completed task, fix it without weakening the visibility contract.
7. Run required validation in order: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, targeted cone/export fixtures, `cargo test --all --all-targets`, and `python3 tools/run_fixtures.py`; also run `python3 tools/spec_fixtures.py check` if review changes any spec-fixture-relevant content.
8. Mark `P3-T07R` as `[DONE]` in both `TODO.md` and `TODO-3.md` and fill the completion record. Do not update `PLAN.md` unless the phase-level plan changes.
9. Commit the review task changes with the required co-author trailer, then stop.

## Progress Log

- Selected `P3-T07R` as the first incomplete task.
- Confirmed the latest commit is the directly relevant `P3-T07` implementation commit.
- Completed review of the named visibility surfaces and found no code defects requiring changes.
- Validation passed: formatting, clippy, targeted cone/export fixtures, full Rust tests, spec fixture check, and full fixture suite.
- Marked `P3-T07R` as `[DONE]` in `TODO.md` and `TODO-3.md`; next step is committing the review task documentation.
