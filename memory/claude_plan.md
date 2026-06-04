# Claude Execution Plan

## Status

- Current invocation started on 2026-06-04.
- `TODO.md` has been read.
- First incomplete task identified: `T2-07-R：Review T2-07`.
- Latest commit is `[T2-07] Define total LIR instruction set`; it does not mention an unfinished issue.
- Review finding: `LirMemberAccessMetadata` still models `resolved` as `Option<LirMemberTarget>`, allowing unresolved member references to be represented in LIR. This is in scope for T2-07-R because T2-07 requires body references to be handle-based and forbids unresolved/placeholder states in LIR instruction definitions.
- Fix applied: `LirMemberAccessMetadata.resolved` is now a required `LirMemberTarget`; a unit test constructs member access metadata with an explicit handle.
- Validation passed: `cargo fmt`; `cargo clippy --all-targets -- -D warnings`; `cargo test --all --all-targets`; `cargo build -p scoop -p scoopc`; `python3 tools/dependency_gate.py`; `python3 tools/spec_fixtures.py check`; `python3 tools/run_fixtures.py`.
- `TODO.md` has been updated to mark `T2-07-R` as `[DONE]` and record the review finding/fix.
- Git status/diff/log have been inspected; modified files are `TODO.md`, `crates/scoopc_lir/src/effect_lowered/instruction.rs`, and `memory/claude_plan.md`.
- Next step is to commit these T2-07-R changes.

## Execution Plan

1. Read `TODO.md` first and identify the first incomplete task exactly as ordered there.
2. Check the latest commit message only for an explicitly unfinished issue directly relevant to that selected task.
3. Read the selected task body, validation requirements, dependencies, and completion-record expectations.
4. Inspect only the files and code paths relevant to that task.
5. Implement the task as written, without narrowing scope or introducing workaround behavior.
6. If a concrete prerequisite or spec mismatch blocks correct implementation, update `TODO.md` with the minimum prerequisite task in dependency order, record the blocker here, commit that bookkeeping, and stop.
7. Run formatting first with `cargo fmt` if Rust/code changes are made.
8. Run linting with `cargo clippy --all-targets -- -D warnings` when applicable.
9. Run the relevant tests and fixtures required by the task. If full validation is required, run `cargo test --all --all-targets` and `python3 tools/run_fixtures.py` with long timeouts.
10. Fix any unscheduled failing test or fixture, or add the minimum explicit follow-up/prerequisite task in `TODO.md` before completion.
11. Mark the task heading in `TODO.md` with `[DONE]` and update its completion record only after implementation and required validation are complete.
12. Update this plan file when key steps complete or if the plan changes.
13. Inspect git status and diff, then commit all changes required for this task with a descriptive task-tagged commit message.
14. Stop after completing and committing exactly one task.

## Current Notes

- This file contains an execution plan and progress log, not hidden chain-of-thought reasoning.
- `PLAN.md` will only be updated if phase-level sequencing, dependencies, assumptions, or completion criteria change.
- Current task is a review task; if review finds a task-scoped defect, fix it before marking T2-07-R done.
