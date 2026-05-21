# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that one task, then stop after committing.
- Do not perform broad historical triage before selecting the current task.

## Reasoning Summary

- The first priority is preserving the repository's declared task order.
- A task is complete only when its `TODO.md` title is explicitly marked `[DONE]`.
- If the selected task exposes a concrete blocker or missing prerequisite, the correct action is to add the minimum prerequisite task in `TODO.md`, commit that bookkeeping, and stop rather than working around the issue.
- `PLAN.md` should only change if phase-level sequencing or completion criteria change.

## Step-by-Step Plan

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Inspect recent git history only enough to detect whether the latest commit names unfinished work directly relevant to that task.
3. Inspect the relevant code, fixtures, and docs for the selected task.
4. Implement the task as written, without narrowing scope or substituting a workaround.
5. Add or update tests/fixtures required by the task.
6. Run focused validation first, then any task-required broader validation.
7. Fix any failures that are direct blockers for the selected task.
8. Update `TODO.md` by prefixing the selected task title with `[DONE]` and recording completion details.
9. Update this file whenever the plan changes or a key step completes.
10. Inspect git status and diff, then commit all intended changes with a task-specific message.
11. Stop without starting the next task.

## Progress Log

- Initial plan written before repository inspection.
- Identified `P3-T04R` as the first incomplete task in `TODO.md`; next steps are to inspect `TODO-4.md` details and the latest commit for directly relevant unfinished work.
- Latest commit is `[P3-T04] Switch downstream MIR queries to MirFacts`, directly relevant to this review task, and does not state an unfinished blocker in the commit subject/stat output. The review will focus on the P3-T04 touched files plus the required searches.
- Review inspection found no remaining `collect_nominal_direct_supertypes_from_mir_file` or `with_nominal_direct_supertypes` paths. Remaining `materialized_pass_view()` uses are the canonical MIR query surface, P4/P5 handoff accessors/tests, or the documented LLVM transition bridge.
- Validation completed successfully: `cargo fmt`, focused `scoopc` tests for `effect_facts_stage`, `effect_lowering_stage`, and `effect_lowered`, the `effect_lowered` fixture suite, `cargo clippy --all-targets -- -D warnings`, and `git diff --check` all passed.
- Marked `P3-T04R` complete in `TODO.md` and `TODO-4.md` with the review conclusion, search results, validation commands, and residual risks.
