# Claude Execution Plan

## Boundaries

- Work from `TODO.md` as the authoritative task list.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`, then stop.
- Do not perform broad historical issue triage before identifying the current task.
- If a blocking implementation/spec issue prevents the current task, add the minimum prerequisite task to `TODO.md`, commit that bookkeeping, and stop.
- Update `PLAN.md` only if phase-level sequencing, dependencies, assumptions, or completion criteria change.
- Commit the completed task or blocker bookkeeping before stopping.

## Step-by-Step Plan

1. Read `TODO.md` to identify the first incomplete task by heading prefix.
2. Read the relevant task body, dependency notes, validation requirements, and completion-record expectations.
3. Inspect recent Git history only enough to see whether the latest commit explicitly mentions an unfinished issue directly relevant to the selected task.
4. Inspect the minimum relevant source, fixture, and documentation files needed for that task.
5. Implement the task as specified, avoiding workarounds or scope narrowing.
6. Add or update focused tests/fixtures required by the task.
7. Run the task-specified validation and any directly relevant narrower checks first; run broader checks if required by the task or by the modified area.
8. Fix any failures that are in scope for the selected task.
9. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling its completion record, or record a blocking prerequisite if completion is impossible.
10. Update this file after key milestones or plan changes.
11. Inspect Git status/diff/log, stage only intended files, and commit with a task-specific message.
12. Stop without beginning the next task.

## Progress

- Initial execution plan recorded before repository inspection.
- Identified first incomplete `TODO.md` task: `P7-A4：B-36 spec-uncovered surface 早拒`.
- Next step: inspect the latest commit only for directly relevant unfinished B-36 notes, then inspect B-36 inventory, strategy, fixtures, and production fallback sites.
- Latest commit is P7-A3 and does not declare an unfinished B-36 blocker.
- B-36 active inventory has 58 `FrontendReject` rows. Current fixture directory is still `IGNORE-UNTIL-FIX:B-36` with placeholder pending-gate diagnostics.
- Implementation plan refined: add resolve-stage diagnostics for unresolved `async`/`await` and generator/`yield` surfaces; activate B-36 fixtures with those diagnostics; replace the 58 B-36 `UnsupportedMainBody` constructors with internal invariants/expectations; then update inventory, retired ledger, bucket docs, fixture index, stale counts, and TODO completion record.
- Implemented the B-36 frontend resolve gate and replaced the 58 B-36 production `UnsupportedMainBody` constructors.
- Synchronized audit data after formatting: `umb-audit diff` reports in sync with 1,159 active entries; `umb-audit stats` reports retired 125 and B-36 active 0.
- Next step: run P7-A4 validation commands and fix any failures before updating `TODO.md` and committing.
- Validation completed: B-36 list is empty; `umb-audit diff`, audit tests, failure-policy tests, B-36 fixtures, formatting, and clippy all pass.
- `TODO.md` was updated with `[DONE] P7-A4` and the completion record. Next step: inspect Git status/diff/log, then commit the task changes.
