# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`, then stop.
- Avoid broad triage before identifying the current task.
- If a blocking prerequisite is discovered, update `TODO.md`, commit that bookkeeping, and stop.

## Step-by-Step Plan

1. Read `TODO.md` and identify the first incomplete task by heading prefix.
2. Check the latest commit message only for unfinished work directly relevant to that task.
3. Inspect the minimum relevant code, tests, fixtures, and documentation needed for the selected task.
4. Implement the task without narrowing scope or introducing workarounds.
5. Add or update tests/fixtures required by the task.
6. Run the task-specified validation plus relevant workspace checks.
7. Fix any task-relevant failures discovered during validation.
8. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling its completion record.
9. Update this file after key milestones or any plan changes.
10. Commit all changes for this task with a clear task-prefixed message.
11. Stop without starting the next task.

## Progress Log

- Initial execution plan recorded before reading the task list.
- Identified first incomplete task: `HIR-T13` (`建立 HIR -> next-stage preflight，阻止 HIR gap 流入 MIR`).
- Next action: check the latest commit for directly relevant unfinished notes, then inspect the refactor HIR verifier/stage tests and MIR handoff code needed for preflight.
- Latest commit is `[HIR-T12] Add top-level HIR init contracts`; it does not mention unfinished work relevant to `HIR-T13`.
- Implementation direction: add a focused refactor HIR preflight test/API that loads the HIR completeness fixture set through typed HIR/no-Todo verification, checks required typed side tables, and runs direct-style MIR smoke only for representative samples.
- Implemented `hir_preflight` test coverage, added assignment place and class literal runtime typecheck fixtures, and validated the preflight, dump-hir fixture set, MIR smoke samples, related no-Todo/dump-hir tests, and clippy.
- Updated `TODO.md` to mark `HIR-T13` as complete with the validation record. Next action: inspect git status/diff, commit this task, then stop.
