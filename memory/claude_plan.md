# Claude Plan

## Current Objective

Complete exactly one task: the first incomplete detailed task referenced by `TODO.md`, using the corresponding `TODO-Px.md` file as the source of truth.

## Execution Plan

1. Read `TODO.md` as an index only.
2. Open referenced `TODO-Px.md` files in task order and identify the first task whose detailed heading is not prefixed with `[DONE]`.
3. Check the latest commit message only for directly relevant unfinished work tied to the selected task.
4. Inspect the selected task requirements, dependencies, validation requirements, and completion record.
5. Implement the selected task as specified, without narrowing scope or using fixture-only workarounds.
6. If a concrete blocker prevents correct implementation, add the minimum prerequisite task in the correct `TODO-Px.md` position, sync `TODO.md`, commit that bookkeeping, and stop.
7. Run relevant tests and formatting/lint checks required by the task and repository guidance.
8. Mark the completed task heading with `[DONE]` in the detailed TODO file and update its completion record.
9. Sync `TODO.md` if any indexed task title/status/order changed.
10. Commit all changes for this task with a descriptive task-tagged commit message.
11. Stop after the single task is complete.

## Progress Log

- Initialized execution plan before inspecting task files or running commands.
- Read `TODO.md` and `TODO-P6-part2.md`; selected first incomplete detailed task `P6-T03`.
- Latest commit is `[P6-T02qd] Publish resumed local/home binding contract`, which is directly relevant as the most recent prerequisite recorded under `P6-T03`.
- Inspected the current refactor LLVM entry: `effect_refactor/body.rs` is still a placeholder and `llvm/emit.rs` still fail-fast rejects effectful refactor body lowering.
- While checking source-slice lowering needs for `effect_multi_escape_indirect_direct_while.scoop`, found a concrete prerequisite gap: canonical MIR now publishes `StatementKind::StoreMember` and `Rvalue::MemberAccess`, but `llvm/codegen/mir_body.rs` still rejects both in the generic MIR lowering helpers that `P6-T03` would need for straight-line source slices.

## Blocker Handling

`P6-T03` cannot be implemented spec-correctly until source-slice member read/write lowering is available from published MIR metadata. I will add one prerequisite task immediately before `P6-T03`, sync `TODO.md`, record the dependency on `P6-T03`, commit the bookkeeping change, and stop.

## Selected Task

`P6-T03`: implement refactor LLVM body lowering from the P5 state graph / boundary contract, without redoing state-machine transformation in the backend.

## Immediate Implementation Steps

1. Add `P6-T02qe` before `P6-T03` in `TODO-P6-part2.md`.
2. Add the same `P6-T02qe` index row in `TODO.md`.
3. Update `P6-T03` dependencies and completion notes to point at `P6-T02qe`.
4. Commit the blocker bookkeeping and stop without implementing `P6-T03`.

## Final Status For This Invocation

- Added prerequisite task `P6-T02qe` to track source-slice member read/write LLVM lowering.
- Synced `TODO.md` with the detailed task order.
- Left `P6-T03` incomplete, as required when a concrete prerequisite blocker is found.
