# Claude Plan

## Session Goal
- Locate the first incomplete detailed task from `TODO.md` and the referenced `TODO-Px.md` files.
- Complete exactly that task if feasible.
- If a concrete blocker prevents spec-correct completion, add the minimum prerequisite task(s), sync `TODO.md`, and stop.

## Execution Plan
1. Read `TODO.md` as the task index.
2. Read the referenced `TODO-Px.md` files in task order to identify the first task whose title is not prefixed with `[DONE]`.
3. Inspect the latest commit message for any directly relevant unfinished work tied to that task.
4. Read the current task details, constraints, dependencies, and completion record in the authoritative `TODO-Px.md` file.
5. Inspect the relevant code, tests, and existing implementation surface needed for the selected task.
6. Implement the minimal correct changes required to complete the task without workaround behavior.
7. Run focused verification first, then broader required checks such as formatting, tests, and linting as appropriate to the scope.
8. Update `memory/claude_plan.md` with progress and any plan changes after key milestones.
9. Mark the task title as `[DONE]` in the relevant `TODO-Px.md` file and update its completion record.
10. Sync `TODO.md` if task markers, ordering, or titles changed.
11. Update `PLAN.md` only if phase-level sequencing, dependencies, or completion criteria changed.
12. Commit all required changes with a task-specific message, then stop.

## Progress Log
- Initialized plan file before reading repository task files.
- Read `TODO.md` and identified the first incomplete detailed task as `P6-T02qb` in `TODO-P6-part2.md`.
- Checked the latest commit: `[P6-T02qb] Track cleanup payload carrier prerequisite`, which is directly relevant to the current task.
- Detected an interrupted in-progress state: the worktree already contains uncommitted edits in `crates/scoopc/src/effect_lowered/{frame,ir}.rs` for this task.
- Ran focused tests to assess the resume state. Current build fails because the new pending-payload transport shape is only partially wired through `materialize.rs`, `opt.rs`, `dump.rs`, and LLVM layout/query helpers.

## Current Implementation Plan
1. Complete the late-lowered contract wiring for pending cleanup/finally payload transport:
   - finish `LateLoweredHandlePendingPayloadTransport` integration in `materialize.rs`;
   - publish typed `HandlePendingPayload` frame-slot usage where required;
   - update dump/rendering and any exhaustive matches.
2. Extend late-lowered optimization/helpers so redirected contracts and frame-slot liveness preserve the new published transport contract.
3. Extend LLVM layout/query publication and fail-fast validation for pending payload transports.
4. Add or update targeted tests covering:
   - late-lowered contract publication for pending payload transport;
   - LLVM query publication and fail-fast on missing/drifted transport;
   - stable dump visibility for the new contract/slot shape.
5. Run the task-required verification commands, then mark `P6-T02qb` as done, sync `TODO.md` if needed, commit, and stop.

## Latest Status
- Completed late-lowered publication for cleanup/finally pending payload transport:
  - added typed `HandlePendingPayload { site_id, case_tag }` frame-slot identities;
  - published `pending_payload_transports` on `LateLoweredHandleDispatchContract`;
  - exposed the contract through LLVM handle-dispatch query/layout with fail-fast validation.
- Updated stable dump text and the directly affected `tests/fixtures/effect_lowered/*.effectlowered` goldens that now need an explicit `pending_payload_transports:` section.
- Synchronized task bookkeeping: `P6-T02qb` is now marked `[DONE]` in `TODO-P6-part2.md` and `TODO.md`.
- Verification completed successfully:
  - `cargo test -p scoopc refactor_handle_dispatch_contract_ --no-fail-fast`
  - `cargo test -p scoopc refactor_llvm_handle_dispatch --no-fail-fast`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/effect_lowered/handle_finally_boundary.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/effect_lowered/dropped_continuation_abandons_remaining_work.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
- Additional note: a broader `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/effect_lowered` sweep still reports an unrelated historical golden mismatch on `dynamic_fallback_widening.effectlowered`. This was not pursued further because it is outside the selected `P6-T02qb` task scope and does not block the required task validation above.
