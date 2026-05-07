# Claude Plan

## Current Invocation Plan

1. Read `TODO.md` first and identify the first task whose title is not prefixed with `[DONE]`.
2. Check the latest commit only for an explicitly mentioned unfinished issue that directly affects that task.
3. Inspect the task requirements, dependencies, validation commands, and completion record.
4. Implement the task as written, without narrowing scope or using fixture-only workarounds.
5. If a concrete blocker prevents spec-correct implementation, update `TODO.md` with the minimum prerequisite task and stop after committing that bookkeeping change.
6. Run the task's required validation and any directly relevant tests; fix failures that are in scope for the current task.
7. Mark the task title `[DONE]` in `TODO.md` and update its completion record.
8. Update `PLAN.md` only if phase-level sequencing or completion criteria change.
9. Commit all changes for this invocation with a task-specific message, then stop.

## Progress Log

- Initialized the invocation plan before running repository commands.
- Read `TODO.md`; the first incomplete task is `CG-T07R` (`Review CG-T07 extern global 与 GC surface`). This invocation will perform only that review task and then stop.
- After reading the full `CG-T07R` entry, its body heading and latest commit indicate the review was completed, but the task index still lacks `[DONE]`. I will re-check `CG-T07R` scope, run/inspect the required validation, fix any real issue found, and at minimum repair the TODO index consistency before committing.
- Reviewed representative extern-global and GC pin/handle implementation paths. Current evidence shows extern globals flow through HIR/MIR storage contracts into LLVM external/TLS globals with unsafe access gating, while GC pin/handle intrinsics carry MIR policy metadata and call runtime root/handle APIs rather than raw pointer shortcuts.
- Ran the CG-T07R validation set, including extern-global unit/fixtures, GC pin/handle fixtures under normal and stress root-verification environments, codegen inventory, runtime ABI allowlist, targeted source searches, and clippy. No failures or new CG-T07 issues were found.
- Updated `TODO.md` so the task index also marks `CG-T07R` as `[DONE]`, matching the completed task heading and review record. Preparing the task commit now.
