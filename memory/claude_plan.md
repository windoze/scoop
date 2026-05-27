# Execution Plan

I will follow the task list in `TODO.md` and complete exactly the first incomplete task, stopping after committing that task.

Selected task: `P0-T01` — establish the old surface / sysroot / fixture migration inventory. This is a documentation-only baseline task; it should not change compiler behavior.

1. Read `TODO.md` to identify the first heading not prefixed with `[DONE]`, and check the latest commit only for directly relevant unfinished work.
2. Inspect the code, tests, fixtures, and task details needed for that task.
3. Implement the required changes without workarounds or unrelated edits.
4. Run formatting, linting, tests, and fixtures required by the task and repository policy.
5. If validation exposes an unscheduled failure, either fix it or add the minimum prerequisite task before marking the current task done.
6. Update `TODO.md` completion status and record. Update `PLAN.md` only if phase-level sequencing changes.
7. Commit all task-related changes with a descriptive message, then stop.

Progress:
- Identified `P0-T01` as the first incomplete task in both `TODO.md` and `TODO-1.md`.
- Confirmed the latest commit is a plan update and does not introduce a directly relevant unfinished implementation issue for this inventory task.
- Gathered required hits for `perform`, handler `with`, tuple `._0` / `._1`, f-string `{...}` / escaped braces, `@Inline`, `AnyRef` / `AnyValue`, implicit-public sysroot/API declarations, operator-like functions without `operator`, and the requested fixture globs.
- Wrote the inventory into `TODO-1.md`, marked `P0-T01` done in both `TODO.md` and `TODO-1.md`, and recorded validation results.
- Validation passed: `python3 tools/spec_fixtures.py check` and `python3 tools/run_fixtures.py`.
- Next step: commit the completed `P0-T01` documentation changes and stop.
