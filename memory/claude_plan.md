# Execution Plan

## Scope

- Work from `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that one task, then stop.
- Do not perform broad historical triage before selecting the current task.

## Planned Steps

1. Read `TODO.md` to find the first incomplete task and its validation requirements.
2. Inspect only the files and recent context needed for that task, including the latest commit if it explicitly affects the selected task.
3. Implement the task directly unless a concrete prerequisite or blocker makes correct implementation impossible.
4. If a prerequisite blocker is found, update `TODO.md` with the minimum prerequisite task, keep the current task incomplete, commit the bookkeeping change, and stop.
5. Run the task-specific tests first, then broader relevant validation required by the task.
6. Fix failures that are in scope for the current task rather than working around them.
7. Mark the completed task heading in `TODO.md` with `[DONE]` and update its completion record.
8. Update this plan file as key steps complete or if the plan changes.
9. Inspect git status and diff, then commit all intended changes with a task-specific commit message.
10. Stop without starting the next task.

## Progress Log

- Initial execution plan recorded before inspecting `TODO.md`.
- Identified first incomplete task: `P7-A1：B-16 控制流 outside-of-context 早拒`.
- Next step: lock the B-16 UMB IDs and inspect only the frontend/typecheck/codegen paths needed for `break`, `continue`, and `return` outside valid contexts.
- Locked B-16 IDs: `UMB-0187`, `UMB-0188`, `UMB-0191`, `UMB-0192`, `UMB-0786`, `UMB-1263`, `UMB-1264`.
- Confirmed existing frontend/typecheck gates: `BreakNotInLoop`, `ContinueNotInLoop`, and `ReturnNotInFunctionBody` reject the relevant illegal contexts before MIR/LLVM lowering.
- Editing plan: replace the seven B-16 `UnsupportedMainBody` constructors with upstream-gated unreachable invariants, then retire the seven IDs in inventory/ledger and activate the B-16 fixtures.
- Implemented P7-A1: removed all seven B-16 `UnsupportedMainBody` constructors, retired the IDs in `audit/UMB_retired.csv`, set B-16 active inventory to 0, activated B-16 fixtures, and updated TODO completion records.
- Validation completed: B-16 list entries 0; UMB diff/stats in sync with active 1,277 and retired 7; audit tests, failure-policy tests, B-16 fixture suite, and clippy all passed.
- Next step: inspect git status/diff and commit the P7-A1 changes, then stop.
