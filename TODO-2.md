# TODO-2：完整 fact 发布 + self-contained artifact（批 2）

> 计划基线：[`PLAN.md`](./PLAN.md) §4 批 2、§3；依据 `FACT_GAPS.md` FG-06/08/09(发布)/10/11(必发)/12/13/15、`EFFECT_INFER.md` §3/§4。
> 索引入口：[`TODO.md`](./TODO.md)
> 目标：HIR/MIR/P4 完整发布分层 effect facts + site/event/provenance/source-signature/boundary facts；artifact 自包含，下游不再回看 `LoweredHir`/`MaterializedMir` side table。
> 依赖：批 1（`TODO-1.md`）全部完成。

## 任务索引

| 任务 | 状态 | 目标 |
| --- | --- | --- |
| T2-01E | [TODO] | Fix effect-lowered GC handle token ABI regression exposed by T2-01F |
| T2-01F | [TODO] | Restore full fixture baseline observed during T2-01 validation |
| T2-01 | [TODO] | HIR 分层 `CallableSourceEffectFacts` + 统一 expression inference + canonical semantic facts（含 hidden init） |
| T2-01R | [TODO] | Review T2-01 |
| T2-02 | [TODO] | MIR `CallableInstanceEffectFacts` + effect-event/site-inventory/provenance facts + backend contracts 收口 |
| T2-02R | [TODO] | Review T2-02 |
| T2-03 | [TODO] | P4 纯消费上游 facts 产出 instance effect facts（local control 必发、call-site target/surface） |
| T2-03R | [TODO] | Review T2-03 |

---

### [TODO] T2-01E：Fix effect-lowered GC handle token ABI regression exposed by T2-01F

- 背景：恢复 `T2-01F` fixture baseline 时，泛型 member dispatch / synthetic array binding 等 failures 已定位并部分修复，但 GC handle 相关 fixtures 仍阻塞完整 baseline。当前 `dump-ir` 中 `GC.handleDrop(h)` 的 MIR metadata/transport 显示参数为 `scoop.core.GcHandle`，但 effect-lowered/LLVM codegen 路径仍把该参数 lowering 为 `Ref`，触发 `MIR GC.handleDrop lowering: argument is not a GcHandle struct`。
- 必须处理的 failures：
  1. `tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop`
  2. `tests/fixtures/build/funptr_enter_native_no_statepoint_writeback.scoop`
  3. `tests/fixtures/runtime_gc/gc_handle_roundtrip.scoop`
  4. 继续复核 `T2-01F` 中列出的其他 `runtime_gc/gc_handle_*` fixtures，确保同一 GC handle token ABI root cause 被成组修复。
- 要求：修复 effect-lowered/plain-call/codegen 对 object singleton GC intrinsic 的参数 carrier 与 local storage `CgTy` 推断，不能用 fixture-only special case 绕过；`GC.handleNew` / `GC.handleGet` / `GC.handleDrop` 的 token ABI 必须保持 `GcHandle.raw` 结构化 token 语义。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；上述 targeted fixtures；`python3 tools/run_fixtures.py`。
- 完成条件：上述 GC handle/build/runtime failures 通过，且不再触发 `GC.handleDrop` argument `Ref`/`GcHandle` drift。
- 依赖：T1-02R
- 完成记录：（待填）

### [TODO] T2-01F：Restore full fixture baseline observed during T2-01 validation

- 背景：执行 `T2-01` 实现与完整 fixture 验证时发现以下 exact failures。根据失败策略，这些不能作为既有噪声忽略；必须在 `T2-01` 标记完成前修复、同步 golden，或进一步证明并拆出更精确的前置任务。
- 必须处理的 failures：
  1. `tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop`
  2. `tests/fixtures/build/funptr_enter_native_no_statepoint_writeback.scoop`
  3. `tests/fixtures/build/intrinsic_sysroot_overlay_array_mutablearray_body_methods_basic.scoop`
  4. `tests/fixtures/effect_lowered/handle_finally_boundary.scoop`
  5. `tests/fixtures/effect_lowered/nested_handle_self_contained_vs_outward.scoop`
  6. `tests/fixtures/mir_lowered/aggregate_transport.scoop`
  7. `tests/fixtures/mir_lowered/call_contracts.scoop`
  8. `tests/fixtures/mir_materialized/pass_pipeline_metadata.scoop`
  9. `tests/fixtures/run-pass/ctor_type_arg_explicit_basic.scoop`
  10. `tests/fixtures/run-pass/ctor_type_arg_lhs_zero_arg_ctor_basic.scoop`
  11. `tests/fixtures/run-pass/delegated_property_observable_vetoable_concurrency_ok.scoop`
  12. `tests/fixtures/run-pass/for_in_array_int_basic.scoop`
  13. `tests/fixtures/run-pass/gc_continuation_multi_thread_concurrent_alloc_resume.scoop`
  14. `tests/fixtures/run-pass/gc_pin_unpin_basic.scoop`
  15. `tests/fixtures/run-pass/generic_class_gc_specialized_type.scoop`
  16. `tests/fixtures/run-pass/generic_class_gc_value_field.scoop`
  17. `tests/fixtures/run-pass/generic_class_method.scoop`
  18. `tests/fixtures/run-pass/generic_class_nested.scoop`
  19. `tests/fixtures/run-pass/global_init/threadlocal_var_initialized_for_worker_thread.scoop`
  20. `tests/fixtures/run-pass/intrinsic_generic_class_body_method_basic.scoop`
  21. `tests/fixtures/run-pass/member_call_generic_class_body_method_basic.scoop`
  22. `tests/fixtures/run-pass/std_thread_basic.scoop`
  23. `tests/fixtures/run-pass/sysroot_atomic_basic.scoop`
  24. `tests/fixtures/runtime_gc/extern_enter_native_gc_arg_spill_reload.scoop`
  25. `tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop`
  26. `tests/fixtures/runtime_gc/funptr_enter_native_roots_gc.scoop`
  27. `tests/fixtures/runtime_gc/gc_handle_roundtrip.scoop`
  28. `tests/fixtures/runtime_gc/gc_handle_stale_callback_token_is_error.scoop`
  29. `tests/fixtures/runtime_gc/gc_handle_token_roundtrip_callback_basic.scoop`
  30. `tests/fixtures/runtime_gc/gc_move_stackmap_heap_fixup.scoop`
  31. `tests/fixtures/runtime_gc/gc_pin_unpin_move_stress_matrix.scoop`
  32. `tests/fixtures/runtime_gc/gc_stw_cross_thread_in_native_roots_basic.scoop`
  33. `tests/fixtures/runtime_gc/gc_stw_cross_thread_roots_basic.scoop`
  34. `tests/fixtures/runtime_gc/std_sync_backend_parity_baseline_moving.scoop`
  35. `tests/fixtures/runtime_gc/std_sync_backend_parity_baseline_nonmoving.scoop`
  36. `tests/fixtures/runtime_gc/std_sync_backend_parity_hosted.scoop`
  37. `tests/fixtures/runtime_gc/std_sync_backend_parity_immix_minor.scoop`
  38. `tests/fixtures/runtime_gc/std_sync_backend_parity_minimal.scoop`
  39. `tests/fixtures/run_pass_cone/dependency_c_sources_extern_call`
  40. `tests/fixtures/run_pass_cone/dependency_cxx_sources_extern_call_cpp_stdlib`
  41. `tests/fixtures/run_pass_cone/explicit_sysroot_thread_dependency`
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。
- 完成条件：完整 fixture suite 通过，或任何仍无法同 invocation 修复的 failure 被拆成更精确且更早的 prerequisite task，并且 `T2-01` 仍依赖全部 prerequisite。
- 依赖：T2-01E
- 完成记录：（待填）

### [TODO] T2-01：HIR source-level effect facts + 统一 expression inference

- 参考：`PLAN.md` §2.3/§3/§4；`EFFECT_INFER.md` §72-153；`FACT_GAPS.md` FG-06/09(source)。`crates/scoopc_hir/src/typecheck/expr/stmt.rs` `check_required_effects_for_fun_decl`、`stage.rs` `FunctionEffectContract`。
- 必须实现的内容：
  1. 发布 `CallableSourceEffectFacts { declared_surface_row, direct_effect_row, inferred_surface_row_template, published_surface_row_template, row_is_closed, inference_status }`（`EffectRowTemplate`）；`check_required_effects_for_fun_decl` 从"只报错"改为"发布 facts"。
  2. 统一 expression-level effect inference：每个 expr 算 `expr_surface_row`（union 子表达式 + callee published row，按 handler 规则移除本地处理不 outward 的 effect）。
  3. canonical semantic expansion facts：delegated property / operator / loop / computed property / constructor-init 发布统一 core call/op fact，effect inference 只消费它们，**删除按语法/名称的 effect 后门**。
  4. **FG-06**：发布 `HiddenInitializerEffectFact`（class ctor / object init / top-level init 的 hidden-effect summary），替代 MIR lowering 里 `HiddenInitEffectAnalyzer` 的重扫（搬运留 T2-02）。
  5. interface(含 default)/abstract/open method 的 `published_surface_row_template` 必须来自显式契约。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`；新增/更新 typecheck fixtures。
- 完成条件：HIR 发布完整分层 source facts + canonical semantic facts；表达式 effect 无语法后门；hidden init effect 由 HIR fact 提供。
- 依赖：T2-01F
- 完成记录：2026-06-01 部分实现已落地：`HirFacts.source_sites` 新增 `CallableSourceEffectFacts`、`CanonicalSemanticOperationFact`、`HiddenInitializerEffectFact`，HIR `FunDecl` 保留显式 source row 元数据，HIR stage 基于 canonical call/perform/handle contracts 计算 direct/inferred/published source rows，并发布 hidden init summaries。新增 `tests/fixtures/typecheck/hir_source_effect_facts_polymorphic_ok.scoop`；同步 HIR golden 摘要计数。验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets` 通过。完整 `python3 tools/run_fixtures.py` 已运行但未通过，发现 41 个 failures；已插入前置 `T2-01F`，因此本任务保持未完成，完成前必须先恢复完整 fixture baseline。

### [TODO] T2-01R：Review T2-01
- 验证：`python3 tools/run_fixtures.py`
- 依赖：T2-01
- 完成记录：（待填）

### [TODO] T2-02：MIR instance facts + effect-event/provenance + backend contracts 收口

- 参考：`PLAN.md` §3；`FACT_GAPS.md` FG-08/10/12/13。
- 必须实现的内容：
  1. 发布 `CallableInstanceEffectFacts { declared_surface_row, actual_surface_row, published_surface_row, step_effect_row }`（稳定表示）；method instance 身份按 published/step row 是否引用 owner eff 区分（getValue 共享 eff-less；setValue eff-keyed；class/itable key eff-aware）。
  2. **FG-08**：发布 `MirEffectEventFact` / `MirBlockEffectRegionFact` / `MirSiteInventoryFact`（结构化 effect event stream、block-to-site inventory、handled-region/cleanup/suspend boundary），供 P4 solver 消费，替代 P4 扫 MIR shape。
  3. **FG-10**：发布 `CallableValueProvenanceFact` / `ResultProvenanceFact`（函数值 points-to/provenance + pass-rewritten summary 稳定查询面）。
  4. **FG-12**：boundary discovery/segmentation 发布结构化 `BoundarySourceContract`（boundary statement anchor、result local、carrier operand source、arg source、closure env decomposition），供 P5 消费。
  5. **FG-13**：MIR facts family/metadata 补 `eff_args`/layout/vtable/itable/extern/native/global init contract，把 `MaterializedBackendContracts` 收口为 fact artifact；搬运 T2-01 的 `HiddenInitializerEffectFact` 到 MIR site metadata。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。
- 完成条件：MIR facts 自包含；instance 身份不再 eff 分叉；effect-event/provenance/boundary/backend contract 由 fact 提供。
- 依赖：T2-01R
- 完成记录：（待填）

### [TODO] T2-02R：Review T2-02
- 验证：`python3 tools/run_fixtures.py`
- 依赖：T2-02
- 完成记录：（待填）

### [TODO] T2-03：P4 纯消费上游 facts 产出 instance effect facts

- 参考：`PLAN.md` §3；`FACT_GAPS.md` FG-08/09(发布)/11(必发)。
- 必须实现的内容：
  1. P4 solver 消费 `MirEffectEventFact`/site/region facts，不再扫 materialized MIR statement/terminator shape（`effect_facts/builder.rs` `scan_block_sites`/`scan_block_*`）。
  2. call-site target/declared row 用已发布 `CallSiteTargetFact`/`CallSiteSurfaceEffectFact`（FG-09），删除 `build_direct_like_call_site`/`union_candidate_rows` 的 overload 选择与 declared row 重算。
  3. **FG-11**：`BodyEffectFacts.local_control_step_schema` 设为 P4 必发 contract（owner step schema 由 `step_effect_row` 确定）。
  4. callable value/closure effect 用 T2-02 的 provenance fact（FG-10），删除 P4 局部数据流恢复。
- 必须遵从的约束：P4 不再做 overload/数据流/effect 重建；env/dispatch table 重建（FG-07）留批 3，本任务先把 effect 求解改为消费 facts。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。
- 完成条件：P4 effect 求解纯消费上游 facts；local control schema 必发。
- 依赖：T2-02R
- 完成记录：（待填）

### [TODO] T2-03R：Review T2-03
- 验证：`python3 tools/run_fixtures.py`
- 依赖：T2-03
- 完成记录：（待填）
