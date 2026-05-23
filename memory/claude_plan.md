# Claude Execution Plan

## Scope

- Work from `TODO.md` as the authoritative task list.
- Identify and complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Stop after completing, documenting, testing, and committing that one task.
- Do not perform broad historical triage before selecting the current task.

## Step-by-Step Plan

1. Read `TODO.md` and identify the first incomplete task exactly as ordered.
2. Check the latest commit only for directly relevant unfinished work tied to that task.
3. Read the task details, dependencies, validation requirements, and nearby completion records.
4. Inspect only the code, fixtures, docs, and tests needed for the selected task.
5. Implement the task completely, or add the minimum prerequisite task if a concrete blocker makes completion impossible.
6. Run focused validation first, then required broader validation from the task record.
7. Fix any relevant failures. For unrelated unscheduled test or fixture failures observed during validation, either fix them or add explicit TODO scheduling before marking the task done.
8. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling its completion record.
9. Update `PLAN.md` only if phase-level sequencing, dependencies, assumptions, or completion criteria changed.
10. Review the git diff and status.
11. Commit all intended changes with a clear task-tagged message.
12. Stop without starting the next task.

## Progress Log

- Initialized execution plan before running repository inspection commands.
- Identified first incomplete task: `P9-T06` in `TODO-7.md`, extracting `scoopc_effect_facts_stage` and `scoopc_lir`.
- Latest commit is `[P9-T05R] Review scoopc_mir extraction`; no directly relevant unfinished issue was recorded in the commit subject.
- Observed untracked `PLUGIN_ABI.md`; treating it as unrelated external work unless later proven relevant.
- Beginning implementation by auditing current `effect`, `effect_facts`, `effect_lowered`, pipeline, LLVM, and dependency-gate boundaries before moving files.
- Blocker found for `P9-T06`: current LIR code still directly publishes HIR-shaped source payload and matches AST delegation kinds; `scoopc_mir` also directly depends on `scoopc_hir`/`scoopc_ast`, making the literal full-tree `scoopc_lir` completion check inconsistent with the current P9 DAG.
- Added prerequisite task `P9-T06-a` in `TODO.md` and `TODO-7.md`; `P9-T06` now depends on it. This invocation will commit the scheduling/blocker update and stop.
