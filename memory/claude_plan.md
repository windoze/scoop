# Claude Execution Plan

## Scope

- Current invocation goal: complete exactly the first incomplete task in `TODO.md`, then stop.
- Source of truth: `TODO.md` for task order, dependencies, validation, and completion records.
- `PLAN.md` will only be changed if phase-level sequencing or completion criteria change.
- This file records the actionable plan and progress summary for auditability; it does not include private chain-of-thought.

## Step-by-Step Plan

1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Inspect the latest commit only for directly relevant unfinished work, if needed for that task.
3. Read the task body carefully, including dependencies, acceptance criteria, and validation commands.
4. Inspect the relevant code and fixtures for the selected task only; avoid broad unrelated triage.
5. Implement the task as written, using the smallest correct changes and no workaround or spec deviation.
6. If a concrete blocker or missing prerequisite prevents correct implementation, update `TODO.md` with the minimum prerequisite task in the correct order, leave the current task incomplete, commit, and stop.
7. Run targeted tests first, then any task-required validation; fix failures that are in scope.
8. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling/updating its completion record.
9. Run formatting or final validation relevant to the changed files.
10. Inspect `git status`, `git diff`, and recent log before committing.
11. Commit all intended changes with a descriptive task-tagged message.
12. Stop without starting the next task.

## Progress Log

- Initialized plan before reading repository task files or running commands.
- Identified first incomplete task from `TODO.md`: `P0-T03` in `TODO-1.md`.
- Explored `P0-T03` scope: old Scoop `const` surface spans lexer/parser/AST, resolver/typecheck `is_const` gates, deleted `comptime` evaluator module, HIR/MIR/codegen top-level `const val` side tables, sysroot/fixture references.
- Starting implementation with the language surface and evaluator removal, then using targeted compile/test feedback to finish cross-stage cleanup.
- Removed old const-eval fixture routes and deleted obsolete `tests/fixtures/comptime` and B-24 comptime/const fixtures; rewrote remaining sysroot/runtime reflection references to ordinary runtime intrinsic or top-level `val` surface.
- Validation completed: formatter, no-default scoopc stage tests, audit tests, fixture-runner tests, selected parse/typecheck/HIR/MIR/run-pass/build fixtures, strict clippy, and old-surface searches all passed after fixes.
- Marked `P0-T03` as `[DONE]` in `TODO.md` and `TODO-1.md` with completion record; `PLAN.md` did not need changes.
