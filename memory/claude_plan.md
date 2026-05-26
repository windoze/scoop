# Execution Plan

I will follow the repository's TODO-driven workflow and complete exactly one task.

1. Inspect `TODO.md` to identify the first task whose heading is not prefixed with `[DONE]`.
2. Review the selected task's requirements, dependencies, validation instructions, and any relevant recent commit context.
3. Inspect the smallest necessary set of source, fixture, documentation, and test files related to that task.
4. Implement the task as specified, without changing unrelated behavior or using workaround representations.
5. Run formatting and validation in the required order: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, then the relevant/full tests and fixture suite required by the task.
6. If validation reveals unscheduled failures, fix them if in scope or add the minimum prerequisite task to `TODO.md`, commit that scheduling change, and stop.
7. When the task is complete, update `TODO.md` by prefixing the task heading with `[DONE]` and filling in the completion record; update `PLAN.md` only if phase-level sequencing changed.
8. Commit all task-related changes with a clear message and the required co-authored-by trailer.
9. Stop after this one task.

Progress log:
- Created initial plan before inspecting project tasks.

## Current Task

First incomplete task: `P0-T04R` — review the fixture-runner self-test fixture deletion from `P0-T04`.

Planned review steps:
1. Inspect the latest commit's file changes to confirm the intended four self-test fixtures and dependent golden outputs were removed.
2. Verify no remaining `.scoop` fixture under `tests/fixtures/` expects `scoop::fixtures::*` error codes.
3. Verify no relevant source special cases remain for the removed self-test fixture names or runner-only error codes.
4. Run the required validation sequence: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, and `cargo run -p scoop -- test`.
5. Confirm the fixture suite reports 1532 checks, matching the documented drop from the prior 1536-check baseline after deleting four fixtures.
6. If all review criteria pass, mark `P0-T04R` as `[DONE]` in `TODO.md`, add the completion record, commit the review, and stop.

Progress log:
- Identified `P0-T04R` as the first incomplete task.
- Confirmed the latest commit is the relevant `P0-T04` deletion commit.
- Confirmed `tests/fixtures/**/*.scoop` has no remaining `scoop::fixtures::` expectations.
- Confirmed the deleted fixture filenames are absent from fixture files; remaining `scoop::fixtures::` occurrences are internal runner diagnostics/tests that are still expected before P3 removes the old runner.
- Starting required validation sequence.

Progress log:
- Validation sequence passed: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, and `cargo run -p scoop -- test`.
- Fixture runner reported `fixtures: ok (1532)`, matching the expected four-check drop from the prior 1536 baseline.
- Next step: update `TODO.md` to mark `P0-T04R` done and record validation evidence.

Progress log:
- Marked `P0-T04R` as `[DONE]` in `TODO.md` and added its completion record.
- No phase-level `PLAN.md` update is needed because the review did not change sequencing, dependencies, or completion criteria.
- Next step: inspect the final diff and commit the task review changes.
