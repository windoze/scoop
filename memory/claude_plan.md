# Execution Plan

I will follow TODO.md as the source of truth and complete exactly the first task whose heading is not prefixed with `[DONE]`.

1. Inspect `TODO.md` to find the first incomplete task and read only the surrounding task details needed to execute it.
2. Check the latest commit message for any unfinished issue directly relevant to that selected task.
3. Review the code, fixtures, and documentation directly related to the selected task.
4. Implement the task as specified, without narrowing scope or introducing workarounds.
5. Run formatting, linting, targeted validation, then the required full validation unless only documentation changed or a prior green result can be reused under the task rules.
6. If validation reveals unscheduled failures, fix them if in scope or add the minimum prerequisite/follow-up task in `TODO.md` before marking anything complete.
7. Update `TODO.md` by prefixing the completed task title with `[DONE]` and adding a completion record. Update `PLAN.md` only if phase-level sequencing or criteria changed.
8. Commit all changes for this invocation with a descriptive message and the required co-author trailer, then stop.

Progress will be updated here at major milestones or if the plan changes.

## Milestone: Selected Task

Selected first incomplete task: `P3-T06R`, the review task for replacing sealed `AnyRef` / `AnyValue` marker bounds with first-class `ref` / `value` constraint kinds. I will verify the prior implementation against the TODO requirements, fix any directly related gaps, run the required validation, update task records, and commit only this task's work plus the requested memory plan file.

## Milestone: Review Focus

Initial review confirms the intended files contain `GenericBound::Ref` / `GenericBound::Value`, `LoweredGenericBound::{Ref,Value}`, sysroot atomic bounds using `<T: ref>` / `<T: value>`, and no active compiler/sysroot `AnyRef` / `AnyValue` marker declarations. I am now specifically checking call-site enforcement for generic forwarding cases, because direct concrete instantiation alone is not enough to prove the bound-kind replacement is complete.

## Milestone: Review Gap Found

The review found a directly relevant enforcement gap: when a generic type parameter was passed to a function requiring `<T: ref>` / `<T: value>`, the call-site where-bound check skipped `TypeKind::Param` instead of verifying that the caller's type parameter carried a compatible bound. This would allow unconstrained generic forwarding around the new bound-kind constraints. I will fix bound-kind implication for type parameters and add negative fixtures covering unconstrained forwarding to both `ref` and `value` requirements, plus a positive constrained forwarding case.

## Milestone: Fix Implemented

Implemented bound-kind implication for type parameters: `ref` / `value` checks now inspect the caller's declared generic bounds instead of skipping `TypeKind::Param`. Added fixtures for unconstrained generic forwarding failures and constrained forwarding success. Next steps are formatting, targeted fixtures, linting, full tests, and full fixture validation.

## Milestone: Constraint Scope Adjustment

Targeted validation showed constrained generic nominal forwarding (`RefBox<U>` with `U: ref`) also needs bound-kind scopes during type-position lowering. I updated type-ref lowering to push generic bound scopes and made type-param bound lookup robust to existing scope identity differences while still preferring exact declaration identity.

## Milestone: Targeted Validation Passed

The targeted ref/value fixture set now passes with the rebuilt compiler, including direct concrete failures, unconstrained generic forwarding failures, and constrained generic forwarding through both functions and nominal type arguments. I am proceeding with clippy, full Rust tests, full fixture suite, spec fixture check, and active marker-name search.

## Milestone: Worktree Isolation

The worktree also contains unrelated default-internal visibility/sysroot changes and a separate untracked document, which are outside `P3-T06R` and would advance `P3-T07` early. I will preserve those unrelated changes out of the validation tree, complete and commit only the `P3-T06R` review fix plus task bookkeeping, then restore the unrelated changes afterward.

## Milestone: Review Complete

The P3-T06R review fix is complete: generic forwarding now enforces `ref` / `value` bound-kind implication for type parameters, including member function bodies and nominal type-argument lowering. Formatting, clippy, targeted ref/value fixtures, full Rust tests, spec fixture check, full fixture suite, and active marker-name search passed. I have marked `P3-T06R` done in `TODO.md` and `TODO-3.md`; next step is committing the P3-T06R files, then restoring the unrelated stashed P3-T07 worktree changes.
