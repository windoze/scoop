# Execution Plan

## Current task

P2-T01R: Review the CI switch result, confirming the workflow uses the new Python fixture/spec commands, no old fixture runner or `scoop_tools` entry remains in CI, and validation passes.

## Execution plan

1. [DONE] Inspect `TODO.md`, the latest commit, `.github/workflows/ci.yml`, and the relevant cleanup plan requirements.
2. [DONE] Search the CI workflow for old entries: `scoop_tools`, `cargo run -p scoop -- test`, `scoop test`, and `test-fixtures`.
3. [DONE] Run formatting and linting first, then the Rust test suite and the new CI fixture/spec commands required by P2-T01R.
4. [DONE] Fix any CI-switch issue or newly observed unscheduled failure instead of working around it. No issue was found.
5. [DONE] Mark P2-T01R `[DONE]` in `TODO.md` with a completion record, update this progress file, commit all task-related changes, and stop.

## Validation results

- Old CI entry search: no `scoop_tools`, `cargo run -p scoop -- test`, `scoop test`, or `test-fixtures` matches in `.github/workflows/ci.yml`.
- New CI entry search: `.github/workflows/ci.yml` uses `python3 tools/spec_fixtures.py check` and `python3 tools/run_fixtures.py`.
- `cargo fmt`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test --all --all-targets`: passed.
- `python3 tools/spec_fixtures.py check`: passed.
- `python3 tools/run_fixtures.py`: passed, 1533 checks.
