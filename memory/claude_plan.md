# Execution Plan

## Scope

- Work from `TODO.md` as the authoritative task list.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Stop after implementing, validating, documenting, and committing that one task.

## Steps

1. Read `TODO.md` and identify the first incomplete task by heading prefix.
2. Check the latest commit only for an unfinished issue directly relevant to that task.
3. Inspect the affected code and fixtures needed for the selected task.
4. Implement the task without narrowing scope or introducing workarounds.
5. Add or update tests/fixtures required by the task.
6. Run the validation commands specified by the task and any directly relevant test commands.
7. If a concrete blocker prevents spec-correct implementation, update `TODO.md` with the minimum prerequisite task, commit that bookkeeping, and stop.
8. If implementation succeeds, mark the task heading `[DONE]`, update its completion record, and avoid changing `PLAN.md` unless phase-level sequencing changed.
9. Commit all relevant changes with a task-specific message.

## Progress Log

- 2026-05-16 invocation started. Per policy, this file records an auditable execution plan and progress summary rather than private chain-of-thought.
- Immediate plan: read `TODO.md` to identify the first incomplete heading, check the latest commit only for directly relevant unfinished work, inspect the necessary code/tests, implement the task exactly as written, run required validation, update `TODO.md`, commit, and stop.
- Initial plan recorded before reading project task details.
- Selected task: `P4-T01l2` from `TODO.md`正文 heading（索引中部分状态已过期，正文 heading 为准）。Latest commit `[P4-T01l] Split into P4-T01l1 / P4-T01l2 / P4-T01l3` is directly relevant and confirms this task is the intended next unit.

## Current Task Plan: P4-T01l2

1. Inspect HIR lowering for member calls and canonical call receiver binding.
2. Inspect sysroot/type kind collection to confirm builtin intrinsic scalar declarations are visible in all relevant HIR paths.
3. Add a focused sysroot overlay fixture for builtin scalar `@Intrinsic` body method receiver injection.
4. Implement the smallest fix so `binding.params[0] = Receiver` cannot be silently dropped when lowering builtin scalar direct member calls.
5. Run the new fixture, targeted tests, full relevant fixture phases, `cargo test -p scoopc`, and clippy.
6. Mark `P4-T01l2` `[DONE]` with completion record, then commit all task changes.

## Current Progress

- Confirmed the risky path: fallback canonical call lowering currently passes `receiver: None`; if the recorded `CallArgBinding` contains `Receiver`, `lower_canonical_call_expr` returns `None` and the later generic fallback can emit a direct call without the receiver argument.
- Planned fix: preserve existing direct member lowering, but make fallback canonical lowering reconstruct the receiver from `MemberAccess` when the binding requires it.
- Implemented the fallback receiver reconstruction and added `intrinsic_sysroot_overlay_scalar_method_basic` plus the LLVM owner test `overlay_core_intrinsic_scalar_body_method_call_keeps_receiver_arg`.
- `cargo fmt --all` also normalized two pre-existing long-line formatting hunks in `mir/materialize.rs` and `resolve/scopes.rs`.
- Validation passed: `cargo fmt --all`; new build fixture; new owner test; full `cargo test -p scoopc`; full run-pass and typecheck fixture phases; `cargo clippy --all-targets -- -D warnings`.
- `TODO.md` now marks `P4-T01l2` as `[DONE]` and records the completion details. Next required task for a future invocation is `P4-T01l3`.

## Current Task Plan: P4-T01l3

1. Confirm the latest commit is directly relevant and does not introduce a separate unfinished prerequisite.
2. Reproduce the `published late-lowered body` failure with a focused sysroot overlay fixture for scalar `toString` direct calls and `println(<scalar>)` interface dispatch.
3. Inspect monomorphization, reachability, late-lowered body publication, and existing `ToString` by-name intercepts to locate where `scoop.core.ToString.toString` becomes reachable without a published body.
4. Implement the smallest class-wide fix that publishes the required callable/default/interface body path for builtin scalar overrides without deleting transitional by-name intercepts or changing dispatch ABI.
5. Add the requested owner fixture and any focused Rust owner test needed to lock the late-lowering publication path.
6. Run the task validation: focused fixture/test, `cargo test -p scoopc`, run-pass/typecheck fixtures, `cargo clippy --all-targets -- -D warnings`.
7. Mark `P4-T01l3` `[DONE]` in `TODO.md`, record completion details, commit all relevant changes, and stop.

## Current Progress: P4-T01l3

- Selected `P4-T01l3` as the first incomplete task from `TODO.md`正文 heading.
- Latest commit `8f1e676b [P4-T01l2] Preserve builtin scalar member receivers` is directly relevant as the completed dependency and does not name a separate unfinished issue.
- Added the requested scalar `toString` sysroot overlay build fixture and reproduced the target failure: LLVM emit reports missing published late-lowered body for `scoop.core.ToString.toString`.
- Implemented reachability/materialization fixes: pass-view MIR interface calls now enqueue itable dispatch candidates, and newly materialized generic instance bodies are recursively scanned so scalar `toString` overrides/default bodies become published late-lowered callables.
- Focused validation passed: new build fixture and new LLVM owner test.
- Full validation passed: `cargo fmt --all`; new build fixture; `cargo test -p scoopc` (863 passed); full run-pass fixture suite (400 passed); full typecheck fixture suite (437 passed); `cargo clippy --all-targets -- -D warnings`.
- `TODO.md` now marks `P4-T01l3` as `[DONE]` and records completion details. Ready to commit this single task.
