# Claude Execution Plan

## Current Invocation

Goal: complete exactly the first incomplete detailed task from the TODO task files, then commit and stop.

## Plan

1. Read `TODO.md` as the task index.
2. Open the referenced detailed `TODO-Px.md` files in task order and identify the first task whose detailed heading is not prefixed with `[DONE]`.
3. Check the latest commit message only for directly relevant unfinished work tied to that selected task.
4. Inspect the code and tests needed for that task, avoiding unrelated historical triage.
5. Implement the smallest spec-correct change that fully satisfies the selected task.
6. Add or update tests/fixtures required by the task.
7. Run relevant validation commands, expanding to broader test commands if needed.
8. If the task is completed, mark its heading `[DONE]` in the authoritative `TODO-Px.md` file, update its completion record, and sync `TODO.md` if the indexed title/status appears there.
9. If a concrete blocker prevents completion, add the minimum prerequisite task in the correct detailed TODO file, sync `TODO.md`, record the blocker here, commit, and stop.
10. Commit all relevant changes for this invocation with a descriptive task-tagged commit message.
11. Stop without starting the next task.

## Progress Log

- Initialized plan before reading task files or running repository commands.
- Read `TODO.md` and `TODO-P7.md`; selected first incomplete detailed task `P7-T02Za`.
- Latest commit `[P7-T02Z] Fix hidden init blockers and add dispatch prerequisite` is directly relevant because it introduced the current prerequisite, so this invocation will implement `P7-T02Za` rather than continuing broader `P7-T02Z` work.

## Selected Task

`P7-T02Za`: close dynamic dispatch ABI schema identity drift blocking hidden suspend virtual/interface helper fixtures.

## Execution Notes

- Focus only on the virtual/interface hidden suspend dynamic dispatch blocker and directly related schema identity mapping issues.
- Do not downgrade source-level dynamic dispatch to direct calls, even for single-candidate dispatch.
- Preserve refactor default path semantics; do not add legacy fallback, fixture special-cases, or verifier weakening.
- Reproduced the target failures. Interface path now runs successfully after mapping carrier shells to canonical dynamic call-surface `Step` and keeping ABI-owned resume/surface generation separate from body-program ids.
- Virtual path now emits concrete candidate bodies and projected carrier shells, but still needs the synthetic dynamic call-surface resume symbol (`k8`) to be published instead of remaining as a declaration.
- Implemented dynamic call-surface resume adapters, validated the virtual/interface/member/object-property hidden suspend fixtures, added a default-refactor integration test, ran relevant scoopc tests and clippy, and marked `P7-T02Za` `[DONE]` in `TODO-P7.md` / `TODO.md`.
