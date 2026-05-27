# P3-T04 execution plan

I cannot record private chain-of-thought, but this file captures the actionable reasoning, task interpretation, and execution plan for this invocation.

## Current task

- Source of truth: `TODO.md`
- First incomplete task: `P3-T04`
- Task title: Delete Rust audit modules and remove their `lib.rs` test-only mount points.
- Scope: delete `crates/scoopc/src/audit/{mod,spec_coverage}.rs`, `crates/scoopc/src/pipeline_gap_audit.rs`, and `crates/scoopc/src/pipeline_user_visible_failure_policy.rs`; remove the corresponding `#[cfg(test)] mod` entries from `crates/scoopc/src/lib.rs`. Do not proceed to P3-T04R.

## Execution plan

1. Inspect the latest commit message to check whether it mentions unfinished work directly relevant to P3-T04.
2. Inspect the current git status so any pre-existing uncommitted work is understood and preserved.
3. Read the relevant `TODO.md` section for P3-T04 and the associated completion record pattern.
4. Search the non-archived source tree for the Rust audit modules and `lib.rs` mount points named by the task.
5. Delete only the P3-T04 module files and remove their `#[cfg(test)] mod` entries.
6. Re-scan the relevant source paths to confirm no deleted module mount points remain.
7. Run required validation in order:
   - `cargo fmt`
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo test --all --all-targets`
   - fixture validation if source/test changes or observed failures require it under the task policy.
8. If validation exposes unrelated unscheduled failures, fix them or add the minimum scheduled task before marking P3-T04 done.
9. Update `TODO.md` by prefixing `P3-T04` with `[DONE]` in the task index and appending a completion record with validation commands and outcome.
10. Commit all task-related changes with a descriptive message and the required co-author trailer.
11. Stop after P3-T04; do not start P3-T04R.

## Progress

- Selected first incomplete task: `P3-T04`.
- Latest commit is `[P3-T03R] Review scoop test removal`; it does not mention unfinished work that supersedes P3-T04.
- Git status contains pre-existing unrelated changes outside this task (`.gitignore`, `CALLER_LOCATION.md`, `RTTI_REFINE.md`); they will be preserved and not included in the P3-T04 commit.
- Found the expected P3-T04 targets: `crates/scoopc/src/audit/mod.rs`, `crates/scoopc/src/audit/spec_coverage.rs`, `crates/scoopc/src/pipeline_gap_audit.rs`, `crates/scoopc/src/pipeline_user_visible_failure_policy.rs`, and their `lib.rs` `#[cfg(test)] mod` mount points.
- Deleted the four P3-T04 Rust audit files and removed their `lib.rs` test-only module mount points.
- Confirmed `crates/scoopc/src` has no remaining deleted audit module mount references, and `crates/scoopc/src/audit/**` has no files.
- Validation passed: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, and `python3 tools/run_fixtures.py` (1533 checks).
- Updated `TODO.md` to mark `P3-T04` as `[DONE]` and appended the completion record.
