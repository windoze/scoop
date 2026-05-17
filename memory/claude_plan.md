## Execution Plan

I will complete exactly the first incomplete task from `TODO.md` and stop after committing the result.

1. Read `TODO.md` to identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit only for an explicitly unfinished issue that directly affects that task.
3. Read the task details, dependencies, validation requirements, and relevant project files.
4. Implement the task as specified, without narrowing scope or using workaround behavior.
5. If a concrete prerequisite blocks correct implementation, update `TODO.md` with the minimum required prerequisite task, keep the original task incomplete, commit that bookkeeping, and stop.
6. Run the task-relevant tests and quality checks, fixing failures that are in scope.
7. Mark the completed task title in `TODO.md` with `[DONE]` and update its completion record.
8. Update this plan file at key milestones or if the plan changes.
9. Commit all changes for this task with a clear task-tagged commit message.
10. Stop without starting the next task.

## Progress

- Initial execution plan recorded before inspecting repository state.
- Identified the first incomplete task as `P10-T02`: remove `__scoop_thread_spawn_join_resume*` and related runtime/compiler entries.
- Checked the latest commit (`f73e435a [P10-T01] Complete atomic surface migration audit`); it does not mention an unfinished issue that changes the `P10-T02` order.
- Located the active implementation surface: sysroot declarations, C runtime thread helpers/API export list, LLVM runtime symbol/ABI declarations, HIR/MIR lowering hooks, typecheck cross-thread resume policy, MIR transport metadata, codegen gap inventory, LLVM codegen tests, sysroot overlay fixtures, and seven direct cross-thread resume fixtures.
- Decision: remove the helper and its compiler transport support completely; delete fixtures whose tested behavior is cross-thread resume, because that helper surface is intentionally retired and will be redesigned with thread/sync later.
- Removed the sysroot declarations, overlay declarations, C runtime helper implementations/API entries, compiler runtime ABI/symbol declarations, HIR/MIR/effect-lowered hooks, MIR thread-resume payload metadata, the obsolete typecheck diagnostic gate, the LLVM transport unit test, codegen gap inventory entry, and direct cross-thread resume fixtures.
- Initial grep checks confirm the deleted helper names no longer appear in active `crates/`, `runtime/`, `sysroot/`, or `.scoop` fixtures.
- `cargo build` and `cargo clippy --all-targets -- -D warnings` passed. `cargo test --all --all-targets` exposed stale MIR golden output containing the removed `thread_resume_payload: None` field; updated those MIR goldens and will rerun tests.
- After updating MIR goldens and audit baselines, `cargo test --all --all-targets`, `cargo clippy --all-targets -- -D warnings`, and `cargo run -p scoop -- test` all pass. Full fixture suite result: 1338/1338 targets, 1375 checks.
- Marked `P10-T02` as `[DONE]` in `TODO.md` and `TODO-5.md`, including completion record, deletion justifications, validation results, PLAN closure, and no-failing-fixture status.

## Current Task Plan: P10-T02

1. Locate every use of `scoop_thread_spawn_join_resume`, `__scoop_thread_spawn_join_resume`, and related compat/transport names across `sysroot/`, `runtime/`, `crates/`, and `tests/fixtures/`.
2. Remove the two sysroot declarations from `sysroot/core.scoop` while preserving `Continuation` itself.
3. Remove the runtime C implementations and API X-macro entries for the spawn/join/resume helpers.
4. Remove compiler declarations or dispatch paths for the deleted runtime entries.
5. Classify affected fixtures: rewrite fixtures that only need single-thread `Continuation` behavior; delete fixtures whose only purpose is cross-thread resume helper coverage.
6. Run targeted grep validation to confirm no deleted helper names remain in `crates/`, `runtime/`, or `sysroot/`.
7. Run `cargo build` and the relevant fixture/full suite validation required by the task.
8. Mark `P10-T02` as `[DONE]` in `TODO.md` and `TODO-5.md`, recording changes, decisions, validation, PLAN closure, and failing-fixture status.
9. Commit all task changes with a `P10-T02` commit message, then stop.
