# Claude Execution Plan

## Scope
- Follow the repository task workflow exactly: complete the first incomplete detailed task and stop.
- Treat `TODO-Px.md` files as authoritative and keep `TODO.md` synchronized with any task status or ordering changes.
- Avoid unrelated triage; only address blockers that directly affect the selected task.

## Steps
1. Read `TODO.md` as the global index.
2. Open the referenced `TODO-Px.md` files in indexed order and identify the first task whose detailed heading is not prefixed with `[DONE]`.
3. Inspect the selected task requirements, dependencies, constraints, validation commands, and completion record.
4. Check the latest commit only for unfinished issues directly relevant to the selected task.
5. Implement the selected task as written, using minimal, spec-correct changes and avoiding workarounds.
6. If a concrete blocker prevents correct implementation, add the minimum prerequisite task in the appropriate detailed TODO file, sync `TODO.md`, commit, and stop.
7. Run the relevant validation commands for the task and fix any failures caused by this work.
8. Mark the completed task heading with `[DONE]` in its `TODO-Px.md` file and update its completion record.
9. Sync the corresponding `TODO.md` index entry with the same `[DONE]` marker if applicable.
10. Run final targeted verification after documentation updates when feasible.
11. Review the worktree, commit all relevant changes with a clear task-prefixed message, and stop without starting the next task.

## Progress Log
- Created initial execution plan before running repository commands.
- Selected `P6-T05a` as the first incomplete detailed task after reading `TODO.md` and `TODO-P6-part3.md`.
- Latest commit explicitly tracks this same `P6-T05a` blocker, so its unfinished issue is in scope for this invocation.
- Reproduced the blocker: `dump-effect-lowered continuation_resume_surface_named_tuple_and_unit_basic.scoop` fails because plain lowering treats a `Resume` site as an ordinary `Call`.
- Implementation direction: extend `Plain` callable handoff with an optional local effect/control contract that publishes state graph, frame, boundary map, resume map, continuation object, resume packings, and source statement classifications without changing the callable's public plain ABI.
- Implemented the plain-local handoff and P6 consumer path: plain bodies with local `handle` / `perform` / `resume` now publish and consume local control state while keeping ordinary callable ABI.
- Fixed direct root `main` symbol collision for plain main wrappers by giving the source plain entry an internal refactor symbol.
- Validation passed for the P6-T05a unit tests, required continuation/no-outward fixtures, and `cargo clippy --all-targets -- -D warnings`.
- Marked `P6-T05a` as `[DONE]` in `TODO-P6-part3.md`, synchronized `TODO.md`, and updated the detailed completion record.
