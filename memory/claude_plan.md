# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify the first incomplete task by finding the first task heading not prefixed with `[DONE]`.
- Complete exactly that one task, then stop.
- Do not perform broad historical triage before selecting the task.

## Execution Steps

1. Read `TODO.md` and identify the first incomplete task.
2. Check recent git context only as needed to detect unfinished work directly relevant to that task.
3. Inspect the code, fixtures, and tests relevant to the selected task.
4. Implement the task with minimal, spec-correct changes.
5. Add or update focused tests/fixtures required by the task.
6. Run relevant validation commands first, then broader validation if required by the task or if failures indicate risk.
7. If an unscheduled failing test or fixture is observed, fix it or add the minimum prerequisite/follow-up task to `TODO.md` before marking completion.
8. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and adding a completion record.
9. Update this plan file after key steps or plan changes.
10. Review `git status` and `git diff`, then commit all task-related changes with a clear task-tagged message.
11. Stop without starting the next task.

## Current Status

- First incomplete task identified: `P7-T05-a` in `TODO-6.md`.
- Latest commit is `[P7-T05R] Schedule backend residual cleanup`, directly relevant to `P7-T05-a`.
- Current focus: remove LLVM production codegen residual access to `MaterializedMirPassView` / `materialized_pass_view()` and replace any remaining callable/body/signature fallback with published LIR/base contracts.
- Implementation in progress: removed `CompilationUnitCodegenInputs.materialized_pass_view`, removed the codegen context accessor, switched direct-call signature lookup to LIR callable signature facts with parameter names, and expanded dependency-gate source boundary checks for the residual class.
- Implementation complete: production LLVM codegen no longer accepts `MaterializedMirPassView` / `materialized_pass_view()`; body lookup uses LIR source callable helpers; source signatures for declaration-only/helper callables are published through LIR/base contracts.
- Validation complete: dependency gate, LIR facts tests, no-default LLVM filters, default LLVM codegen tests, full run-pass fixtures, clippy with `-D warnings`, and `git diff --check` passed.
- TODO status updated: `P7-T05-a` is marked `[DONE]` in `TODO.md` and `TODO-6.md` with completion record.
