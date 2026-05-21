# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that task, then stop.
- If a concrete blocker prevents correct implementation, update `TODO.md` with the minimum prerequisite task instead of using a workaround.

## Step-by-Step Plan

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check recent git context only as needed for that task, including whether the latest commit mentions an unfinished issue directly relevant to it.
3. Inspect the relevant source, tests, fixtures, and documentation for the selected task.
4. Implement the task with minimal, spec-correct changes.
5. Run focused validation first, then broader required validation from the task.
6. Fix any failures that are caused by or block the current task.
7. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and updating its completion record.
8. Update this file when key steps complete or if the plan changes.
9. Review git status and diff, then commit all intended changes with a task-specific message.
10. Stop without starting the next task.

## Progress

- Initial plan recorded before inspecting the task list.
- `TODO.md` inspected; first incomplete task is `P4-T04R` (`Review P4 全包完成度`) in `TODO-5.md`.
- `TODO-5.md` inspected. Review requires re-running P4-T04 validation plus searches for mutable MIR input / nested P4 output paths, and then recording the P4 purity conclusion.
- Latest commit is `[P4-T04] Complete effect facts cleanup audit`, directly relevant as the task being reviewed; no separate unfinished issue was mentioned in the commit summary.
- Static review completed: `EffectFactsStageOutput` holds only `MaterializedEffectFacts`; P4 uses read-only `&MirStageOutput`; docs state P4 purity as complete and leave LIR/codegen nested handoff to P5/P7.
- Required validation passed: `cargo fmt`, dependency gate, `scoopc_effect_facts` tests, `effect_facts_stage` tests, clippy with warnings denied, `git diff --check`, and the P4 mutable/nested-output searches.
- `TODO.md` and `TODO-5.md` updated to mark `P4-T04R` as `[DONE]` with the review conclusion and validation record.
