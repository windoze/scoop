# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify and complete exactly the first incomplete task whose heading is not prefixed with `[DONE]`.
- Stop after completing that one task, recording completion and committing changes.

## Execution Plan

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Inspect the latest commit message only for directly relevant unfinished work tied to that task.
3. Read the relevant project files for the selected task, limiting investigation to the current task and blocking prerequisites.
4. Implement the selected task as written, without narrowing scope or using workarounds.
5. Add or update focused tests/fixtures required by the task.
6. Run formatting first with `cargo fmt`.
7. Run linting with `cargo clippy --all-targets -- -D warnings`.
8. Run the relevant tests first, then full validation required by the task; use long timeouts for full suites.
9. If unscheduled failures are observed, fix them if in scope or add the minimum prerequisite/follow-up task in `TODO.md` before marking anything complete.
10. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and adding a completion record.
11. Update this file when key steps complete or if the plan changes.
12. Inspect git status and diffs, then commit all intended changes with a task-specific message.
13. Stop without starting the next task.

## Progress Log

- Created initial execution plan before reading task files or running commands.
- Identified first incomplete task: `P2-T01` in `TODO-2.md`, covering nursery-full minor GC retry. Latest commit is `[P1-T02R] Review GC pacing env knobs`, with no explicit unfinished issue found in its subject.
- Inspected the Immix allocation path and minor GC implementation. The required code change is to trigger one minor collection after nursery allocation fails, retry nursery once, then fall back to old space. Minor GC also needs to count reclaimed dead nursery bytes so the required `bytes_freed` validation is meaningful.
- Implemented the nursery-full minor retry path and updated Immix runtime tests. Targeted tests passed: `gc_immix_nursery`, `gc_immix_write_barrier`, and `gc_immix_minor_collect`.
- Validation passed: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, `python3 tools/spec_fixtures.py check`, and `python3 tools/run_fixtures.py`.
- Updated `TODO.md` and `TODO-2.md` to mark `P2-T01` as `[DONE]` with implementation and validation notes. Only documentation changed after the successful full validation runs.
