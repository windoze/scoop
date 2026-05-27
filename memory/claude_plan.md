# Execution Plan

## Goal
Complete exactly the first incomplete task listed in TODO.md, following TODO.md as the authoritative ordering and stopping after one task is completed, documented, validated, and committed.

## Step-by-step plan
1. Read TODO.md to identify the first task whose heading is not explicitly prefixed with [DONE].
2. Check the latest commit message for any unfinished issue directly relevant to that selected task.
3. Inspect only the files and documentation needed for that task; avoid broad unrelated triage.
4. Implement the task as specified, adding prerequisite TODO entries only if a concrete blocker makes spec-correct implementation impossible.
5. Run formatting, linting, tests, and fixtures required by the task and repository policy, addressing any unscheduled failures.
6. Update TODO.md by prefixing the completed task title with [DONE] and filling its completion record; update PLAN.md only if phase-level planning changed.
7. Commit all changes for this task with a clear task-tagged message and the required co-author trailer, then stop.

## Progress
- Plan initialized before task execution.
- Selected first incomplete task: P3-T02, removing the `scoopc test-fixtures` CLI surface and module exports.
- Inspection found the `scoopc` runtime code path already absent from `driver_cli.rs`, `bin/scoopc.rs`, and `lib.rs`; remaining work is to add explicit parser regression coverage, validate the unknown-subcommand behavior, and update TODO bookkeeping.
- Added `driver_cli::tests::test_fixtures_subcommand_is_removed`.
- Validation: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test -p scoopc --all-targets`, direct `cargo run -q -p scoopc -- test-fixtures`, and `python3 tools/run_fixtures.py` passed for the P3-T02 boundary.
- Full `cargo test --all --all-targets` stops in two `scoop test` wrapper integration tests; the exact failures were added to P3-T03 because deleting that wrapper is the next scheduled task.
