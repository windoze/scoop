# TODO-2：完整 fact 发布 + self-contained artifact（批 2）

> 计划基线：[`PLAN.md`](./PLAN.md) §4 批 2、§3；依据 `FACT_GAPS.md` FG-06/08/09(发布)/10/11(必发)/12/13/15、`EFFECT_INFER.md` §3/§4。
> 索引入口：[`TODO.md`](./TODO.md)
> 目标：HIR/MIR/P4 完整发布分层 effect facts + site/event/provenance/source-signature/boundary facts；artifact 自包含，下游不再回看 `LoweredHir`/`MaterializedMir` side table。
> 依赖：批 1（`TODO-1.md`）全部完成。

## 任务索引

| 任务 | 状态 | 目标 |
| --- | --- | --- |
| T2-01E | [DONE] | Fix effect-lowered GC handle token ABI regression exposed by T2-01F |
| T2-01F | [DONE] | Restore full fixture baseline observed during T2-01 validation |
| T2-01 | [DONE] | HIR 分层 `CallableSourceEffectFacts` + 统一 expression inference + canonical semantic facts（含 hidden init） |
| T2-01R | [DONE] | Review T2-01 |
| T2-02 | [DONE] | MIR `CallableInstanceEffectFacts` + effect-event/site-inventory/provenance facts + backend contracts 收口 |
| T2-02R | [DONE] | Review T2-02 |
| T2-03A | [TODO] | Restore higher-order function-value / closure continuation routes under P4 fact-only call-site facts |
| T2-03 | [TODO] | P4 纯消费上游 facts 产出 instance effect facts（local control 必发、call-site target/surface） |
| T2-03R | [TODO] | Review T2-03 |

---

### [DONE] T2-01E：Fix effect-lowered GC handle token ABI regression exposed by T2-01F

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
- 完成记录：2026-06-01 完成。修复 retained object-singleton `scoop.core.GC.*` intrinsic direct-call lowering 的显式参数类型推断：当 published function param list 含隐式 `GC` receiver 时，MIR lowering 会剥离该 receiver，再用真实显式参数推导 call arg carrier，避免 `GC.handleGet` / `GC.handleDrop` 的 `invoke_args_tuple_ty` 从 `scoop.core.GcHandle` 漂移到 `scoop.core.GC` / `Ref`。新增 effect-facts regression `gc_handle_intrinsic_call_sites_use_handle_token_carrier`，覆盖 `GC.handleGet` / `GC.handleDrop` 的 stable handle token carrier。验证：`cargo fmt`；`cargo test -p scoopc_effect_facts_stage gc_handle_intrinsic_call_sites_use_handle_token_carrier` 通过；`cargo build -p scoop -p scoopc` 通过；targeted fixtures `tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop`、`tests/fixtures/build/funptr_enter_native_no_statepoint_writeback.scoop`、`tests/fixtures/runtime_gc/gc_handle_roundtrip.scoop`、`tests/fixtures/runtime_gc/gc_handle_stale_callback_token_is_error.scoop`、`tests/fixtures/runtime_gc/gc_handle_token_roundtrip_callback_basic.scoop` 均通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test --all --all-targets` 通过；`python3 tools/run_fixtures.py` 已完整运行，剩余 7 个 failures 均是本文件下一任务 `T2-01F` 已明确列出的 fixture baseline 修复项（`effect_lowered/handle_finally_boundary.scoop`、`effect_lowered/nested_handle_self_contained_vs_outward.scoop`、`mir_lowered/aggregate_transport.scoop`、`mir_lowered/call_contracts.scoop`、`mir_materialized/pass_pipeline_metadata.scoop`、`run_pass_cone/dependency_c_sources_extern_call`、`run_pass_cone/dependency_cxx_sources_extern_call_cpp_stdlib`），本任务要求的 GC handle/build/runtime failures 已恢复通过且不再触发 `GC.handleDrop`/`GC.handleGet` argument `Ref`/`GcHandle` drift。

### [DONE] T2-01F：Restore full fixture baseline observed during T2-01 validation

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
- 完成记录：2026-06-01 完成。恢复 T2-01 validation 期间观察到的完整 fixture baseline：修复 dependency cone artifact 读取失败的根因，移除 MIR metadata 默认字段上的 `skip_serializing_if`，避免这些字段经 LIR `lir_program.bin` bincode 持久化时发生字段错位，并新增 `mir_metadata_default_fields_are_bincode_stable` 回归测试覆盖 `TopLevelRef` / `DispatchMetadata` / `CallKind::Direct` 的空默认字段 roundtrip。同步 5 个已确认语义正确的 snapshot golden（`effect_lowered/handle_finally_boundary.scoop`、`effect_lowered/nested_handle_self_contained_vs_outward.scoop`、`mir_lowered/aggregate_transport.scoop`、`mir_lowered/call_contracts.scoop`、`mir_materialized/pass_pipeline_metadata.scoop`），反映当前常量 operand 不再强制落临时 local 后的 MIR/LIR 输出与 source-slice/state-id 更新。验证：7 个 T2-01E 记录的剩余 targeted fixtures 全部通过；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test --all --all-targets` 通过；`python3 tools/run_fixtures.py` 完整通过（1664 checks）。

### [DONE] T2-01：HIR source-level effect facts + 统一 expression inference

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
- 完成记录：2026-06-01 完成。`HirFacts.source_sites` 已发布 `CallableSourceEffectFacts`、`CanonicalSemanticOperationFact`、`HiddenInitializerEffectFact`，HIR `FunDecl` 保留显式 source row 元数据，HIR stage 基于 canonical call/perform/handle contracts 统一计算 direct/inferred/published source rows，并发布 class ctor / object init / top-level init hidden summaries。新增 `tests/fixtures/typecheck/hir_source_effect_facts_polymorphic_ok.scoop`；同步 HIR golden 摘要计数。此前完整 fixture baseline 阻塞已由前置 `T2-01E` / `T2-01F` 修复。验证：`T2-01` 实现提交已运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets` 通过；`T2-01F` 恢复 baseline 后又运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`python3 tools/run_fixtures.py` 完整通过（1664 checks）。本次收口仅更新任务文档，代码自上述绿色验证后未改变，故复用该完整验证结果。

### [DONE] T2-01R：Review T2-01
- 验证：`python3 tools/run_fixtures.py`
- 依赖：T2-01
- 完成记录：2026-06-01 完成。Review T2-01 时发现 canonical semantic operation 发布存在一个非 `ExprKind::Call` 位点缺口：operator 等语义调用虽然已由 typecheck 写入 selected callable binding，但 HIR contract 收集只在显式 `Call` 表达式上发布 call-site contract，导致 `CanonicalSemanticOperationFact` 和 source `expr_surface_row` 会漏掉 operator callee 的 published row。已修复 HIR contract 收集对 unary/binary/member-access semantic call binding 的发布，并让 unary source surface row 消费对应 semantic core call；新增 `stage::tests::source_effect_facts_include_operator_semantic_bindings` 覆盖 private omitted-row 函数通过二元/一元 operator 推导并发布 `Log` effect。验证：`cargo fmt`；`cargo test -p scoopc_hir source_effect_facts_include_operator_semantic_bindings`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py` 完整通过（1664 checks）。

### [DONE] T2-02：MIR instance facts + effect-event/provenance + backend contracts 收口

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
- 完成记录：2026-06-01 完成。扩展 `scoopc_mir_facts` 为自包含 MIR handoff：新增 effect/site/event/block-region/call-target/call-surface facts、callable/result provenance facts、boundary source contracts、backend contract facts，并在 materialized instance inventory 中发布稳定 `eff_args`。`MaterializedMir` 现在携带 HIR 已发布的 callable effect row 模板，MIR handoff 按实例 `eff_args` 发布 `CallableInstanceEffectFacts { declared_surface_row, actual_surface_row, published_surface_row, step_effect_row }`；MIR lowering 的 hidden initializer effects 改为搬运 HIR `HiddenInitializerEffectFact`，不再用 `HiddenInitEffectAnalyzer` 重新扫描 effect。`mir_stage` 在 P4-ready handoff 发布 site inventory、effect event stream、block region、call target/surface、callable value/result provenance、boundary source anchor/operand contract，以及 source signature/layout/vtable/itable/extern/native/global init backend facts。新增单测 `p4_ready_mir_facts_publish_self_contained_handoff_contracts` 覆盖 self-contained handoff 关键 fact group。验证：`cargo fmt`；`cargo check --all-targets`；`cargo test -p scoopc p4_ready_mir_facts_publish_self_contained_handoff_contracts`；`cargo test -p scoopc_mir_facts`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets` 完整通过；`python3 tools/run_fixtures.py` 完整通过（1664 checks）。

### [DONE] T2-02R：Review T2-02
- 验证：`python3 tools/run_fixtures.py`
- 依赖：T2-02
- 完成记录：2026-06-01 完成。Review T2-02 时修复三类 handoff correctness 问题：`MaterializedCallableEffectTemplate` 改为携带完整 `TemplateKey`，MIR instance effect rows 按 `(fqn, source_path, decl_span)` 精确匹配，避免 overload 共享 display FQN 时串用 source effect row；MIR fact verifier 增加 schema version 校验，并将 wire schema bump 到 `1.3` 覆盖新增 effects/provenance/boundary/backend handoff groups 与 instance `eff_args`；callable value provenance 只在目标 local 类型确为 function 时把 `TopLevelRef` 发布为 callable provenance，并把 block/statement 纳入 fact identity，避免普通 top-level value 被伪装成 `DirectFunction` 或重复 local identity。新增回归测试覆盖 overload effect identity、非 callable top-level value provenance、unsupported MIR fact schema version；`scoop run` 测试 helper 改为每次重建 `scoopc`，避免 schema bump 后 stale subprocess binary 写出旧 artifact。同步 `mir_materialized/pass_pipeline_metadata.mir` golden，使 materialized MIR dump 捕获 T2-02 发布的 handoff fact groups。验证：`cargo fmt`；targeted tests `cargo test -p scoopc_mir_facts verifier_rejects_unsupported_schema_version`、`cargo test -p scoopc mir_callable_instance_effects_match_overload_template_identity`、`cargo test -p scoopc mir_callable_value_provenance_does_not_relabel_top_level_values_as_functions`、`cargo test -p scoop run_builds_and_executes_minimal_main` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test --all --all-targets` 完整通过；`python3 tools/run_fixtures.py tests/fixtures/mir_materialized/pass_pipeline_metadata.scoop` 通过；`python3 tools/run_fixtures.py` 完整通过（1664 checks）。

### [TODO] T2-03A：Restore higher-order function-value / closure continuation routes under P4 fact-only call-site facts

- 背景：执行 `T2-03` 时，P4 已改为消费 MIR-published site/event/target/surface/provenance facts，并删除 P4 内部 MIR shape 扫描、dispatch union 与 callable-value 局部数据流恢复。`cargo test --all --all-targets` 随后暴露两个直接相关的 continuation 路由回归：
  1. `single_pipeline_runs_higher_order_function_value_handled_effect_cli`：`tests/fixtures/run-pass/effect_indirect_perform_nonresuming_function_value_higher_order_when_direct.scoop` 输出停在 `5\ncaught\n9\n` 且退出码为 `1`，未恢复到 `10`。
  2. `single_pipeline_runs_indirect_perform_closure_resume_cli`：`tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure.scoop` resume 后未重新进入 closure resume 路径，输出缺少 `closure_resume` 且结果为 `32` 而不是 `42`。
- 根因边界：这不是可忽略的既有噪声；它由当前 T2-03 的 fact-only 切换暴露/触发，说明 P4/P5 仍需要由 MIR facts 明确发布并消费 higher-order function-value / closure callable provenance 与跨 callable continuation route 所需的 call-site contract。不得恢复 P4 MIR 数据流扫描或用 fixture 形状绕过。
- 必须实现的内容：
  1. 扩展 MIR-published provenance/target facts，使 `choose(mode)()`、closure carrier、parameter-carried function value 等 higher-order call sites 能以 fact 形式表达足够的 callable/candidate provenance（包括多来源 join 或参数来源），供 P4 消费。
  2. P4 只消费这些 facts 生成 `CallSiteEffectFacts`，不得回扫 `MaterializedMir` statements/terminators 或重做局部数据流。
  3. 保持 P5 callee-suspend / cross-call continuation provenance 能为 function-value 和 closure indirect perform 恢复正确 route；不得通过禁用 fact-only call-site lowering 绕过。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test -p scoop --test p7_default_pipeline`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。
- 完成条件：上述两个 p7 regressions 恢复通过，且 P4 仍不恢复 MIR shape 扫描、dispatch union 或 callable-value 局部数据流恢复。
- 依赖：T2-02R
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
- 依赖：T2-03A
- 完成记录：（待填）

### [TODO] T2-03R：Review T2-03
- 验证：`python3 tools/run_fixtures.py`
- 依赖：T2-03
- 完成记录：（待填）
