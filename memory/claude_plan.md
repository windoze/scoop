# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Select and complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Do not proceed to later tasks after completing or blocking the selected task.

## Steps

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check recent git context only as needed for direct relevance to that task.
3. Inspect the implementation areas and tests related to the selected task.
4. Make the smallest spec-correct changes needed to complete the task, without workarounds.
5. Run formatting and linting first, then task-relevant tests, then the required full suites if code changed.
6. If a concrete blocker or unscheduled failing test is found, update `TODO.md` with the minimum prerequisite task and stop after committing that bookkeeping.
7. If the task is completed, mark its heading `[DONE]`, update its completion record, and commit all relevant changes.

## Progress Log

- Started by recording this execution plan before implementation work.
- Identified first incomplete task: `P1-T02` in `TODO-1.md`.
- Latest commit is `[P1-T01R] Review GC pacing core`; it is the direct predecessor and does not add an unfinished blocker for `P1-T02`.
- Confirmed current pacing core still hardcodes growth factor/min threshold and only has a partial `SCOOP_GC_PACING=off` helper.
- Implementation direction: parse pacing config once in `scoop_runtime_init`, bypass pacing when GC stress is enabled, and let GC backends read the initialized config.
- Implemented initial code/test changes: runtime env parsing, configurable Immix target calculation, and subprocess-based heap-growth regressions for on/off/env/stress behavior.
- Full Rust tests initially failed because internal pacing getters became exported runtime symbols. Adjusted design to store parsed pacing config in `ScoopGcHeap` fields instead, avoiding public ABI expansion.
- Validation completed successfully: targeted pacing env test, clippy, full Rust test suite, 10M heap-growth on/off metrics, hosted/minimal backend runtime tests, and full fixture suite.
- Updated `TODO.md` and `TODO-1.md` to mark `P1-T02` as `[DONE]` with completion details. `PLAN.md` was not changed because phase-level sequencing did not change.
