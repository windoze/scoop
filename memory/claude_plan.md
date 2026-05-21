# Claude Execution Plan

## Scope
- Execute exactly the first incomplete task in `TODO.md`.
- Treat `TODO.md` as authoritative for task ordering, requirements, dependencies, validation, and completion records.
- Stop after completing and committing that one task, or after committing any required blocker/prerequisite task-list update.

## Step-by-Step Plan
1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit message only for unfinished work directly relevant to that selected task.
3. Read the selected task details, dependencies, validation requirements, and relevant project files.
4. Implement the selected task as written, without narrowing scope or using fixture-only workarounds.
5. If a spec mismatch or missing prerequisite blocks the task, update `TODO.md` with the minimum prerequisite task in the correct order, keep the current task incomplete, commit the bookkeeping change, and stop.
6. Run the task-specific validation and any relevant broader tests required by the task.
7. Fix any issues introduced by the implementation, then rerun validation until it passes or a concrete blocker is recorded.
8. Mark the task heading in `TODO.md` with `[DONE]` and update its completion record.
9. Update this progress file after key milestones or if the plan changes.
10. Inspect git status/diff/log, commit all intended changes with a task-scoped message, and stop.

## Progress Log
- Plan initialized before reading task details.
- Identified first incomplete task as `P2-T02`: fix the public HIR stage handoff shape to HIR plus `hir_facts`.
- Latest commit is `P2-T01R` review and does not add a directly relevant unfinished prerequisite.
- Implementation in progress: added `HirFacts` contract bridge coverage, renamed the public HIR stage output to `HirStageOutput`, and routed preflight checks through `hir_facts()`.
- Validation completed: HIR facts tests, HIR stage tests, HIR preflight tests, HIR fixture suite, and full clippy with `-D warnings` passed.
- Task bookkeeping completed: `P2-T02` marked `[DONE]` in `TODO.md` and `TODO-3.md` with completion record.
