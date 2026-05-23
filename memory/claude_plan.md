# Execution Plan

Date: 2026-05-23

This file records the public execution plan and progress for the current invocation. It intentionally excludes private chain-of-thought.

## Plan

1. Read `TODO.md` and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check whether the latest commit mentions an unfinished issue directly relevant to that task.
3. Inspect the task's referenced code, fixtures, and validation requirements.
4. Implement the task completely, or add the minimum prerequisite task in `TODO.md` if a concrete blocker makes correct implementation impossible.
5. Run the task-specific validation and any broader required tests, addressing unscheduled failures according to the failure policy.
6. Mark the task `[DONE]` in `TODO.md` with a completion record, unless blocked by a newly scheduled prerequisite.
7. Commit all relevant changes with a task-specific message and stop without starting the next task.

## Progress

- Initialized the invocation plan before inspecting project task state.
- Identified the first incomplete task as `P9-T02`: extract `scoopc_ast` from `crates/scoopc/src/{ast,parser,syntax}` while preserving `scoopc` facade paths.
- Checked the latest commit summary: `[P9-T01R] Review stage split back edges`; it does not explicitly name an unfinished issue that preempts `P9-T02`.
- Created the new `scoopc_ast` package skeleton, moved `ast`, `parser`, and `syntax` sources into it, and switched `scoopc` to facade re-exports for those paths.
- Updated dependency gate paths for moved parser/syntax files and added `scoopc_ast` to base-only stage dependency checks.
- `cargo check --workspace`, `cargo fmt`, `cargo build --workspace`, and `cargo run -p scoop_tools -- dependency-gate` have passed after the split.
- Fixed the only cross-crate visibility regression found by tests by adding a public AST side-table snapshot accessor and updating the affected test.
- Full validation passed: `cargo test --all --all-targets`, `cargo clippy --all-targets -- -D warnings`, final `cargo build --workspace`, final `cargo run -p scoop_tools -- dependency-gate`, `cargo tree -p scoopc_ast`, and `git diff --check`.
- Marked `P9-T02` as `[DONE]` in `TODO.md` and `TODO-7.md` with a completion record.
