# Claude Execution Plan

## Scope

- Read `TODO.md` as the task index, then inspect the referenced detailed `TODO-Px.md` files in order.
- Select the first detailed task whose heading is not prefixed with `[DONE]`.
- Complete exactly that one task, or if it is blocked by a concrete prerequisite, record the prerequisite in the correct detailed TODO file, sync `TODO.md`, commit, and stop.

## Execution Steps

1. Identify the first incomplete detailed task from the TODO index and detailed phase files.
2. Inspect only the code, fixtures, tests, and documentation relevant to that task.
3. Implement the smallest spec-correct change for the selected task; do not use workarounds or weaken fixtures.
4. Run targeted validation first, then broader required validation if feasible.
5. Update the detailed TODO file by prefixing the completed task heading with `[DONE]` and adding a completion record.
6. Sync `TODO.md` with any changed task status, title, ordering, or new prerequisite tasks.
7. Update this plan file whenever a key step completes or the plan changes.
8. Commit all relevant changes with a descriptive task-tagged commit message.
9. Stop after the commit without starting the next task.

## Current Status

- Selected task: `P6-T06` in `TODO-P6-part3.md` - make `NoOutward` LLVM lowering use plain ABI and ordinary call dispatch.
- Latest commit `26193bc1 [P5-T08] Keep NoOutward late-lowered handoff plain` is a completed prerequisite and does not record an unfinished issue.
- Implemented initial plain-ABI path in P6: ABI query now publishes plain callable layouts separately from effect-step layouts; plain callable bodies are emitted as ordinary functions from P5 plain source slices; pure direct calls to plain callees use ordinary direct calls; generic reachable-body emission now skips refactor-owned plain callables.
- Validation passed for pure NoOutward plain ABI and effect-step SingleCase/CanonicalFull fixtures.
- Blocking issue found while running `continuation_resume_surface_named_tuple_and_unit_basic.scoop`: P4/P5 can publish a `NoOutward` callable as `Plain` even when the body still contains local effect/control constructs (`Handle` / `Perform` / `Resume`), but the plain handoff/emitter does not yet publish or consume a spec-correct lowering contract for those constructs.
- Updated plan: add a prerequisite task before `P6-T06`, sync `TODO.md`, leave `P6-T06` incomplete, commit current progress plus the new prerequisite, and stop.
