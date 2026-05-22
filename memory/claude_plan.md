# Current Invocation Plan

This file records the actionable execution plan and progress for the current TODO-driven invocation. It intentionally contains concise rationale and observable steps, not private reasoning.

## Plan

1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check recent git context only as needed for the selected task, including whether the latest commit mentions an unfinished issue directly relevant to it.
3. Read the selected task details, dependencies, validation requirements, and nearby completion records.
4. Inspect the relevant implementation and tests for that task.
5. Implement the task fully, or if a concrete prerequisite blocks spec-correct implementation, update `TODO.md` with the minimum prerequisite task and stop after committing that bookkeeping change.
6. Run targeted validation first, then broader required validation from the task where feasible. Any unscheduled failing test or fixture observed will be fixed or explicitly scheduled before marking the task complete.
7. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and adding a completion record. Update `PLAN.md` only if phase-level sequencing or criteria changed.
8. Run formatting/linting or other quality checks required by the task and repository guidance.
9. Inspect git status and diff, then commit all intended changes with a clear task-tagged commit message.
10. Stop after exactly one completed task or after committing a blocking prerequisite update.

## Progress

- Initial execution plan written before reading project task files or running commands.
- Read `TODO.md`; the first incomplete task is `P7-T04-b-3R` in `TODO-6.md`.
- Checked the latest commit summary: `f3f1c150 [P7-T04-b-3] Introduce ClassInstanceKey`, which is directly relevant to the selected review task.
- Read `TODO-6.md` task details and started the review. Found two review-scope fixes to make: restrict `ClassInstanceKey::for_unparameterized` to HIR internals, and make the direct-style MIR production verifier reject class constructor target type mismatch instead of leaving that solely to materialized/codegen validation.
- Implemented the review fixes, formatted the workspace, and reran `cargo test -p scoopc --no-default-features mir`; it passed with 172 tests.
- Completed required validation for the review task, updated `TODO-6.md` and `TODO.md` to mark `P7-T04-b-3R` done, and recorded the review conclusion plus validation commands.
