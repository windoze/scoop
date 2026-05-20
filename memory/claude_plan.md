# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Stop after implementing, validating, documenting, and committing that one task.

## Execution Plan

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check the latest commit only for directly relevant unfinished work after the task is identified.
3. Inspect the smallest necessary code and fixture areas for the selected task.
4. Implement the task without narrowing scope or introducing workarounds.
5. Run the task-specific tests first, then broader required validation if specified by the task.
6. If a concrete blocking prerequisite is discovered, update `TODO.md` with the minimum prerequisite task, keep the current task incomplete, commit the bookkeeping change, and stop.
7. If the task is completed, mark its heading with `[DONE]`, update its completion record, and avoid changing `PLAN.md` unless phase-level sequencing changed.
8. Inspect git status and diff, then commit all intended changes with a task-specific message.

## Progress Log

- Initialized plan before reading `TODO.md`.
- Identified first incomplete task: `P1-T02R` in `TODO-2.md`.
- Latest commit subject is `[P1-T02] Migrate span and source base crates`; no unfinished issue is explicit in the visible subject, so the review proceeds against that migration.
- Current review will check duplicate authoritative definitions, base crate dependency direction, specified call surfaces, and P1-T02 validation commands.
- File review so far: authoritative `Span` is in `scoopc_span`; authoritative `SourceFile` / `SourceId` / `SourceMap` are in `scoopc_source`; `crates/scoopc/src/span.rs` and `crates/scoopc/src/source.rs` are re-export adapters.
- Next step: run P1-T02R validation commands and dependency-tree checks before updating TODO completion records.
- Validation passed: `cargo fmt`; `cargo test -p scoopc_span`; `cargo test -p scoopc_source`; `cargo test --all --all-targets --no-default-features`; `cargo clippy --all-targets -- -D warnings`; `cargo tree -p scoopc_source`; `cargo run -p scoop_tools -- dependency-gate`; authoritative-definition search.
- Next step: mark `P1-T02R` as `[DONE]` in both TODO indexes and record the review conclusion.
- Updated `TODO.md` and `TODO-2.md`: `P1-T02R` is marked `[DONE]` with review conclusion, validation commands, dependency direction, and residual risk.
- Next step: inspect git status/diff/log, run whitespace diff check, then commit the review bookkeeping.
- Pre-commit inspection completed: only `TODO.md`, `TODO-2.md`, and this progress file are modified; `git diff --check` passed.
- Next step: commit the completed `P1-T02R` review task and stop.
