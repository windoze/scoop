# P3-T04R execution plan

I cannot record private chain-of-thought, but this file captures the actionable task interpretation, execution plan, and progress for this invocation.

## Current task

- Source of truth: `TODO.md`
- First incomplete task: `P3-T04R`
- Task title: Review audit module deletion result.
- Scope: verify the P3-T04 deletion of `crates/scoopc/src/audit/{mod,spec_coverage}.rs`, `crates/scoopc/src/pipeline_gap_audit.rs`, `crates/scoopc/src/pipeline_user_visible_failure_policy.rs`, and their `lib.rs` mount points. Fix review-discovered issues if any. Do not proceed to P3-T05.

## Execution plan

1. Re-read the relevant `TODO.md` section and completion record for P3-T04/P3-T04R.
2. Inspect the latest commit and current worktree so pre-existing unrelated changes are preserved.
3. Search the non-archived source tree for audit module files, module mount points, and direct references that P3-T04 should have removed.
4. Inspect `crates/scoopc/src/lib.rs`, `crates/scoopc/src/`, and related references if searches find anything suspicious.
5. Apply only review fixes required for P3-T04R, if any.
6. Run validation in the required order: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, and any directly relevant fixture/script checks if source changes or observed failures require them.
7. Update `TODO.md` by prefixing `P3-T04R` with `[DONE]` and appending a completion record with review findings and validation outcome.
8. Commit all task-related changes with a descriptive message and the required co-author trailer.
9. Stop after P3-T04R; do not start P3-T05.

## Progress

- Selected first incomplete task: `P3-T04R`.
- Latest commit is `[P3-T04] Remove scoopc Rust audit modules`, directly matching the review task.
- Initial git status includes pre-existing unrelated changes (`.gitignore`, `CALLER_LOCATION.md`, `RTTI_REFINE.md`); they will be preserved and excluded unless the task requires otherwise.
- Confirmed `crates/scoopc/src/audit/**` has no files.
- Confirmed `crates/scoopc/src/lib.rs` has no audit module `#[cfg(test)] mod` mount points.
- Confirmed `rg '\bmod audit\b|\bpipeline_gap_audit\b|\bpipeline_user_visible_failure_policy\b|spec_coverage' crates/scoopc/src` has no matches.
- Validation passed: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, and `python3 tools/run_fixtures.py` (1533 checks).
- Updated `TODO.md` to mark `P3-T04R` as `[DONE]` and appended the completion record.
