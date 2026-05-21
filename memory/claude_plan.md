# Execution Plan

## Scope

- Use `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that one task, then stop after committing the result.

## Steps

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check recent repository context only as needed for that selected task, including the latest commit if it may mention an unfinished issue relevant to the task.
3. Inspect the code, fixtures, and docs related to the selected task.
4. Implement the smallest spec-correct change that fully satisfies the task.
5. Add or update focused tests and fixtures required by the task.
6. Run targeted validation first, then any broader validation explicitly required by the task.
7. Fix any regressions or task-blocking issues encountered during validation.
8. Update this plan file when key steps complete or if the plan changes.
9. Update `TODO.md` by prefixing the completed task title with `[DONE]` and filling in the completion record.
10. Update `PLAN.md` only if phase-level sequencing, dependencies, assumptions, or completion criteria actually change.
11. Inspect git status and diff, then commit all relevant changes with a task-specific commit message.
12. Stop without starting the next task.

## Current Status

- Initial execution plan written.
- First incomplete task selected: `TODO-5-INIT`.
- Latest commit is `[P3-T07R] Review P3 completion`; no unfinished issue was identified there.
- P4/P5 boundary inspection completed.
- Current P4 findings: `EffectFactsStageOutput` still nests `MirStageOutput`; the effect facts builder still takes `&mut MaterializedMir` and extends snapshot-owned type context.
- Current P5 findings: `EffectLoweredStageOutput` still wraps `EffectFactsStageOutput + LateLoweredProgram`; LLVM codegen still reads pass view, effect facts, types, and HIR compatibility scaffold through that path.
- `TODO-5.md` has been rewritten into detailed P4/P5 tasks with review steps and completion criteria.
- `TODO.md` has been synchronized with `TODO-5-INIT` marked done and the new P4/P5 task rows inserted.
- Validation completed: `git diff --check` passed.
- Next step: inspect git status/diff/log and commit this task.
