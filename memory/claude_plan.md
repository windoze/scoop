# Execution Plan

## Current Invocation

- Record this plan before running build, test, or project commands.
- Read `TODO.md` to identify the first task whose heading is not prefixed with `[DONE]`.
- Inspect only the files needed to understand and complete that task.
- Implement the task as written, without narrowing scope or introducing fixture-only workarounds.
- If a concrete prerequisite blocks the task, update `TODO.md` with the minimum prerequisite task, leave the blocked task incomplete, commit that bookkeeping, and stop.
- Run the task's required validation plus relevant regression tests.
- Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling its completion record.
- Commit all relevant changes for this invocation with a task-scoped message.
- Stop after completing exactly one task.

## Progress Log

- Plan initialized.
- First incomplete task identified: `MIR-T14` (`建立 MIR-only 验证矩阵并完成阶段退出审计`).
- Latest commit is `[MIR-T13R] Review policy gates`; no separate unfinished issue was identified from the latest commit message.
- Scope for this invocation: complete `MIR-T14`, update `TODO.md`, run the specified targeted validations, commit, and stop before `MIR-T14R`.
- Checked the remaining `HirOnly` preflight samples with refactor `dump-mir`; all three can now run through strict MIR production validation.
- Planned edits: remove HIR-only preflight exceptions, make MIR smoke use the strict stage boundary, add/update `mir_refactor/*.mir` goldens for every `mir_refactor/*.scoop`, and add a phase exit audit tied to the targeted validation matrix.
- Implemented the preflight/golden/audit edits. Next step is to format and run the `MIR-T14` validation matrix, then update `TODO.md` and commit if validation passes.
- Validation passed and `TODO.md` has been updated with `[DONE] MIR-T14` plus the completion record. Next step is to review the staged diff state and create the required task commit.
