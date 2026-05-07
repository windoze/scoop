# Claude Execution Plan

## Scope
- Work on exactly the first incomplete task in `TODO.md`.
- Do not skip review tasks or tasks that have completion notes but no `[DONE]` prefix.
- Stop after completing and committing one task, or after committing any required blocker/prerequisite update.

## Execution Plan
1. Read `TODO.md` to identify the first heading not prefixed with `[DONE]`.
2. Check the latest commit message for any explicitly unfinished issue directly relevant to that task.
3. Read the task details, dependencies, validation requirements, and nearby completion records.
4. Inspect only the relevant project files needed to implement or review that task.
5. Implement the smallest spec-correct change required by the task, avoiding workarounds or fixture-only hacks.
6. Run the task-required validation and any focused tests needed to prove correctness.
7. If a concrete blocker prevents correct implementation, update `TODO.md` with the minimum prerequisite task and stop after committing that bookkeeping.
8. If the task is complete, update `TODO.md` by prefixing the task title with `[DONE]` and filling the completion record.
9. Update this plan file at key milestones or if the plan changes.
10. Commit all relevant changes with a task-scoped commit message, then stop.

## Current Status
- Identified first incomplete task: `CG-T04b` value boxing composite transport lowering.
- Latest commit `4ee11722 [CG-T04a] Add composite transport contract gate` matches the completed prerequisite and does not add a new unfinished prerequisite.

## CG-T04b Focus
1. Locate the current composite layout descriptor, verifier gate, runtime hook surface, and value boxing/unboxing lowering paths.
2. Determine whether tuple/struct/value-type boxing can be implemented with the existing MIR-T10/CG-T04a metadata contract.
3. If the contract is sufficient, implement descriptor-driven allocation/store/erase/unbox without `u64`/ref fallback.
4. Preserve an explicit gate for payload-bearing enum boxing until `CG-T04c`.
5. Add focused tests for value boxing transport and run the task-required validation.

## Blocker Discovered
- `MirBoxingReason::AnyErasure` and `MirBoxingReason::RefErasure` exist in `crates/scoopc/src/mir/transport.rs`, but there is no MIR lowering/materialization producer for value-to-`Any`/`Ref` boxing intent.
- Implementing `CG-T04b` without that contract would require codegen to infer erasure from source/target types, which violates the current task constraints.
- Inserted `CG-T04b0` before `CG-T04b` in `TODO.md`, kept `CG-T04b` incomplete, and recorded the blocker there.
- Committed the task-order update as `58e7b637 [CG-T04b] Add boxing transport prerequisite`.
