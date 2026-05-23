# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that one task, then stop.

## Steps

1. Read `TODO.md` and identify the first incomplete task.
2. Check recent repository context only as needed for that task, including the latest commit if it appears relevant.
3. Inspect the code, tests, and fixtures directly related to the selected task.
4. Implement the smallest spec-correct change that fully satisfies the task.
5. Add or update tests/fixtures required by the task.
6. Run targeted validation first, then broader required validation if feasible.
7. If validation exposes unscheduled failures, fix them or add the minimum prerequisite task before marking the current task done.
8. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling its completion record.
9. Update this file when key steps complete or the plan changes.
10. Commit all relevant changes with a clear task-tagged message.
11. Stop without starting the next task.

## Current Status

- Plan recorded before task execution.
- `TODO.md` first incomplete task identified: `P7-T05R` in `TODO-6.md`.
- `P7-T05R` requirements inspected. Latest commit `[P7-T05-a] Clear LLVM codegen residuals` is directly relevant and is the prerequisite cleanup to review.
- Current review focus: verify LLVM production backend no longer reaches through `MaterializedMirPassView` / HIR or raw MIR fallback for body/signature/ABI contracts, and rerun the P7-T05 validation matrix.
- Review found a concrete blocker: pass-view residuals are gone, but LLVM production still has HIR-derived callable signature/ABI/body paths (`source_signatures`, `fun_index`, `hir::FunDecl`) and class ctor HIR body lowering.
- Added prerequisite task `P7-T05-b` before `P7-T05R` in `TODO.md` and `TODO-6.md`. `P7-T05R` remains incomplete and now depends on `P7-T05-b`.
- Validation complete for the blocker scheduling change: `git diff --check` passed. The full `P7-T05R` validation matrix was not run because the review remains blocked by `P7-T05-b`.
- Next step: commit the task-list update and stop.
