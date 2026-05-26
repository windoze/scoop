# P2-T01 execution plan

## Current task

First incomplete task in `TODO.md`: **P2-T01** — switch `.github/workflows/ci.yml` from old test-infrastructure entrypoints to the new Python tools.

Required changes:

- Replace `cargo run -p scoop_tools -- spec-fixtures check` with `python3 tools/spec_fixtures.py check`.
- Replace `cargo run -p scoop -- test` with `python3 tools/run_fixtures.py`.
- Keep the change limited to CI wiring unless validation exposes a task-blocking issue.

## Step-by-step plan

1. Inspect `.github/workflows/ci.yml` and confirm the old CI entrypoints.
2. Edit only the CI workflow to call the new Python scripts.
3. Search the workflow for old `scoop_tools`, `scoop test`, and `cargo run -p scoop -- test` entrypoints.
4. Run repository validation in the requested order where meaningful for this YAML-only task:
   - `cargo fmt`
   - `cargo clippy --all-targets -- -D warnings`
   - targeted new CI commands: `python3 tools/spec_fixtures.py check` and `python3 tools/run_fixtures.py`
   - `cargo test --all --all-targets`
5. If validation exposes an unscheduled failure, fix it if directly in scope or add the minimum prerequisite task to `TODO.md`, commit, and stop.
6. If validation passes, update `TODO.md` by prefixing P2-T01 with `[DONE]` and appending a completion record.
7. Review the diff and commit all task-related changes with a `[P2-T01]` message and required co-author trailer.

## Progress

- Identified P2-T01 as the first incomplete task.
- Confirmed `.github/workflows/ci.yml` currently uses the old `scoop_tools` spec fixture command and old `scoop test` fixture runner command.
- Updated `.github/workflows/ci.yml` so CI calls `python3 tools/spec_fixtures.py check` and `python3 tools/run_fixtures.py`.
- Verified the workflow no longer references old fixture entrypoints.
- Validation passed: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, `python3 tools/spec_fixtures.py check`, and `python3 tools/run_fixtures.py` (`fixtures: ok (1533)`).
