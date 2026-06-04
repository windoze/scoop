# Claude Execution Plan

## Scope

- Work on exactly the first incomplete task in `TODO.md`.
- Treat a task as complete only if its heading is prefixed with `[DONE]`.
- Stop after completing that task or after recording and committing a concrete blocker/prerequisite.

## Plan

1. Read `TODO.md` and identify the first task heading that is not prefixed with `[DONE]`.
2. Check recent Git context only as needed for the selected task, including whether the latest commit mentions an unfinished issue directly relevant to it.
3. Inspect the code, tests, fixtures, and documentation relevant to the selected task.
4. Implement the selected task without workarounds or spec deviations.
5. Add or update the smallest relevant tests/fixtures for the implemented behavior.
6. Run formatting, linting, and the required validation in this order: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, then full tests/fixtures as applicable.
7. If validation exposes an unscheduled failure, fix it or add the minimum prerequisite/follow-up task in `TODO.md` before marking the current task done.
8. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling its completion record. Update `PLAN.md` only if phase-level planning changes.
9. Update this file after key milestones or plan changes.
10. Inspect Git status/diff/log, commit all intended changes with a task-specific message, and stop.

## Current Status

- Plan initialized before reading `TODO.md` or running project commands.
- First incomplete task identified: `TC-02-PRE2：收敛 plain-LIR 剩余 fixture/runtime 残差`.
- Git context checked: latest commit is `[TC-02] Advance plain LIR lowering`, directly relevant to this task; working tree only had this plan file modified.
- Reproduced representative failures:
  - `effect_lowered_member_codegen_emit_llvm.scoop` now emits equivalent LIR names (`lir_member_load`) while the fixture still expects old `pass_mir_*` names.
  - `array_lit_infer_string_char_float_basic.scoop` fails at link time because LIR plain direct calls still emit `scoop.core.Float64.toInt` as a normal callable instead of routing to the existing Float.toInt runtime lowering.
- Current edit focus: add a LIR-compatible named intrinsic route for float `toInt`, then re-run the affected runtime fixtures and update stale LLVM substring fixtures only where the new assertions remain semantically equivalent.
- Implemented `float_to_int` named intrinsic routing for `Float32/Float64.toInt` and reused existing Float.toInt runtime lowering from LIR plain calls.
- Targeted check passed: `python3 tools/run_fixtures.py tests/fixtures/run-pass/array_lit_infer_string_char_float_basic.scoop --exit-on-failure -j 1`.
- Full fixture suite after the first fix reported 27 remaining failures.
- Implemented LIR plain handling for top-level function values/top-level `FunPtr` direct calls and `scoop.unsafe.invoke` lowering.
- Targeted checks now pass for `callable_value_pattern_binder_receiver_named_args_basic.scoop`, `top_level_callable_value_call_basic.scoop`, `unsafe_funptr_extern_call_basic.scoop`, `unsafe_funptr_receiver_call_basic.scoop`, and `unsafe_funptr_aggregate_return_tuple.scoop`.
- Fixed namespace-only top-level refs, external/bodyless direct-call result type inference, LIR `GC.pin/unpin`, recursive LIR atomic lvalue resolution, top-level atomic storage, and cross-cone public callable ABI symbol stability.
- Additional targeted checks now pass: enum value-only when, extension/computed properties, companion object namespace access, safe member access, GC pin/unpin, cross-thread GC, atomic lvalue runtime/top-level IR, and the three dependency cone fixtures.
- Updated stale LLVM substring/golden fixtures for LIR naming, atomic nested GEP regex, platform struct literal IR, and MIR intrinsic-callable count.
- Targeted rechecks for those fixtures pass.
- Full validation baseline passed: `cargo fmt`; `cargo clippy --all-targets -- -D warnings`; `cargo test --all --all-targets`; `cargo build -p scoop -p scoopc`; `python3 tools/dependency_gate.py`; `python3 tools/spec_fixtures.py check`; `python3 tools/run_fixtures.py`.
- `TODO.md` updated: `TC-02-PRE2` marked `[DONE]` with completion record.
- Next step: inspect Git status/diff/log, commit intended changes, and stop.
