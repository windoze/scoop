# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Do not proceed to any later task after finishing or blocking the selected task.

## Execution Plan

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check the latest commit message for any unfinished issue directly relevant to that task.
3. Read only the project files needed to understand and implement the selected task.
4. Implement the task without workarounds or scope narrowing.
5. Run the task-specific validation commands, plus broader checks if the task or repository guidance requires them.
6. If a concrete blocker prevents correct implementation, update `TODO.md` with the minimum prerequisite task, commit that bookkeeping, and stop.
7. If implementation succeeds, update `TODO.md` by prefixing the selected task title with `[DONE]` and refreshing its completion record.
8. Commit all relevant changes with a task-specific message, then stop.

## Progress Log

- Started by recording this plan before inspecting the task list.
- Identified `P6-T03` as the first incomplete task from `TODO.md`.
- Read `TODO-6.md` task details. Latest commit is `[P6-T02R] Review eager top-level init order`, whose remaining `scoop_once` split is directly in scope for `P6-T03`.

## P6-T03 Working Plan

1. Inspect current object-init, top-level global/TLS codegen, member access, runtime once helper, and HIR/typecheck storage gates.
2. Confirm where `scoop_once` is still used by top-level eager init and where storage policy can escape to backend panic or undefined behavior.
3. Make the smallest correct implementation changes so once helpers are object-only, top-level eager init uses storage-specific codegen, and unsupported TLS behavior is rejected at the semantic barrier if not implemented.
4. Add or update focused fixtures/tests for object recursion/self access, `@Global`, entry-thread `@ThreadLocal`, unsupported non-entry-thread TLS policy, and `@Extern` storage rules as required by the task.
5. Run the P6-T03 validation matrix and fix any task-relevant failures.
6. Mark `P6-T03` `[DONE]` in `TODO.md` and `TODO-6.md`, record validation results, commit, and stop.

## P6-T03 Progress

- Replaced top-level immutable eager init's runtime `scoop_once` calls with a compiler-private guard state machine.
- Added a generated `scoop_thread_init_current` TLS hook and wired `scoop.thread` worker entry to call it when present.
- Added storage-policy conflict rejection for simultaneous `@ThreadLocal` and `@Global` on top-level storage roots.
- Added focused run-pass/typecheck/LLVM test coverage for TLS worker initialization, object-once separation, and top-level eager init not using runtime once.
- Validation progress: `cargo fmt`, `cargo test -p scoopc --no-default-features storage_policy`, typecheck fixtures, global-init fixtures, `cargo test -p scoop_runtime --all-targets`, and the two new LLVM separation tests pass. The first runtime run exceeded a 20-minute tool timeout at `stackmap_registry`; a standalone run and a full rerun with a longer timeout both passed.
- Marked `P6-T03` as `[DONE]` in `TODO.md` and `TODO-6.md` with completion notes and validation record.
