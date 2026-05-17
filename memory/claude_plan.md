# Claude Plan

## Scope
- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that one task, or add the minimum prerequisite task if a concrete blocker prevents spec-correct implementation.

## Execution Plan
1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check the latest commit only for an explicitly mentioned unfinished issue directly relevant to that task.
3. Inspect the relevant code, fixtures, and documentation for that task.
4. Implement the smallest spec-correct change needed for the task.
5. Add or update tests/fixtures required by the task.
6. Run targeted validation first, then any broader validation required by the task.
7. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling its completion record.
8. Update this file with implementation and validation progress.
9. Commit all task-related changes with a clear task-prefixed message.
10. Stop without starting the next task.

## Status
- Plan initialized before running project commands.
- First incomplete task identified: `P10-T01` (`__AtomicInt` series moves from `scoop.core` to `scoop.unsafe`).
- Latest commit checked: `[P9-T03] Remove stdlib injection paths`; no explicit unfinished issue for this task.
- Existing unrelated worktree item noted: untracked `CLOSURE_FIX.md`; it will not be modified or committed for this task.

## Current Task Plan: P10-T01
1. Locate every `__AtomicInt` / `__atomicInt*` declaration, intrinsic dispatch entry, and fixture use.
2. Move the typealias and three intrinsic declarations from `sysroot/core.scoop` to the reserved internal atomics section in `sysroot/unsafe.scoop`.
3. Update any callers to import `scoop.unsafe.*` or otherwise reference the new source package.
4. Update compiler intrinsic FQN dispatch from `scoop.core` to `scoop.unsafe` without changing lowering behavior.
5. Run targeted atomic fixture validation and then the required full fixture suite.
6. Update `TODO.md` and `TODO-5.md` completion records, then commit only task-related changes.

## Progress
- Located current atomic surface: declarations are already in `sysroot/unsafe.scoop`, and `sysroot/core.scoop` has no `__AtomicInt` / `__atomicInt*` declaration block.
- Located fixture users: `unsafe_atomic_int_basic`, `unsafe_atomic_int_field_lvalue_basic`, `unsafe_atomic_int_top_level_storage_llvm`, `unsafe_atomic_int_field_lvalue_llvm`, and `gc_stw_cross_thread_roots_basic`; visible fixtures already import `scoop.unsafe.*`.
- Located compiler dispatch/lowering users: current FQNs are `scoop.unsafe.__AtomicInt` and `scoop.unsafe.__atomicInt*`, preserving SeqCst LLVM atomic lowering.
- Targeted atomic owner fixtures passed individually: two run-pass fixtures, two LLVM build fixtures, and `runtime_gc/gc_stw_cross_thread_roots_basic.scoop`.
- Full fixture suite passed: `cargo run -p scoop -- test` reported 1345/1345 targets and 1382 checks OK.
- Clippy passed: `cargo clippy --all-targets -- -D warnings`.
- `TODO.md` and `TODO-5.md` were updated to mark `P10-T01` as `[DONE]` with completion notes.
