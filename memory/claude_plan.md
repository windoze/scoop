# Claude Plan

## Initial execution plan

1. Read TODO.md to identify the first task whose heading is not prefixed with [DONE].
2. Read the selected task details, related PLAN.md context if needed, and the latest commit only if it explicitly mentions unfinished work relevant to that task.
3. Inspect the smallest relevant code and test areas for that task.
4. Implement the task exactly as specified, adding or updating tests/fixtures where required.
5. Run formatting, clippy, tests, and fixtures as required by the task and repository policy.
6. Update TODO.md by prefixing the completed task with [DONE] and filling its completion record; update PLAN.md only if phase-level sequencing changes.
7. Commit all task-related changes with a clear task-tagged message and stop without starting the next task.

## Progress

- Initial plan recorded before task execution.
- Selected first incomplete task: P1-T03R, reviewing `tools/fixtures_matrix.py` against the old Rust `scoop_tools fixtures-matrix` implementation.
- Read TODO/PLAN context, latest commit, Python implementation, old Rust implementation, and old CLI dispatch. Current worktree has this plan update plus an unrelated untracked `RTTI_REFINE.md`, which will be left untouched.
- Parity review found no implementation changes needed: current-repo `check`/`stdlib` output matches the old Rust tool exactly, and focused temporary inputs match for duplicate chapters, repeated comment prefixes, missing fixtures, stdlib domains, and invalid unclosed fences.
- Validation completed: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, `python3 tools/run_fixtures.py tests/fixtures`, and `cargo run -p scoop -- test` all passed. `TODO.md` has been updated to mark P1-T03R done.
