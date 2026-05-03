## Execution Plan

I will not record private chain-of-thought verbatim, but I will keep a complete actionable plan and progress log here as requested.

### Initial Plan

1. Read `TODO.md` as the global index.
2. Open the referenced detailed task files (`TODO-P0.md`, `TODO-P1.md`, `TODO-P2.md`, etc.) in task order.
3. Identify the first detailed task whose heading is not prefixed with `[DONE]`.
4. Check the latest commit message for any directly relevant unfinished work related to that task.
5. Inspect the code and tests needed for that task only; avoid unrelated triage.
6. Implement the task completely, or if blocked by a concrete prerequisite, add the minimum prerequisite task in the correct detailed TODO file and sync `TODO.md`.
7. Run the relevant verification commands, including targeted tests and broader required checks when appropriate.
8. Update this file with progress and any plan changes during execution.
9. Mark the completed task with `[DONE]` in the relevant `TODO-Px.md` file, update its completion record, and sync `TODO.md` if needed.
10. Update `PLAN.md` only if phase-level sequencing or dependencies changed.
11. Commit all resulting changes with a task-specific message, then stop.

### Progress Log

- Started invocation.
- Created initial execution plan before repository inspection.
- Read `TODO.md` and identified the first incomplete detailed task as `P6-T03` in `TODO-P6.md`.
- Read the `P6-T03` task body and dependencies from `TODO-P6.md`.
- Checked the latest commit: `[P6-T02l] Publish handle region routing contract`.
- No new unfinished issue was explicitly called out in the latest commit message beyond the just-completed prerequisite.

### Current Execution Focus

- Inspect the existing refactor LLVM codegen implementation under `crates/scoopc/src/llvm/codegen/effect_refactor/` and the current pipeline entry points.
- Determine which portions of whole-body lowering already exist and which pieces are still missing for `P6-T03`.
- Implement the minimum complete refactor body emitter changes required by the task, then run the required targeted tests and checks.

### Findings So Far

- `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs` currently contains only a placeholder module comment.
- `crates/scoopc/src/llvm/emit.rs` still enforces `ensure_refactor_effect_lowering_is_supported(...)`, which rejects outward cases, resume-state lowering, and all effect boundary lowering paths before LLVM body lowering can run.
- This confirms `P6-T03` is not partially done in the backend yet; the core task is still to land the first real refactor body emitter and hook it into module emission.
- After tracing the late-lowered state graph and ABI query, I found a new concrete prerequisite blocker rather than a mere implementation-size issue.
- The current handoff publishes `ContinuationSchemaId -> surface-resume symbol/signature`, but it does not publish how that shared surface-resume symbol authoritatively dispatches to the owner-specific resume implementation.
- `ContinuationSchemaId` is canonicalized by shape in `crates/scoopc/src/effect_facts/builder.rs`, so one schema may legitimately be shared by multiple continuation objects / owners.
- Without a new published dispatch contract, `P6-T03` would have to scan raw continuation objects or invent runtime dispatch rules in the backend, which is explicitly forbidden by the task requirements.

### Plan Change

- Do not implement `P6-T03` yet.
- Add the minimum new prerequisite task in `TODO-P6.md` before `P6-T03`.
- Sync `TODO.md` to include the new index entry.
- Record the blocker in `P6-T03` completion notes and stop after committing these tracking changes.

### Progress Log

- Added new prerequisite task `P6-T02m` to track the missing continuation surface-resume -> owner dispatch contract.
- Updated `P6-T03` dependencies and blocker notes.
- Synced `TODO.md` with the new detailed task ordering.
