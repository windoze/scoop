# Execution Plan

I will follow `TODO.md` as the source of truth, complete exactly the first incomplete task, commit it, and stop. This file records the execution plan and progress without private reasoning.

Selected task: `P0-T02` — establish overload bug and diagnostics baseline samples.

1. Confirm the first incomplete task in `TODO.md` and read the detailed `P0-T02` entry in `TODO-1.md`.
2. Check the latest commit for directly relevant unfinished work that affects `P0-T02`.
3. Inspect the existing overload tests, fixtures, diagnostics conventions, and any baseline/audit files referenced by the task.
4. Add or update the minimum required baseline samples for overload bugs and diagnostics, following existing fixture organization and expected-output conventions.
5. Run formatting and the validation required by the task, escalating to broader validation if code changes or observed failures require it.
6. Mark `P0-T02` as `[DONE]` in both `TODO.md` and `TODO-1.md`, with a completion record that includes the meaningful changes and validation results.
7. Commit all task-related changes with a descriptive `P0-T02` commit message, including this progress file if it remains modified, then stop.

Progress:
- Identified `P0-T02` as the first incomplete task from `TODO.md`.
- Read `P0-T02`, `PLAN.md` P0, `OVERLOAD_RESOLUTION.md` sections 1 and 10, the fixture runner expectation parser, and the user-visible failure audit forbidden terms.
- Confirmed the fixture runner has `EXPECT: fail` for negative diagnostics and `IGNORE-UNTIL-FIX` for current-failing target-pass fixtures, so the overload bug baselines can live in `tests/fixtures/run-pass` without accepting backend/codegen failures as final expected behavior.
- Added skipped run-pass baseline fixtures for `overload_concrete_bug`, `overload_arity_bug`, and `overload_gvc_ok` with their final expected exit codes for P5-T04 to enable.
- Updated the P5-T04 TODO entry to reference enabling those P0-T02 fixture files instead of recreating the samples later.
- Validation completed: targeted overload fixture runs skip as intended, new files parse, the current failures were reproduced, `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, and `python3 tools/run_fixtures.py` all passed.
- Marked `P0-T02` `[DONE]` in `TODO.md` and `TODO-1.md` with the completion record.
