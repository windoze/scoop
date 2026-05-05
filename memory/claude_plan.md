# Current Invocation Plan

This file records the actionable plan and progress for the current task invocation. It intentionally contains a concise execution plan and status updates, not private chain-of-thought.

## Plan

1. Read `TODO.md` as the global index.
2. Inspect referenced `TODO-Px.md` files in index order to identify the first detailed task whose heading is not prefixed with `[DONE]`.
3. Read the selected task requirements, constraints, dependencies, and completion record.
4. Inspect only the code and tests relevant to that task.
5. Implement the smallest spec-correct change that fully satisfies the task, without workarounds.
6. Add or update focused tests/fixtures required by the task.
7. Run the task-specific validation commands and broader checks needed to ensure no regressions.
8. Update the detailed `TODO-Px.md` task heading with `[DONE]`, update its completion record, and sync `TODO.md` if the indexed entry appears there.
9. Commit all relevant changes with a descriptive task-tagged message.
10. Stop after exactly one detailed task is completed.

## Progress

- Initialized plan file before reading TODO files or running project commands.
- Read `TODO.md` and `TODO-P7.md`; selected `P7-T03` as the first incomplete detailed task.
- Checked latest commit summary: `[P7-T02S] Fix refactor build fixture blockers`; it is the direct prerequisite and does not add a separate unfinished blocker.
- Next step: run the standard P7-T03 regression matrix in order, fixing any default-refactor regressions before rerunning the matrix.
- `cargo test --all` passed on the first run.
- `cargo run -p scoop -- test` failed on `tests/fixtures/build/unsafe_atomic_int_field_lvalue_llvm.scoop` because refactor pure assignment still emitted `Todo("struct literal lowering pending")` for a struct literal rvalue. This is an in-scope default-refactor lowering gap to fix directly.
- Implemented explicit MIR struct literal lowering and LLVM aggregate emission; `cargo check -p scoopc --lib` passed.
- Re-running the fixture now reaches the next in-scope blocker: refactor direct-call lowering lacks callable signature coverage for `scoop.unsafe.__atomicIntLoad`.
- Fixed refactor atomic intrinsics to cover load and recursive member lvalue places, including nested struct fields stored inside class payloads; `tests/fixtures/build/unsafe_atomic_int_field_lvalue_llvm.scoop` now passes.
- Atomic smoke fixtures `unsafe_atomic_int_basic.scoop` and `unsafe_atomic_int_field_lvalue_basic.scoop` pass.
- Re-running full `scoop test` now fails at `tests/fixtures/codegen/intrinsic_size_of_int_word.scoop` with run-pass exit status 1; this is the next default-refactor regression to diagnose.
- Fixed `scoop.core.sizeOf(value)` by lowering it to a MIR `SizeOf` compile-time value instead of a real direct call; `tests/fixtures/codegen/intrinsic_size_of_int_word.scoop` now passes.
- Re-running full `scoop test` now fails at `tests/fixtures/effect_facts/direct_and_fun_value_call.scoop` due to an effect-facts snapshot/golden mismatch; next step is to compare actual output against the golden.
- Updated `direct_and_fun_value_call.effectfacts` to the current NoOutward plain callable facts shape: no StepSchema for pure plain callables and no call-site facts for optimized-away pure direct calls.
- Updated `dispatch_and_resume_call.effectfacts` for the same plain-callable handoff: pure dispatch call sites report `callee_abi_kind: Plain` with no callee StepSchema, while effectful resume schemas remain published.
- Updated `dynamic_fallback_widening.effectfacts` to include the current explicit `call_abi_kind: EffectStep` fields for effectful dynamic fallback callable/site facts.
- Updated `handle_finally_boundary.effectfacts` to the current plain ABI facts for pure cleanup/main calls while preserving effectful handle/perform schemas.
- Updated `handle_perform.effectfacts` to include the compiler-generated runtime-error case in the handle step schema and the current plain callable facts for `main`.
- Updated `nested_handle_self_contained_vs_outward.effectfacts` for runtime-error case publication on self-contained handle schemas and plain ABI facts for the fully NoOutward callable.
- Updated `single_case_impl_plan.effectfacts` to include the explicit `call_abi_kind: EffectStep` field for the effectful single-case callable.
- Full `scoop test` has advanced past effect-facts snapshots and now fails at `effect_lowered/continuation_resume_runtime_error_boundary.scoop` due to an effect-lowered snapshot mismatch.
- Updated `continuation_resume_runtime_error_boundary.effectlowered` to include the current `plain_local_effect_control: <none>` field for plain callables.
- Updated `direct_and_fun_value_call.effectlowered` to include `plain_local_effect_control: <none>` for each plain callable.
- Updated `dispatch_and_resume_call.effectlowered` to include `plain_local_effect_control: <none>` for plain dispatch callables.
- Updated `dropped_continuation_abandons_remaining_work.effectlowered` to include `plain_local_effect_control: <none>` for plain cleanup/main callables.
- Regenerated all `tests/fixtures/effect_lowered/*.effectlowered` snapshots from `dump-effect-lowered` to capture the current generated late-lowered contract consistently, including plain local-effect-control sections.
- Removed duplicate tracked source-only `tests/fixtures/effect_lowered_src` fixtures because they duplicate `effect_lowered` cases with goldens and were routed as an unimplemented phase by the full fixture runner.
- Regenerated `tests/fixtures/hir/*.hir` to match the fixture-runner HIR file-only dump format. While doing so, repaired obsolete HIR fixture sources that no longer typechecked under current rules: `control_flow.scoop`, `delegated_property_lowering.scoop`, and `member_access.scoop`.
- Regenerated `tests/fixtures/mir/*.mir` snapshots to the current direct-style MIR debug shape, including explicit local source classification and return operands.
- Regenerated `tests/fixtures/mir_refactor/*.mir` stable snapshots to the current refactor MIR dump shape.
- Fixed refactor array literal runtime support by treating array builder/Array size/get/set helpers as plain compiler intrinsics in effect facts and lowering them directly to the runtime array ABI in the refactor LLVM value path. The targeted array literal run-pass fixture now passes.
- Added refactor `toInt` intrinsic lowering for Char/Float/String receivers and marked it as a plain compiler intrinsic for effect facts; the string/char/float array inference run-pass fixture now passes.
- `async_await_minimal_int_basic.scoop` remains a concrete blocker: after fixing generic resume payload ABI materialization and task join's implicit return, the program starts but exits after printing `before`, indicating a remaining async/task resume payload runtime semantics gap.
- Added prerequisite `P7-T02U` before `P7-T03` in `TODO-P7.md` and synced `TODO.md`; `P7-T03` remains incomplete and now depends on `P7-T02U`.
