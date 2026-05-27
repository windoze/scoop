# Execution Plan

I will maintain this file as a progress log and plan for the current invocation. I will not include private chain-of-thought; this records the actionable plan and milestone updates.

1. Read TODO.md to identify the first incomplete task by the [DONE] prefix rule.
2. Check the latest commit only for directly relevant unfinished work tied to that task.
3. Inspect the files and tests relevant to that task.
4. Implement the task exactly as specified, avoiding workarounds or spec deviations.
5. Run formatting, linting, targeted validation, then broader validation required by TODO.md.
6. If validation exposes unscheduled failures, fix them or add the minimum prerequisite task(s) in TODO.md before marking completion.
7. Mark the task [DONE] in TODO.md and update its completion record.
8. Commit all task-related changes with the required co-author trailer, then stop.

## Progress

- Created initial execution plan.

## Current Task

- First incomplete task identified: P1-T02R Review P1-T02 `@Inline` deletion results.
- Next: inspect the review task body, latest commit, and the P1-T02 changes to verify the removal is complete.

## Review Audit Started

- Auditing required P1-T02R surfaces for remaining `Inline` / `@Inline` active support.
- Checking lowercase `inline` removed-keyword diagnostics separately from removed `@Inline` annotation support.

## Audit Finding

- Required active surfaces no longer expose `annotation class Inline`, `BuiltinAnnotationKind::Inline`, or inline-specific checker names.
- Found active `umb_fix` fixture metadata still referencing the removed `#51-inline` / `#52-non-local-return` spec anchors; checking and correcting if stale.

## Plan Update

- Correct stale active fixture metadata that referenced removed `inline` spec anchors.
- Keep lowercase `inline` removed-keyword diagnostic and `@Inline` negative fixture unchanged.

## Validation Started

- Stale spec anchors corrected; running targeted searches, formatting, linting, spec fixture check, and fixture suite.

## Validation Progress

- `cargo fmt --all` completed.
- `cargo clippy --all-targets -- -D warnings` completed.
- Targeted searches found no remaining removed Inline built-in/sysroot/typecheck hooks and no stale `#51-inline` references.

## Validation Completed

- `python3 tools/spec_fixtures.py check` completed with `spec fixtures: ok (1)`.
- `python3 tools/run_fixtures.py` completed successfully.
- Next: update TODO completion records and commit.

## Completion Documentation

- Marked P1-T02R `[DONE]` in TODO.md and TODO-1.md.
- Recorded the review fix, validation results, and no-PLAN-update decision in TODO-1.md.
- Exact fixture suite summary: `fixtures: ok (1534)`.

## Commit

- Committing P1-T02R review completion and fixture metadata correction.
