# Execution Plan

## Constraints

- `TODO.md` is the authoritative task source.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Do not skip review tasks or tasks with completion notes but no `[DONE]` prefix.
- Do not use workarounds for spec mismatches; if a concrete prerequisite is required, add the minimum prerequisite task to `TODO.md`, commit that bookkeeping, and stop.
- Update `PLAN.md` only if phase-level sequencing, dependencies, assumptions, or completion criteria change.
- Mark the completed task by prefixing its title with `[DONE]` and updating its completion record.
- Commit all relevant uncommitted files after finishing or after recording a blocker.

## Step-by-Step Plan

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Inspect only the files and context relevant to that task, including recent git state if needed to detect directly relevant unfinished work.
3. Implement the task as written, without narrowing scope or introducing fixture-only behavior.
4. Add or update the smallest relevant tests or fixtures required by the task.
5. Run the task-specified validation commands and any directly relevant regression tests.
6. If validation fails, fix the cause when it is in scope; if a concrete prerequisite blocks the task, update `TODO.md` with that prerequisite and stop after committing.
7. Update this plan file after key milestones or any plan change.
8. When the task is complete, update `TODO.md` with `[DONE]` and a completion record.
9. Run final relevant validation, inspect git status/diff, and commit the completed task with a clear task-tagged message.
10. Stop without starting the next task.

## Progress Log

- Initial execution plan recorded before project commands.
- First incomplete task identified: `MIR-T14R` (`Review MIR-T14 phase exit audit`). This invocation will perform only that review task, rerun its required validations, update `TODO.md` with the review conclusion if successful, commit, and stop.
- Static review completed: `MIR_REFACTOR_PHASE_EXIT_AUDIT.md`, `PLAN.md` M8/§4, `PIPELINE_GAPS.md` §9, fixture/golden coverage, diagnostics comments, stable dump sharing, and codegen owner links were checked.
- Validation completed successfully: `refactor_hir_preflight`, `refactor_mir_no_todo`, `refactor_materialized_mir`, `dump_mir`, `mir_refactor` fixture matrix, 16 diagnostics fixtures, and `clippy` for `scoopc`/`scoop` with warnings denied all passed.
- `TODO.md` updated: `MIR-T14R` is marked `[DONE]` with the review conclusion, audit coverage, fixture checks, diagnostics checks, validation commands, and lint results.
