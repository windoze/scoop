# Claude Execution Plan

## Scope

- Work on exactly the first incomplete task in `TODO.md`.
- Treat a task as complete only when its heading is prefixed with `[DONE]`.
- Do not proceed to the next task after completing or blocking the current one.

## Plan

1. Read `TODO.md` and identify the first incomplete task.
2. Check the latest commit only for directly relevant unfinished work that affects that task.
3. Inspect the relevant source, tests, fixtures, and documentation for the selected task.
4. Implement the smallest spec-correct change needed for that task, without workarounds or fixture-only hacks.
5. Add or update focused tests and fixtures required by the task.
6. Run the task-specified validation commands and any directly relevant checks.
7. If the task is complete, update `TODO.md` by prefixing the task heading with `[DONE]` and filling in the completion record.
8. If a concrete blocker prevents correct implementation, add the minimum prerequisite task to `TODO.md`, keep the current task incomplete, record the blocker here, commit, and stop.
9. Commit all task-related changes with a descriptive message, then stop.

## Progress Log

- Initial plan recorded before reading `TODO.md` or running project commands.
- Selected current task: `P3-T03` (`mutableArrayNew<T>`, `MutableArray<T>.push`, `MutableArray<T>.freeze` sysroot wrappers).
- Latest commit check: `ced1dae9 [P3-T02] Split array intrinsics by layout`; no directly relevant unfinished issue was mentioned in the commit subject.

## Current Task Execution Plan

1. Inspect `sysroot/core.scoop`, `sysroot/unsafe.scoop`, existing `MutableArray` declarations, extern annotation syntax, unsafe primitive patterns, and relevant tests.
2. Determine whether plain `as` casts can express the required generic erase/cast and value-to-word conversions; if not, add general unsafe intrinsic substrate rather than wrapper-specific compiler hacks.
3. Add `@Extern(abi = "scoop")` runtime declarations and ordinary generic wrappers in `sysroot/core.scoop`.
4. Add focused owner fixtures for word, ref, composite, grow, and freeze behavior.
5. Run targeted fixtures, fix any implementation or type/lowering/runtime issues, then run full baseline and lint/test checks required by the task.
6. Mark `P3-T03` `[DONE]` in `TODO.md` and `TODO-1.md` with completion record if validation passes.
7. Commit all related changes and stop.

## Current Task Findings

- The old stdlib `MutableArray<Int>.push(value): MutableArray<Int>` conflicts with the required in-place `MutableArray<T>.push(value): Unit` surface, so stdlib call sites must be adjusted to the new semantics for baseline to remain valid.
- The existing `descOf<T>()` codegen returns `0` for every type; this is sufficient for non-composite arrays but directly blocks `MutableArray<struct>` because the runtime requires a composite transport descriptor for `SCOOP_ARRAY_ELEM_KIND_COMPOSITE`.
- The declared raw pointer surface (`stackAlloc` / `Ptr.store` / `ptrToUIntPtr`) is not currently lowered in the LLVM backend, and `Ptr<T>` is gated to GC-free pointees. The implementation will add general unsafe named intrinsics for generic erase/cast, value-to-word, value-to-Any, and immediate value-slot address materialization instead of adding a MutableArray-specific compiler special case.

## Completion Update

- Implemented `P3-T03` and marked it `[DONE]` in `TODO.md` and `TODO-1.md`.
- Added sysroot wrappers, unsafe named intrinsic substrate, runtime reflection codegen for generic `kindOf` / `alignOf` / `descOf`, owner fixtures, and stdlib/overlay updates needed by the new in-place push surface.
- Validation passed: owner fixtures, `cargo run -p scoop -- test` (`fixtures: ok (1374)`), `cargo test --all --all-targets` (840 tests), and `cargo clippy --all-targets -- -D warnings`.
- Next required action: commit all task-related changes and stop.
