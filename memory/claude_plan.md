# Claude Plan

## Working Notes

I will keep this file updated with a concise execution plan, progress, and any blockers. I am not recording private chain-of-thought here, but I will maintain a complete actionable log of steps taken and decisions made.

## Initial Plan

1. Read `TODO.md` to identify the first task whose title is not prefixed with `[DONE]`.
2. Read the selected task entry carefully, including requirements, dependencies, and validation expectations.
3. Check the latest commit message to see whether it mentions unfinished work that is directly relevant to the selected task.
4. Inspect only the code and documents needed for the selected task.
5. Implement the task completely, keeping changes minimal and spec-correct.
6. Run the relevant validation commands for the task, plus required repo-wide quality gates if they are in scope.
7. If a concrete blocker prevents correct completion, update `TODO.md` with the minimum prerequisite task(s), adjust dependencies/order, update this file, commit, and stop.
8. If the task is completed, update `TODO.md` to mark the task title with `[DONE]` and fill in its completion record.
9. Update `PLAN.md` only if the phase-level plan or dependencies change.
10. Commit all required changes with a task-scoped commit message, then stop.

## Selected Task

- Current task: `CG-T07S0a18`
- Goal: fix `tests/fixtures/run-pass/stdlib_string_basic.scoop` failure where `sysroot/string.scoop` support-source String intrinsic member calls such as `byteLength()`, `getByte()`, and `unsafeSliceBytes()` degrade into unresolved `MemberAccess` plus `CallKind::FunValue` instead of authoritative typed member/intrinsic call lowering.
- Latest commit subject checked: `[CG-T07S0a17] Close star-projection array read-view blocker`. No directly stated unfinished follow-up for the new `String` blocker was found in that subject alone.

## Task-Specific Plan

1. Reproduce the failure with the task-specified fixture command.
2. Inspect `sysroot/string.scoop` and the lowering pipeline artifacts/tests around String intrinsic support-source calls.
3. Identify the earliest stage where authoritative resolution/type information is lost.
4. Implement the minimal spec-correct fix in the responsible stage(s), without backend guessing or fixture reshaping.
5. Add or update focused regression coverage for at least `byteLength()` and one of `getByte()` / `unsafeSliceBytes()`.
6. Run the task validation commands, plus any targeted compiler tests needed for confidence.
7. Update `TODO.md` completion record and task title if the task is fully complete; otherwise record the blocker and prerequisite ordering.
8. Commit all required changes with a `CG-T07S0a18`-scoped message and stop.

## Progress

- Plan file created before repository inspection.
- Read `TODO.md` and identified the first incomplete task as `CG-T07S0a18`.
- Read the full `CG-T07S0a18` entry and checked the latest commit subject for directly relevant unfinished work.
- Reproduced the task failure with `cargo run -p scoop -- build tests/fixtures/run-pass/stdlib_string_basic.scoop -o /tmp/stdlib_string_basic`.
- Confirmed the same underlying issue also affects `tests/fixtures/run-pass/string_byte_accessors.scoop`.
- Root cause identified: the early typecheck branch for built-in String methods returns types for `byteLength`, `getByte`, and `unsafeSliceBytes` but does not publish member resolution or call-arg binding, so HIR/MIR preserve them as unresolved member values and the call path degrades to `CallKind::FunValue`.
- Next edit: publish explicit call contracts for these String methods in typecheck, route lowered calls through synthetic direct extension-style FQNs, add direct-call codegen handlers, and add regression tests for the lowered call shape.
- Implemented the fix: published receiver-prefixed extension-style call contracts for `String.byteLength`, `String.getByte`, and `String.unsafeSliceBytes`; added direct-call handling in both LLVM codegen paths; added a focused HIR/MIR regression test.
- Validation passed for the focused work: the new compiler test passed; `string_byte_accessors.scoop`, `string_unsafe_slice_bytes.scoop`, and `stdlib_string_basic.scoop` all build/test successfully; `cargo clippy --all-targets -- -D warnings` passed.
- Full suite result: `cargo run -p scoop -- test` no longer stops at `stdlib_string_basic.scoop` and now advances to a new blocker at `tests/fixtures/run-pass/stdlib_string_methods_extended.scoop`.
- Follow-up bookkeeping required: mark `CG-T07S0a18` done in `TODO.md`, add the new prerequisite task for the `stdlib_string_methods_extended.scoop` blocker, update `CG-T07S0a` dependencies, then commit and stop.
