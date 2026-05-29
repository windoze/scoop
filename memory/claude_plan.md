# Claude Execution Plan

## Scope
- Follow `TODO.md` as the authoritative task list.
- Identify and complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Stop after committing that one task.

## Plan
1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check recent git context only as needed for that task, especially the latest commit if it mentions an unfinished issue relevant to the selected task.
3. Inspect the relevant code, tests, fixtures, and documentation for the selected task.
4. Implement the smallest spec-correct change needed for the task, without introducing workarounds.
5. Run formatting, linting, targeted tests, and required full validation in the requested order.
6. If validation reveals an unscheduled failure, fix it or add the minimum prerequisite/follow-up task before marking completion.
7. Update `TODO.md` by prefixing the completed task title with `[DONE]` and filling its completion record.
8. Update this file with key progress changes.
9. Commit all intended changes with a task-scoped commit message, then stop.

## Progress
- Initial execution plan recorded before running project commands.
- Selected first incomplete task: `P2-T04` from `TODO-2.md`, requiring hosted/minimal backend pacing parity.
- Latest commit is `73aa4ed5 [P2-T03R] Review GC hard cap OOM paths`; it directly precedes and unblocks this task.
- Implemented shared pacing helper hooks in `scoop_gc.h` and wired hosted/minimal allocation registration plus safepoint polling to request and consume pacing collections.
- Added hosted/minimal coverage in `gc_pacing_env.rs` for default-on pacing, `PACING=off`, env threshold tuning, and stress bypass semantics.
- Validation passed: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, hosted/minimal/default pacing tests, hosted/minimal runtime tests, `cargo test --all --all-targets`, `python3 tools/spec_fixtures.py check`, and `python3 tools/run_fixtures.py`.
- Marked `P2-T04` as `[DONE]` in `TODO.md` and `TODO-2.md` with completion notes.
