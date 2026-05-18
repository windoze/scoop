# Claude Execution Plan

## Scope

Execute exactly the first incomplete task in `TODO.md`, then stop after committing the completed task or any required dependency/task-list update.

## Reasoning Summary

`TODO.md` is the authoritative task source. I will not perform broad historical triage before selecting the first incomplete task. If a blocker directly affects that task, I will either fix it as part of the task or add the minimum prerequisite task before it and stop after committing that bookkeeping change.

## Step-by-Step Plan

1. Read `TODO.md` and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit message only for unfinished issue context directly relevant to that selected task.
3. Read the selected task details, dependencies, validation requirements, and relevant project files.
4. Implement the selected task as written without narrowing scope or using workarounds.
5. Add or update the smallest relevant tests/fixtures needed for the specified behavior.
6. Run focused validation first, then broader validation required by the task and repository guidelines as feasible.
7. If validation exposes an in-scope defect, fix the root cause and rerun relevant validation.
8. If a concrete prerequisite blocks correct completion, update `TODO.md` with the minimum prerequisite task, update this plan file, commit the task-list change, and stop.
9. On success, mark the selected task heading with `[DONE]` in `TODO.md` and update its completion record.
10. Update `PLAN.md` only if the phase-level plan, dependency structure, assumptions, or completion criteria changed.
11. Inspect the working tree and diff, then commit all intended changes with a clear task-tagged message.
12. Stop without starting the next task.

## Progress Log

- Created initial execution plan before running project commands.
- Identified first incomplete task: `P7-B2.6` for B-19/B-20/B-22/B-23 layout and member contract retirement. Latest commit is `[P7-B2.5] Retire scalar literal UMB rows`, with no directly blocking unfinished issue in the subject.
- Implemented the first verifier/codegen pass for P7-B2.6: materialized MIR now validates class constructor, top-level store, enum payload, member target, and dispatch metadata contracts; matching LLVM fallback sites were converted to internal invariants. Focused `cargo test -p scoopc mir::materialize -- --nocapture` passed after tightening generic-owner handling.
- Synchronized audit data for P7-B2.6: active inventory is now 652, retired ledger is 632, and B-19/B-20/B-22/B-23 active counts are 0. Activated the relevant fixture directories and fixed fixture expectations for B-19/B-20/B-22/B-23 smoke/negative coverage.
