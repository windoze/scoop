# Execution Plan

## Constraints

- Use `TODO.md` as the authoritative task source and complete exactly the first incomplete task.
- Treat a task as complete only when its heading is prefixed with `[DONE]`.
- Do not perform broad historical triage before selecting the current task.
- If a blocking missing feature, spec mismatch, or unscheduled failing test is found, add the minimum prerequisite task in `TODO.md`, commit, and stop.
- Update `PLAN.md` only if phase-level sequencing or dependencies change.
- Commit the completed task or documented blocker before stopping.

## Step-by-Step Plan

1. Read `TODO.md` to identify the first task heading that is not prefixed with `[DONE]`.
2. Check the latest commit only for unfinished work directly relevant to that selected task.
3. Inspect the selected task requirements, dependencies, validation requirements, and completion record.
4. Examine the relevant source, fixture, documentation, and tests needed for that task.
5. Implement the smallest spec-correct change that fully satisfies the selected task, without workarounds or weakened fixtures.
6. Run focused validation first, then broader required validation from the task. Address any observed unscheduled failures by fixing them or adding prerequisite tasks before marking completion.
7. Update `TODO.md` by prefixing the selected task title with `[DONE]` and recording completion details if the task is fully complete.
8. Update this file after key progress points or plan changes.
9. Review `git status`, `git diff`, and recent commits before committing.
10. Commit all intended changes with a clear task-tagged message, then stop without starting the next task.

## Current Status

- First incomplete task selected: `P7-T04` in `TODO-6.md`.
- Latest commit is `[P7-T03R] Review LLVM body emission migration`; no directly relevant unfinished issue was identified from the commit title.
- Inspection found a concrete prerequisite gap: current LIR facts/base context do not publish the initializer body, physical layout, callable identity, or single-owner TypeStore contracts needed to remove LLVM's HIR/pass-view handoff without a workaround.
- `TODO.md` and `TODO-6.md` were updated to insert `P7-T04-a` before `P7-T04`; `P7-T04` now depends on that prerequisite.
- Stop condition for this invocation: validate the TODO-only dependency update, commit it with the plan file, and do not start `P7-T04-a`.
