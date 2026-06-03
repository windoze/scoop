# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that task, validate it, update bookkeeping, commit, and stop.

## Execution Plan

1. Read `TODO.md` first and identify the first incomplete task by heading prefix.
2. Check the latest commit only for directly relevant unfinished work tied to that selected task.
3. Inspect the files and tests relevant to the selected task, avoiding broad unrelated triage.
4. Implement the smallest spec-correct change needed for the selected task.
5. Update or add focused tests/fixtures that validate the task requirements.
6. Run `cargo fmt`, then `cargo clippy --all-targets -- -D warnings`.
7. Run the relevant tests first, then the full Rust and fixture suites if code changed and prior successful results cannot be reused.
8. If any unscheduled test or fixture failure appears, fix it if in scope or add the minimum prerequisite/follow-up task in `TODO.md` before marking the current task done.
9. Mark the completed task heading in `TODO.md` with `[DONE]` and update its completion record.
10. Commit all intended changes with a task-tagged message, then stop without starting the next task.

## Progress Log

- Created the initial execution plan before inspecting project files or running commands.
- Read `TODO.md`; selected `T2-03: 迁移跨引用字段到 LirCallableId` as the first incomplete task.
- Latest commit is `[T2-02-R]`; no directly relevant unfinished blocker was identified.
- Implementation approach: change LIR fact contract live callable references to local `LirCallableId`, with an explicit hash-backed reference for cross-cone/bodyless targets, then update builders, verifiers, codegen consumers, tests, and dumps.
- Implemented the callable-reference migration and updated affected verifier/codegen/test paths.
- Validation progress: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all --all-targets` passed.
- Completed full validation: `cargo build -p scoop -p scoopc`, `python3 tools/dependency_gate.py`, `python3 tools/spec_fixtures.py check`, and `python3 tools/run_fixtures.py` all passed.
- Marked `T2-03` as `[DONE]` in `TODO.md` and recorded completion details.
