# Execution Plan

I will follow TODO.md as the source of truth and complete exactly the first incomplete task.

1. Read TODO.md to find the first task whose heading is not prefixed with [DONE].
2. Check the latest commit message only for directly relevant unfinished work tied to that selected task.
3. Inspect the task requirements, dependencies, and validation instructions.
4. Make the smallest complete implementation needed for that task without working around spec gaps.
5. Run formatting, linting, tests, and fixture validation as required by the task and repository policy.
6. If a blocking prerequisite or unscheduled failing test is found, update TODO.md with the minimum necessary task ordering change, commit that, and stop.
7. If the task is completed, mark its TODO.md heading with [DONE], update its completion record, commit all relevant changes, and stop without starting the next task.

Progress:
- Plan initialized.
- Identified first incomplete task: P1-T08, porting `crates/scoopc/src/pipeline_user_visible_failure_policy.rs` to `tools/audit_user_visible_failure_policy.py`.
- Next steps: inspect the existing Rust audit and neighboring Python audit ports, implement the Python script with equivalent checks/output, validate it against the Rust test, update TODO.md completion records, and commit only the completed task changes.
- Implemented `tools/audit_user_visible_failure_policy.py` with the same audit file set, frontend reject surfaces, upstream guard records, stale unsupported marker guard, no-production-`todo!` check, and exact internal bug sentinel baseline as the Rust audit.
- Targeted validation passed: `python3 -m py_compile tools/audit_user_visible_failure_policy.py`, `python3 tools/audit_user_visible_failure_policy.py`, and `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`.
- Next step: run repository formatting/linting/full validation, then update TODO.md and commit P1-T08.
- Full validation passed: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, `python3 tools/run_fixtures.py tests/fixtures`, and `cargo run -p scoop -- test`.
- Marked P1-T08 as `[DONE]` in TODO.md and appended its completion record.
- Committed P1-T08 changes in Git.
