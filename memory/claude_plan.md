# Current Invocation Plan

## Scope

- Follow `TODO.md` as the authoritative ordered task list.
- Complete exactly the first task whose title is not prefixed with `[DONE]`, then stop.
- Do not skip review tasks or tasks with partial completion notes.
- Do not perform broad historical issue triage before selecting the current task.

## Execution Plan

1. Read `TODO.md` and identify the first incomplete task by title prefix.
2. Check the latest commit message only for unfinished issues directly relevant to that selected task.
3. Read the selected task body, dependencies, validation requirements, and completion-record expectations.
4. Inspect only the code and fixtures needed to implement or validate that task.
5. If the task is blocked by a concrete prerequisite or spec mismatch, update `TODO.md` with the minimum prerequisite task, commit that bookkeeping, and stop.
6. Otherwise, implement the task with the smallest correct code/documentation/fixture changes.
7. Run targeted validation first, then the task-required validation commands.
8. Fix any failures that are in scope for the selected task.
9. Update `TODO.md` by prefixing the selected task title with `[DONE]` and filling in its completion record.
10. Update this file when key steps complete or if the plan changes.
11. Commit all task-related changes with a descriptive task-tagged commit message.
12. Stop without starting the next task.

## Current Status

- `TODO.md` has been read.
- First incomplete task identified: `C4-T01D` (`新增 shared-state primitive fixtures`).
- Next step: check latest commit for directly relevant unfinished work and inspect current worktree status before editing.

## Selected Task Notes

- Add user-visible fixtures covering `RefCell`, `Box`, `AtomicInt`, `AtomicBool`, `Atomic<T: AnyRef>`, and `AtomicValue<T: AnyValue>`.
- Include bound reject coverage for `Atomic<Int>` and `AtomicValue<MyClass>`; existing reject fixtures may be reused if sufficient.
- Add LLVM/build-level atomic-ref fixture coverage for atomic instructions and GC barrier shape without overfitting unstable private symbol spelling.
- Required validation: `cargo run -p scoop -- test` and `cargo test -p scoopc atomic -- --nocapture`.

## Implementation Plan For C4-T01D

1. Reuse existing `sysroot_refcell_box_basic`, `closure_capture_refcell_make_counter`, `sysroot_box_value_assign_is_error`, and atomic bound reject fixtures where they already satisfy the task requirements.
2. Extend `sysroot_atomic_basic` to include CAS failure coverage for `AtomicInt`, `AtomicBool`, and `Atomic<Node>` pointer identity.
3. Add a typecheck reject fixture proving `AtomicValue<T>::cas` rejects `expected: T` and requires `expected: Box<T>`.
4. Add a build LLVM fixture proving public `Atomic<T: AnyRef>` emits pointer atomic load/store/CAS and invokes the GC barrier path.
5. Run targeted fixtures, then required `cargo test -p scoopc atomic -- --nocapture` and `cargo run -p scoop -- test`.
6. Update `TODO.md` completion record, commit all task-related changes, and stop.

## Progress

- Extended `sysroot_atomic_basic` with failed CAS coverage for `AtomicInt`, `AtomicBool`, and `Atomic<Node>`.
- Added `sysroot_atomic_value_cas_expected_requires_box.scoop` to reject `AtomicValue<T>::cas(expected: T, ...)`.
- Added `sysroot_atomic_ref_llvm.scoop` to check public `Atomic<T: AnyRef>` emits pointer atomic load/store/CAS plus GC write barrier usage.
- Targeted run-pass, typecheck, and build LLVM fixtures pass.
- `cargo test -p scoopc atomic -- --nocapture` passed.
- `cargo run -p scoop -- test` passed with `fixtures: ok (1405)`.
- `TODO.md` updated: `C4-T01D` is marked `[DONE]` with completion record.
- `cargo clippy --all-targets -- -D warnings` passed.
- Next step: inspect git diff/status, then commit task changes.
