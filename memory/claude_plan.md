# Claude Execution Plan

## Scope

- Complete exactly the first incomplete detailed task from the TODO index and then stop.
- Use `TODO-Px.md` files as the source of truth for task completion state.
- Keep `TODO.md` synchronized with any task title/status changes.
- Do not update `PLAN.md` unless phase-level sequencing or completion criteria actually change.

## Steps

1. Read `TODO.md` as the global index.
2. Read the referenced `TODO-Px.md` files in task order.
3. Select the first detailed task whose heading is not prefixed with `[DONE]`.
4. Inspect only the code and tests needed for that task.
5. Implement the task without workarounds or spec deviations.
6. Add or update focused tests/fixtures required by the task.
7. Run the relevant validation commands, and fix failures that are in scope.
8. Mark the task `[DONE]` in its detailed `TODO-Px.md` file and update its completion record.
9. Sync `TODO.md` if task titles, ordering, or completion markers changed.
10. Commit all task-related changes with a descriptive task-tagged message.
11. Stop without starting the next task.

## Progress

- Plan initialized before task discovery.
- First incomplete detailed task identified: `P7-T03` in `TODO-P7.md`.
- Current task requirement: run the default-refactor standard full regression matrix, fix any default-path regressions, then mark `P7-T03` complete and commit.
- Latest commit checked: `538b59db [P7-T02Y] Fix nested continuation replay`; no directly relevant unfinished issue was recorded in the commit message.
- Worktree already contains uncommitted changes in compiler/refactor implementation files plus `memory/claude_plan.md`; inspect diffs before editing so task work does not overwrite unrelated changes.
- Validation progress: `cargo test --all` passed on the current worktree.
- Next validation step: run `cargo run -p scoop -- test` under the default refactor pipeline and fix any fixture regression it exposes.
- 2026-05-06 resume: continuing the same first incomplete task, `P7-T03`; `TODO.md` and `TODO-P7.md` agree that `P7-T03` is the first heading without `[DONE]`.
- User note: some run-pass fixtures may hang, so run run-pass fixtures individually with a small timeout (30 seconds) when isolating or rechecking fixture failures.
- Immediate plan: inspect the existing uncommitted diff from the interrupted P7-T03 work, run targeted validation from the recorded next step, fix any default-refactor regressions without legacy fallback, then rerun the required matrix and commit all resumed-task changes.
- `cargo test --all` passed on 2026-05-06 with the resumed worktree.
- Next: run `tests/fixtures/run-pass/*.scoop` one file at a time with a 30s timeout to identify any remaining hanging or failing default-refactor fixtures before attempting broader `scoop test` coverage.
- First individual run-pass sweep completed without timeouts but exposed many shared default-refactor failures. Root cause identified from `fun_call_add_basic.scoop` / `var_assign_basic.scoop`: materialized MIR can carry builtin scalar sysroot nominal types such as `scoop.core.Int32`, and MIR codegen was treating them as ordinary structs; if-expression compiler temporaries typed as `Any` also boxed branch ints before returning `Int`.
- Implemented builtin nominal scalar ABI mapping plus compiler-temporary slot inference for concrete assigned values; `fun_call_add_basic.scoop` and `var_assign_basic.scoop` now pass through `scoop test --fixtures`.
- Additional shared fixes: static enum unit-variant member access (`EnumName.Variant`) now lowers as an enum constant instead of an instance field; direct-call result temporaries can infer callee return ABI; effect runtime slot intrinsics are treated as plain compiler intrinsics and lowered through the refactor MIR direct-call path. Targeted fixtures now passing: `enum_value_only_when_basic.scoop`, `extension_property_getter_basic.scoop`, `effect_runtime_slot_abi_basic.scoop`.
- More shared fixes landed while reducing the prior failure set: pass-MIR shifts now mask shift counts like the legacy expression path; string equality, mixed-width float comparisons, Float `abs/isNaN/isInfinite`, f-string parts with stale `Any` expression types, tuple-get result locals, static enum payload sources, and pure function-value calls inside effect-step source slices now lower through generic contracts. Targeted fixtures now passing include `int_bitops_shift.scoop`, `string_equality_basic.scoop`, `float_literal_runtime_basic.scoop`, `float_literal_other_contexts_basic.scoop`, `with_update_tuple_nested_single_eval_basic.scoop`, `enum_function_payload_basic.scoop`, `enum_function_payload_boxed_multi_field_basic.scoop`, and `enum_variant_non_scalar_payload_basic.scoop`.
- Handle/function-return contract progress: non-Unit handle body completion can use enclosing-function `Return` payloads when there is no normal handle-body completion, while normal completion remains preferred when both paths exist; targeted `effect_handle_return_from_function_basic.scoop`, `effect_handle_return_from_function_finally.scoop`, and `effect_handle_return_from_function_nested_handle.scoop` now pass.
- Added a first-class MIR `StoreTopLevelVar` statement and threaded it through MIR analyses/materialization/refactor codegen, so top-level mutable variable writes no longer remain `Todo`. `top_level_var_threadlocal_global_counter_basic.scoop` now passes. `cargo check -p scoopc` passes after these broad MIR shape updates.
- `cargo test --all` passed after the accumulated fixes.
- Remaining default-refactor run-pass blockers are still substantial and independent enough to require an explicit prerequisite before P7-T03 can honestly run the full standard matrix. Added `P7-T02Z` in `TODO-P7.md` and synced `TODO.md`; P7-T03 now depends on that prerequisite and remains incomplete.
