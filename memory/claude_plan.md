# Execution Plan

I will follow the repository task order exactly and complete only the first incomplete task from `TODO.md`.

## Steps
1. Inspect `TODO.md` to find the first task whose heading is not prefixed with `[DONE]`.
2. Review the selected task's requirements, dependencies, validation commands, and any relevant latest-commit context.
3. Inspect only the code, fixtures, and documentation needed for that task.
4. Implement the task completely, or if a concrete blocker makes that impossible, add the minimum prerequisite task in `TODO.md` and stop.
5. Run required formatting, linting, tests, and fixtures according to the task and repository policy.
6. Update `TODO.md` with the `[DONE]` prefix and completion record only after the task is fully validated.
7. Commit all changes for this invocation with a descriptive message and the required co-author trailer.
8. Stop without starting the next task.

## Progress
- Initial plan recorded.

## Selected Task
- First incomplete task: P2-T04 — switch f-string interpolation from `{...}` to `${...}`.
- I will inspect the task-specific parser, spec, and fixtures before editing.

## Implementation Progress
- Updated parser f-string splitting so only `${...}` starts interpolation and bare braces remain text.
- Updated f-string text decoding so braces are no longer doubled/undoubled.

## Targeted Validation
- Refreshed `tests/fixtures/parse/f_string_interpolation.ast`.
- Targeted f-string parse, run-pass, codegen, and typecheck fixtures pass.

## Completion Record
- Marked P2-T04 `[DONE]` in `TODO.md` and `TODO-2.md`.
- Full validation passed: `cargo test --all --all-targets` and `python3 tools/run_fixtures.py` (`fixtures: ok (1538)`).
- Next step is committing this invocation only; no next TODO task will be started.

## Final Documentation Check
- Re-ran `python3 tools/spec_fixtures.py check` after final spec wording edits; it passed.
