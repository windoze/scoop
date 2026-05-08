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

- Current task: `CG-T07S0a19`
- Goal: fix `tests/fixtures/run-pass/stdlib_string_methods_extended.scoop` failure where `String.isEmpty()`, `replace(...)`, `charAt(...)`, and `repeat(...)` builtin member calls still degrade into unresolved `MemberAccess` plus `CallKind::FunValue` instead of authoritative typed direct/member call lowering.
- Latest commit message checked: `[CG-T07S0a18] Close String support-source intrinsic call blocker`. The commit body explicitly records that full-suite now advances to `stdlib_string_methods_extended.scoop`; this is already tracked as the next prerequisite task `CG-T07S0a19` in `TODO.md`, so no extra task insertion is needed before execution.

## Task-Specific Plan

1. Reproduce the `stdlib_string_methods_extended.scoop` failure with the task-specified build command.
2. Inspect the current String builtin member typecheck/codegen paths and compare them with the already-fixed `byteLength/getByte/unsafeSliceBytes` extension-style contract.
3. Publish authoritative receiver-prefixed extension/direct-call metadata for `isEmpty`, `replace`, `charAt`, and `repeat` in typecheck so HIR/MIR/materialized MIR stop emitting unresolved member callee values.
4. Add the matching legacy dispatch/codegen and refactor direct-call lowering hooks for `scoop.core.isEmpty`, `scoop.core.replace`, `scoop.core.charAt`, and `scoop.core.repeat`.
5. Add a focused compiler regression test that proves the affected member calls lower to direct calls instead of `FunValue` calls.
6. Run task validation: focused compiler test, fixture build/test, default full-suite, and `cargo clippy --all-targets -- -D warnings`.
7. If validation uncovers a new blocker, record the minimum prerequisite in `TODO.md`; otherwise mark `CG-T07S0a19` as `[DONE]`, update its completion record, commit, and stop.

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
- 2026-05-08 current invocation: re-read `TODO.md`; confirmed `CG-T07S0a18` is already marked `[DONE]`, the worktree is clean, and the first incomplete task is now `CG-T07S0a19`.
- Read the `CG-T07S0a19` task body, the latest commit message, and the current String builtin code paths. Initial inspection shows the same pattern as `CG-T07S0a18`: `isEmpty` / `replace` / `charAt` / `repeat` are typed in `typecheck/expr/call.rs` but do not yet publish authoritative member resolution/call-arg binding, and neither legacy dispatch nor refactor direct-call lowering has the corresponding `scoop.core.*` extension handlers.
- Reproduced the `CG-T07S0a19` failure with `cargo run -p scoop -- build tests/fixtures/run-pass/stdlib_string_methods_extended.scoop -o /tmp/stdlib_string_methods_extended`; it failed in frontend prepare with `refactor plain function-value callee type`, confirming the unresolved member-call contract regression.
- Implemented the fix: typecheck now publishes receiver-prefixed extension-style call contracts for `String.isEmpty`, `String.replace`, `String.charAt`, and `String.repeat`; legacy LLVM dispatch, refactor direct-call lowering, and effect-facts plain-intrinsic classification were updated to consume the same synthetic `scoop.core.*` direct-call FQNs.
- Added focused regression coverage in `crates/scoopc/src/llvm/tests.rs` to assert HIR/materialized MIR lower these four String member calls to direct calls and no longer emit `FunValue` callees.
- Validation passed for the task-targeted work: `cargo test -p scoopc builtin_string_member_calls_lower_to_direct_calls -- --nocapture`, `cargo test -p scoopc builtin_string_intrinsic_member_calls_lower_to_direct_calls -- --nocapture`, `cargo run -p scoop -- build tests/fixtures/run-pass/stdlib_string_methods_extended.scoop -o /tmp/stdlib_string_methods_extended`, `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/stdlib_string_methods_extended.scoop`, and `cargo clippy --all-targets -- -D warnings` all succeeded.
- Full-suite validation advanced past `stdlib_string_methods_extended.scoop`; `cargo run -p scoop -- test` now fails later at `tests/fixtures/run-pass/string_trim_indent_basic.scoop`.
- Reproduced the new blocker with `cargo run -p scoop -- build tests/fixtures/run-pass/string_trim_indent_basic.scoop -o /tmp/string_trim_indent_basic`; it fails with the same `refactor plain function-value callee type` shape, indicating the remaining `String.trimIndent()` builtin member call still lacks an authoritative direct-call contract.
- Updated `TODO.md`: marked `CG-T07S0a19` as `[DONE]`, recorded the implementation and validation, inserted the new prerequisite task `CG-T07S0a20` for the `trimIndent()` blocker, and updated `CG-T07S0a` dependencies accordingly.
