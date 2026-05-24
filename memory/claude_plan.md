# Execution Plan

I will follow `TODO.md` as the authoritative task list and complete only the first incomplete task in this invocation.

1. Read `TODO.md` to identify the first heading that is not prefixed with `[DONE]`.
2. Check the current Git status and latest commit for context relevant to that task.
3. Inspect only the files needed for the selected task and determine the implementation scope.
4. Implement the task without changing unrelated code or using workarounds.
5. Run formatting, linting, relevant tests, then broader validation as required by the task.
6. Update `TODO.md` to mark the task `[DONE]` and record completion details; update this plan file at key milestones.
7. Commit all task-related changes with a descriptive message and stop without starting the next task.

## Current Task

First incomplete task: `P9-T09` (`P9 全包清场、文档同步与依赖审计`).

## Progress

- Confirmed `P9-T09` is the first incomplete task in `TODO.md`; `P9-T09R` remains next.
- Checked Git state and latest commit. Existing unrelated dirty files were present before task edits: `run_agent.sh` and untracked `PLUGIN_ABI.md`.
- Ran `cargo run -p scoop_tools -- dependency-gate`; it passed and reported the P9 crate classes and source-boundary checks.
- Audited direct stage/cone dependencies with `cargo tree --depth 1`; the current shape matches the P9 task records and dependency gate.
- Searched `crates/scoopc/src`; remaining stage references are facade/orchestration wrappers under the umbrella, not standalone stage implementation ownership.
- Updated README, `PIPELINE-CLEANUP.md`, `TODO.md`, and `TODO-7.md` for P9-T09 completion and the P9 post-split boundary freeze.
- Validation completed successfully: `cargo fmt`; `cargo clippy --all-targets -- -D warnings`; `cargo run -p scoop_tools -- dependency-gate`; `cargo build --workspace`; `cargo test --all --all-targets`; `git diff --check`.
