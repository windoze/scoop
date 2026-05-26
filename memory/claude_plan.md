# Current Invocation Plan

## Scope
- Use `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that one task, then stop.

## Execution Steps
1. Read `TODO.md` to find the first incomplete task and its requirements, dependencies, and validation instructions.
2. Check the latest commit message only for unfinished work directly relevant to that selected task.
3. Inspect the code, fixtures, and documentation directly related to the selected task.
4. Implement the task without changing unrelated behavior or using workarounds.
5. Run formatting, linting, and task-relevant validation in the required order; run broader suites when code changes require them.
6. If validation exposes an unscheduled failing test or fixture, either fix it or add the minimum prerequisite/follow-up task to `TODO.md` before marking the task complete.
7. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling its completion record. Update `PLAN.md` only if phase-level sequencing changes.
8. Update this file at key milestones or if the plan changes.
9. Commit all changes for this task with a clear task-tagged message and the required co-author trailer.

## Notes
- This file records the actionable plan and progress notes, not private reasoning.

## Progress
- Identified first incomplete task: P1-T06 (`tools/audit_spec_coverage.py` port from `crates/scoopc/src/audit/spec_coverage.rs`).
- Next: inspect Rust audit semantics and existing Python tool conventions before implementing.
- Focused Rust baseline passed: 7 spec coverage audit tests.
- Implementing `tools/audit_spec_coverage.py` as a standalone Python standard-library audit.
- Implemented `tools/audit_spec_coverage.py`; running py_compile, script audit, and focused Rust audit parity validation.
- Focused validation passed; running `cargo fmt` followed by `cargo clippy --all-targets -- -D warnings`.
- Formatting and clippy passed; running full Rust test suite with `cargo test --all --all-targets`.
- Full Rust test suite passed; running full fixture suite with `python3 tools/run_fixtures.py tests/fixtures`.
- Python fixture suite passed with 1533 checks; running legacy `cargo run -p scoop -- test` parity suite.
- Legacy fixture suite passed with 1533 checks.
- Updated `TODO.md` to mark P1-T06 `[DONE]` and added the completion record.
- Next step: commit P1-T06 changes only, leaving unrelated untracked files untouched.
