# Execution Plan

I will follow the project task workflow without exposing private reasoning details.

1. Read `TODO.md` to identify the first task whose heading is not prefixed with `[DONE]`.
2. Read only the directly relevant project files for that task, plus `PLAN.md` if phase-level context is needed.
3. Implement the task exactly as specified, avoiding workarounds or spec deviations.
4. Run formatting, linting, and relevant validation in the required order; address any unscheduled failures that appear.
5. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and recording completion details.
6. Update this plan file at key milestones or if the approach changes.
7. Commit all task-related changes with a descriptive message and the required co-author trailer.
8. Stop after this one task.

## Milestone: Selected task

Selected first incomplete task: `P1-T01`, implementing `tools/run_fixtures.py` as the standalone Python fixture runner. The runner will port target discovery and directive parsing, then call only public `scoopc`/`scoop` command surfaces. Multi-file and multi-cone frontend cases will be materialized as temporary cone projects so `scoopc check-source` can validate selected sources without using legacy fixture APIs.

## Milestone: Diagnostic blocker

Validation found that `scoopc check-source` returns stable diagnostic codes but does not render primary label locations in stderr. Because `tools/run_fixtures.py` must honor `EXPECT-ERROR-AT` without calling legacy fixture APIs, I will update the public `check-source` command surface to append a stable `location: <line>:<col>` note when a checked source diagnostic carries labels.

## Milestone: Task complete

Implemented `tools/run_fixtures.py`, fixed the public command surfaces needed by the runner, validated the new runner over `tests/fixtures` with 1532 checks, and confirmed the legacy runner still reports 1532 checks. Final validation passed with `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, `python3 tools/run_fixtures.py tests/fixtures`, and `cargo run -p scoop -- test`. I will now commit the P1-T01 changes and stop.

## Current invocation: P1-T01R review

Selected first incomplete task: `P1-T01R`, reviewing `tools/run_fixtures.py` against the legacy `scoop test` / `scoopc test-fixtures` runner for equivalent pass/fail target sets and check counts.

Execution plan:

1. Inspect the new Python runner and the legacy Rust fixture runner paths that define target planning, expectation parsing, command execution, and summary reporting.
2. Run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, the Rust test suite, the new Python fixture suite, and the legacy fixture suite in the required order.
3. Compare the new and legacy fixture outputs for total checks and any pass/fail target differences.
4. If review finds a real equivalence bug, fix it and rerun the affected validations; if it finds an unscheduled blocking failure, update `TODO.md` with the minimum prerequisite instead of marking this task done.
5. When equivalence is confirmed, mark `P1-T01R` `[DONE]`, record validation details in `TODO.md`, commit the task changes, and stop.

## Milestone: P1-T01R validated

Completed the review of `tools/run_fixtures.py` against the legacy Rust fixture runner. Static comparison found no target planning, expectation parsing, phase execution, environment, timeout, golden comparison, or summary/check-count discrepancy. Validation passed with `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, `python3 tools/run_fixtures.py tests/fixtures`, and `cargo run -p scoop -- test`. The new and legacy fixture logs both reported 1503 passing targets, 0 failures, and 1532 checks; sorted target/status/check lists matched exactly. I will commit the P1-T01R review record and stop.
