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
