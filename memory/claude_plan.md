# Execution Plan

## Objective
Complete exactly the first incomplete task in `TODO.md`, then stop after committing the completed work.

## Reasoning Summary
`TODO.md` is the authoritative task list. I will identify the first heading that is not explicitly prefixed with `[DONE]`, treat that as the only execution unit for this invocation, and avoid unrelated issue triage unless a failure directly blocks that task or must be scheduled under the test/fixture failure policy.

## Step-by-Step Plan
1. Read `TODO.md` to identify the first incomplete task, including its dependencies, validation requirements, and completion record expectations.
2. Check the latest commit only for directly relevant unfinished work tied to that selected task.
3. Inspect the smallest necessary set of code, tests, fixtures, and documentation for the selected task.
4. Implement the task as written, without workarounds or spec deviations. If a concrete prerequisite blocks correct implementation, add the minimum prerequisite task to `TODO.md`, commit that scheduling change, and stop.
5. Run validation in the required order: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, then the relevant full Rust and fixture suites unless the task is documentation-only and no compiled behavior changed.
6. Address any unscheduled failing test or fixture by fixing it or scheduling the minimum required prerequisite/follow-up before marking the task complete.
7. Mark the completed task title with `[DONE]` in `TODO.md`, update its completion record, and update this plan file at major milestones.
8. Commit all task-related changes with a descriptive message and the required co-author trailer.
9. Stop without starting the next task.

## Current Status
First incomplete task identified: `P4-T05R`, "Review constructor overload definition-time 规则". The latest commit is `[P4-T05] Implement constructor overload definition checks`, which is the direct dependency and review target. Next step is to review that commit's constructor overload implementation against the P4-T05R requirements.

## P4-T05R Review Plan
1. Inspect the P4-T05 diff and relevant implementation surfaces: AST/parser constructor generic metadata, resolver constructor overload metadata, type lowering/scope wiring, and `typecheck/overloads.rs` constructor collection/checking.
2. Confirm constructor duplicate signature, ctor-level generic shape mismatch, and vararg/non-vararg overlap all use the same definition-time helpers as function overloads.
3. Confirm ctor-level type parameters are distinguished from class-level type parameters in effective signatures and diagnostics.
4. Confirm P4-T05 did not implement or change P5 call-site constructor specificity.
5. Add or fix review findings if needed, then run required validation.
6. Mark `P4-T05R` `[DONE]` in both `TODO.md` and `TODO-4.md`, update the completion record, commit, and stop.

## Milestone: Review Validation Passed
The P4-T05R review found no blocking constructor overload defects. Formatting, clippy, targeted constructor overload fixtures, full Rust tests, spec fixture check, and the full fixture suite passed. `TODO.md` and `TODO-4.md` now mark `P4-T05R` complete. Next step is committing the task-related changes.
