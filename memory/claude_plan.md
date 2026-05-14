# Claude Plan

## Planning Note

I will not record private chain-of-thought verbatim. This file tracks a concise reasoning summary, execution plan, and progress updates for the current invocation.

## Initial Goal

Complete exactly the first incomplete task in `TODO.md`, following the ordering and dependency rules defined there, then stop.

## Execution Plan

1. Read `TODO.md` and identify the first task whose title is not prefixed with `[DONE]`.
2. Check the latest commit message to see whether it mentions unfinished work directly relevant to that task.
3. Inspect only the files needed to understand and implement that task.
4. Implement the task with minimal, spec-correct changes.
5. Run the required validation for the task and any directly relevant broader checks.
6. If a blocking spec mismatch or missing prerequisite is discovered, update `TODO.md` with the minimum prerequisite task, update `PLAN.md` only if phase planning changes, commit that bookkeeping, and stop.
7. If the task is completed, mark it `[DONE]` in `TODO.md`, update its completion record, commit all required changes, and stop.

## Progress Log

- Plan file created before repository inspection.
- Read `TODO.md` and identified the first incomplete task: `P6-T02A`.
- Checked the latest commit: `[P6-T02A] Record typed contract blocker before final gap rewrite`.
- Current interpretation: the blocker recorded by the latest commit is directly relevant to `P6-T02A`, so the next step is to inspect the affected call-contract and effect-facts publication paths, confirm whether the task can be implemented now, and either fix the regression or record a concrete prerequisite if a deeper blocking issue is uncovered.
- Reproduced representative failures from the task definition:
  - `tests/fixtures/run-pass/receiver_function_value_call_basic.scoop` currently fails at runtime test execution.
  - `tests/fixtures/effect_lowered/dropped_continuation_abandons_remaining_work.scoop` currently fails due to effect-lowered snapshot drift.
- Next step: capture the precise compiler/runtime diagnostics for the run-pass failure and inspect the generated effect-lowered output diff to determine which contract publication path regressed.
- Captured precise failure sites:
  - `receiver_function_value_call_basic`: failing span is `this.length()`.
  - `std_process_args_exit_basic`: failing span is `args.get(0).length()`.
  - `delegated_property_lazy_init_once_basic`: MIR panic points at synthetic helper site using the property span (`"x"`), suggesting synthetic call-site identity collision.
- Additional representative failures:
  - `struct_function_field_call_basic`: typed HIR call contract missing for a function-valued member call.
  - `member_call_interface_named_args_basic` and `member_call_virtual_named_args_basic`: dispatch lowering reaches LLVM but named-arg canonicalization causes `unsupported value coercion from Ref to Int`.
  - `where_clause_bound_method_fun`: link-time undefined symbol for a specialized bound method implementation, suggesting direct-target / materialization publication drift rather than an early-stage type error.
- Strong root cause identified for the first cluster: `P6-T01` added a `MemberAccess` special-case in `crates/scoopc/src/pipeline/hir_stage.rs` that returns `None` for all non-`GC.*` member callees before the generic callable-value branch can run. This regressed `this.length()` and other member-access callable surfaces.
- Current implementation plan:
  1. Fix `hir_stage.rs` so non-`GC.*` member-access callees continue through the normal callable contract classification path.
  2. Fix receiver-aware typed contract publication for member-style direct/extension calls whose receiver is carried by the callee shape instead of `args[0]`.
  3. Give delegated-property synthetic helper calls stable distinct spans so lock/unlock/helper contracts do not overwrite each other.
  4. Re-run the representative fixtures to see which failures remain, then address the dispatch named-arg and bound-method materialization regressions if they are still present.
- Applied the first batch of fixes:
  - `pipeline/hir_stage.rs`: non-`GC.*` member-access callees no longer get trapped by the GC intrinsic special-case; receiver-aware contract publication now also reads the receiver from member-access callee shape when the binding carries a receiver slot.
  - `hir/lower/mod.rs`: synthetic helper calls created via `call_top_level_fun(...)` now get distinct synthetic call-site spans.
  - `mir/lower.rs`: dispatch call lowering now strips receiver-only binding state before non-receiver arg expected-type/canonicalization, fixing named-arg dispatch receiver pollution.
  - `typecheck/expr/call.rs`: function-valued member access used as a callee now publishes its own inferred callee type back into the expr-ty side table.
  - `effect_facts/builder.rs`: `scoop.core.trimIndent` is treated as a plain runtime helper surface like the other string/runtime helpers.
- Re-run results after the first batch:
  - `receiver_function_value_call_basic`: passes.
  - `struct_function_field_call_basic`: passes.
  - `delegated_property_lazy_init_once_basic`: passes.
  - `string_trim_indent_basic`: passes.
  - `std_process_args_exit_basic`: passes.
  - `member_call_interface_named_args_basic`: passes.
  - `member_call_virtual_named_args_basic`: passes.
- Remaining checks to finish the task:
  1. Re-run `where_clause_bound_method_fun` and determine whether the earlier link failure still exists. Status: PASS.
  2. Re-run the effect-lowered regression fixture and inspect/update any remaining drift only after confirming the contract behavior is correct. Status: PASS.
  3. `mir_refactor/call_contracts.scoop`: golden updated to reflect the corrected typed callee value type for closure-call (`Any` → `(Int) -> Int / Pure`). Status: PASS.
  4. Run the full task validation set (plus clippy), then update `TODO.md` / commit.

- Re-run results round 2:
  - `delegated_property_lazy_init_once_basic`: FAIL — actual stdout is `before / init / 1 / init / 1` instead of expected `before / init / 7 / 7`. The lazy initializer runs on every access AND returns `1` instead of `7`. Strongly suspects the synthetic-span change in `member_access_to_class_field` (sugar.rs) broke the lazy `inited` / `value` field reads/writes.
  - Need to investigate why the synthetic span breaks lazy field access semantics.

- Round 2 investigation result:
  - Root cause: synthesized lazy `assign_value` and `assign_inited` Stmts both used the original `c.x` source span as their stmt span. `synthesize_assign_place_contract_if_missing` keys by stmt span, so the second AssignPlaceContract was silently dropped. At MIR lowering time, `c.x$lazy_inited = true` looked up the contract for `C.x$lazy_value`, writing 1 into the value field and never marking `inited` as true.
  - Fix: every synthesized lazy assign-stmt now gets its own `fresh_synthetic_call_site_span`, applied to all three thread-safety modes.

- Round 3 cleanup:
  - Full-regression revealed two more pre-existing typed-contract surface gaps: `for_in_array_int_basic` (synthetic `scoop.core.size` / `scoop.core.get` calls without unique span) and `delegated_property_lowering` (generic delegated property without resolved member binding). Both are part of P6-T02A's stated scope (synthetic helper / delegated property contract publication), so I closed them rather than deferring.
  - For-in lowering: switched the synthetic size/get calls to `call_top_level_fun`, which already emits a fresh synthetic span and a typed contract automatically.
  - Generic delegated property: extended `GenericDelegatedPropertyInfo` with the delegate class FQN (extracted from the `Delegate()` constructor candidate at info-collection time), wrote `MemberRef::Fun { fqn }` into the synthetic getValue/setValue callee, and added a narrow `resolved_member_call_binding` path in the contract collector that turns those resolved synthetic member calls into `MemberDirect` typed contracts. The new path is gated to receivers ending in `$delegate` to avoid touching ordinary source-level member calls.
  - Effect-lowered: `build_call_boundary_operand_contract` now feeds `KnownInstance` `FunValue` calls through the same `build_known_instance_closure_call_arg_sources` path used for `Closure` calls, so 0-arg user calls on a closure with a non-Unit env tuple no longer trip "0 sources only allowed for Unit carrier".
  - Snapshot drift in `tests/fixtures/hir/*.hir`, `tests/fixtures/effect_lowered/*.effectlowered`, and `tests/fixtures/mir_refactor/call_contracts.mir` was regenerated only after the underlying contract behavior had been fixed.

- Final verification:
  - 10 task validation fixtures: all PASS.
  - `cargo run -p scoop -- test`: 1232 fixtures PASS, 0 fail (1269 checks).
  - `cargo clippy --all-targets -- -D warnings`: clean.
  - `cargo test --all`: net -5 baseline regressions (5 pre-existing tests now pass), 0 new regressions; 21 unrelated baseline snapshot drifts remain for P6-T03 / P7-T01.

- Marked `P6-T02A` as `[DONE]` in `TODO.md` and ready to commit.
