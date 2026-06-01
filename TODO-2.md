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
| T2-03A1 | [DONE] | MIR 发布 higher-order callable target/provenance facts，覆盖 closure/function-value/param/join |
| T2-03A2a0 | [DONE] | 修复 composed resume 的 surface-resume owner dispatch target 发布/选择 |
| T2-03A2a | [DONE] | 修复 escaped closure callee-suspend composed continuation resume route |
| T2-03A2 | [DONE] | P4 只消费 published call-site facts，移除 higher-order 下游反向重建 |
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

### [DONE] T2-03A1：MIR 发布 higher-order callable target/provenance facts

- 背景：执行 `T2-03` 时，P4 已切换为消费 MIR-published site/event/target/surface/provenance facts，并删除 P4 内部 MIR shape 扫描、dispatch union 与 callable-value 局部数据流恢复。随后暴露两个 continuation 路由回归：
  1. `single_pipeline_runs_higher_order_function_value_handled_effect_cli`：`tests/fixtures/run-pass/effect_indirect_perform_nonresuming_function_value_higher_order_when_direct.scoop` 输出停在 `5\ncaught\n9\n` 且退出码为 `1`，未恢复到 `10`。
  2. `single_pipeline_runs_indirect_perform_closure_resume_cli`：`tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure.scoop` resume 后未重新进入 closure resume 路径，输出缺少 `closure_resume` 且结果为 `32` 而不是 `42`。
- 根因边界：这不是可忽略的既有噪声；fact-only 切换暴露了 MIR facts 对 higher-order function value / closure / parameter-carried callable / join provenance 的发布不完整。修复应发生在 MIR fact producer 或明确的 pre-P4 fact publication 层，不能让 P4 重新成为 interprocedural callable points-to solver。
- 必须实现的内容：
  1. 扩展 `scoopc_mir_facts` 的 callable provenance/target fact schema，使 dynamic callable call sites 能表达 authoritative target contract：closed known instance、closed candidate set、known closure fn_ptr、direct function、param-carried callable、join sources，以及是否仍需 `DynamicFallback`。
  2. 对 `choose(mode)()`、closure carrier、direct-call result returning callable、parameter-carried function value、多来源 join 发布稳定、可序列化、带 identity 的 facts。若一个 call site 的候选集合已闭合，fact 必须直接携带 stable instance keys；若存在未知或开放参数来源，必须显式标记 open/dynamic fallback，不能用 `CandidateSet([])` 表达未知。
  3. 若需要跨 callable 参数传播，例如 `callIt(f)` 内部 `f()` 的候选来自 caller 实参，必须由 MIR fact publication 产出显式的参数/实参替换关系或最终 per-call-site target fact；P4 不应通过扫描所有 body 的 call targets 反向推导该关系。
  4. `CallSiteSurfaceEffectFact` 与 callable target/provenance fact 必须保持同一来源语义：closed candidate set 的 surface row 来自候选 published rows 的上游发布/合并；dynamic fallback 的 surface row 来自函数类型签名，precision 标记为 fallback/widened。
  5. 保持 artifact self-contained：新增 fact 必须进入 MIR artifact verifier、stable dump/schema version、bincode roundtrip 覆盖；下游不应回看 `LoweredHir`/`MaterializedMir` side table 才能解释 higher-order target。
- 明确禁止：不得在 P4 中恢复 MIR statement/terminator shape 扫描；不得在 P4 中恢复局部数据流；不得在 P4 中全局扫描 caller body 来反解某个 callee 参数的候选；不得用 fixture 形状 special case；不得把未知 target 编码为空 candidate set。
- 验证：`cargo fmt`；新增/更新 MIR facts 单测覆盖 param、join、closure、call-result provenance；`cargo test -p scoopc <新增测试名>`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`。
- 完成条件：MIR published facts 能单独解释上述 two regressions 所需的 higher-order call-site target/provenance；fact verifier/schema/stable dump 同步；P4 仍未改动为下游重建。
- 依赖：T2-02R
- 完成记录：2026-06-02 完成。扩展 `scoopc_mir_facts` callable target/provenance schema：`CallSiteTarget` 现在能表达 `Param`、`Join { sources, requires_dynamic_fallback }` 与显式 `DynamicFallback { reason }`，`CallableValueProvenance` 支持 join sources；MIR fact verifier 拒绝空 `CandidateSet` / 空 join，stable dump 输出 callable value provenance 与 result provenance，并将 wire schema bump 到 `1.5`。MIR stage 新增 body-local callable provenance dataflow，从参数、`TopLevelRef`、closure carrier、direct-call result provenance 与 CFG join 发布稳定 callable-value facts，并为 higher-order `FunValue` call site 发布 authoritative target fact；closed 多来源 join 会收敛成 stable-key `CandidateSet`，param/open 来源保持显式 param/join/dynamic fallback，不把未知编码为空候选集合。新增测试 `mir_facts_round_trip_callable_join_target_and_provenance` 覆盖 schema/dump/bincode roundtrip，新增 `mir_higher_order_callable_targets_publish_param_join_and_closure_facts` 覆盖 param-carried callable、closure carrier、direct-call result returning callable、join candidate set。验证：`cargo fmt` 通过；targeted tests `cargo test -p scoopc_mir_facts mir_facts_round_trip_callable_join_target_and_provenance`、`cargo test -p scoopc mir_higher_order_callable_targets_publish_param_join_and_closure_facts` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test --all --all-targets` 已运行，剩余 2 个 failures 是下一任务 `T2-03A2` 已明确要求恢复的 p7 higher-order/closure continuation regressions；`python3 tools/run_fixtures.py` 已运行，剩余 22 个 failures 已补充登记到 `T2-03A2` 的必须处理清单，均属于后续 P4/P5 消费 published facts 与 fact-only continuation route 收口范围，本任务只发布 MIR facts，不恢复 P4 反向重建。

### [DONE] T2-03A2a0：修复 composed resume 的 surface-resume owner dispatch target 发布/选择

- 背景：执行 `T2-03A2a` 时确认 wrapper continuation 已捕获 callee continuation，但 composed resume 继续把 `resume(32)` 当作 caller boundary complete。进一步生成 LLVM IR 后发现：composed resume 对 LIR callee continuation 依赖 `surface_resume` / `surface_resume_outcome`，而这些 surface 函数在单 target 或 dynamic adapter fallback 下会静态/递归选择不匹配的 owner surface；例如 callIt wrapper resume 应先恢复 closure continuation，但 surface owner dispatch 会回到 caller/callIt owner，把 payload 直接投到 caller boundary result，或在尝试动态化后命中不自洽 target 而 trap。
- 根因边界：这不是 P4 points-to 问题，也不能通过 P4 反向重建或 fixture 变形解决。缺口位于 P5/LLVM surface-resume dispatch ABI：published continuation schema / owner continuation object / owner trampoline target 必须能按实际 continuation object descriptor 选择正确 owner，且 composed resume 必须消费该 published dispatch contract，而不是在 composed route 私自猜测 owner。
- 必须实现的内容：
  1. 让 `codegen_surface_resume` / `codegen_surface_resume_outcome` / `codegen_continuation_drive_outcome` 对 composed route 所需的 continuation schema 发布并使用完整 owner target set；单 target fast path 只能在 target set 已证明闭合且实际 object descriptor 唯一时启用。
  2. 修复 `codegen_dynamic_surface_resume_adapter` 的 candidate 到 owner target 映射，避免把某个 continuation object descriptor 映射到不接受该 descriptor 的 owner surface；必要时从 ABI dispatch target 中携带 owner trampoline symbol，而不是递归调用等价但不匹配的 surface symbol。
  3. 保持 artifact self-contained：target set 必须来自 published LIR/ABI facts，不得在 P4 或 LLVM composed route 扫描 caller/callee body 反解。
  4. 增加 LLVM/LIR 级回归，断言 `effect_escape_continuation_indirect_perform_closure.scoop` 的 composed resume 会先按 captured callee continuation object dispatch 到 closure resume owner，再把 complete 投影回 caller boundary result。
- 验证：`cargo fmt`；`cargo build -p scoopc`；相关新增 targeted test；`cargo test -p scoop --test p7_default_pipeline single_pipeline_runs_indirect_perform_closure_resume_cli -- --nocapture`。
- 完成条件：surface-resume owner dispatch 对 composed callee continuation target 自洽，`single_pipeline_runs_indirect_perform_closure_resume_cli` 不再把 resume payload 直接写入 caller boundary result。
- 依赖：T2-03A1
- 完成记录：2026-06-02 完成。修复 composed resume 所依赖的 surface-resume owner dispatch target 发布/选择：当 call-boundary wrapper target set 仍是开放/dynamic fallback 时，LIR dispatch inventory 会发布所有 ABI-compatible continuation object internal-method owners，并由 LLVM surface/outcome/drive entry 按实际 continuation object descriptor dispatch，而不是单 target 直连到静态 owner。`codegen_dynamic_surface_resume_adapter` 改为映射到 owner-specific trampoline symbol，避免递归调用不匹配的 shared surface；surface outcome wrapper 会先接收 owner outcome，再按 published wrapper projection 重新编码为 wrapper outcome。新增 p7 LLVM 回归 `single_pipeline_emits_descriptor_dispatched_closure_resume_surface_cli`，断言 closure composed resume surface 同时发布 wrapper owner 与 captured closure owner descriptor 分派；既有 runtime 回归 `single_pipeline_runs_indirect_perform_closure_resume_cli` 输出恢复为包含 `closure_resume\n32` 和 `body_done\n42`。验证：`cargo fmt`；`cargo build -p scoop -p scoopc`；targeted p7 closure tests 通过；`python3 tools/run_fixtures.py tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure.scoop --processes 1` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test --all --all-targets` 完整通过；`python3 tools/run_fixtures.py` 已完整运行，剩余 15 个 failures 均是本文件后续 `T2-03A2` 已显式列出的 fixture snapshot/run-pass 收口项（effect_facts/dispatch_and_resume_call、dynamic_fallback_widening、handle_finally_boundary、handle_perform、nested_handle_self_contained_vs_outward；effect_lowered/dispatch_and_resume_call、dropped_continuation_abandons_remaining_work、effect_boundary_inside_expr_context、handle_finally_boundary、handle_perform、nested_handle_self_contained_vs_outward；mir_materialized/pass_pipeline_metadata；run-pass/delegated_property_observable_vetoable_basic、delegated_property_observable_vetoable_concurrency_ok、effect_handle_return_from_function_any_boxing）。

### [DONE] T2-03A2a：修复 escaped closure callee-suspend composed continuation resume route

- 背景：执行 `T2-03A2` 时，已修复 `choose(mode)()` closed `CandidateSet` 被 MIR `CallSiteSurfaceEffectFact` 发布为 `Pure` 的缺口，并让 callable-carrier dynamic invoke 接受 closed `CandidateSet`；`single_pipeline_runs_higher_order_function_value_handled_effect_cli` 已恢复通过。但 `single_pipeline_runs_indirect_perform_closure_resume_cli` 仍失败，输出停在 `body_done\n32`，未先恢复 closure resume tail 输出 `closure_resume\n32` 并返回 `42`。
- 根因边界：这不是可忽略既有噪声；当前 P4 已消费 published per-site facts，剩余问题集中在 P5/LLVM composed continuation / callee-suspend resume route。修复不得把 P4 重新变成跨 body callable points-to solver，也不得禁用 fact-only call-site lowering、dynamic invoke、continuation wrapper/projection 校验或改变 fixture 形状。
- 必须实现的内容：
  1. 让 escaped continuation 保存的 composed callee continuation 在 `k.resume(...)` 时先恢复 closure/function-value callee suspend point，再把 callee completion 投影回 caller boundary result；不能把 resume payload 直接当作外层 call boundary complete。
  2. 覆盖 `callIt(f)` param-carried callable、closure literal/closure carrier、direct/inlined call boundary 的同类路径；若需要额外 MIR published provenance 或 call-target substitution fact，必须在 MIR fact producer 发布并由 P4/P5 消费，不能在 P4 扫描 caller body 反解。
  3. 保持 closed `CandidateSet` / `KnownInstance` / `DynamicFallback` target contract 强语义；未知/open-param 不得编码为空候选集合。
  4. 增加或更新 LIR/LLVM 级回归，断言 composed resume route 使用 captured callee continuation，而不是直接写 caller boundary result。
- 验证：`cargo fmt`；`cargo build -p scoopc`；`cargo test -p scoop --test p7_default_pipeline single_pipeline_runs_indirect_perform_closure_resume_cli`；`python3 tools/run_fixtures.py tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure.scoop --processes 1`；相关 closure locals fixture；修复后继续运行 `T2-03A2` 要求的完整验证。
- 完成条件：`single_pipeline_runs_indirect_perform_closure_resume_cli` 输出恢复为 `body_start\nclosure_enter\narm\nresult\n99\nclosure_resume\n32\nbody_done\n42\nafter_resume\n`；同类 closure locals fixture 不再跳过 callee resume tail；P4 无跨 body 反向重建。
- 依赖：T2-03A2a0
- 完成记录：2026-06-02 完成。复核最新 `[T2-03A2a0]` 实现后确认 current tree 已满足本任务 runtime route 要求：escaped closure 保存的 composed callee continuation 在 `k.resume(32)` 时会先按 captured closure continuation owner 恢复 callee suspend point，输出 `closure_resume\n32`，再把 callee completion 投影回 caller boundary result，最终输出 `body_done\n42`；同类 closure locals fixture 也恢复 callee resume tail 与 capture/local restore。当前任务未新增代码变更，仅做 `T2-03A2a` 状态收口；剩余完整 fixture failures 均为下一任务 `T2-03A2` 已显式列出的 P4 fact-only/snapshot/run-pass 收口项。验证：`cargo test -p scoop --test p7_default_pipeline single_pipeline_runs_indirect_perform_closure_resume_cli -- --nocapture` 通过；targeted fixtures `python3 tools/run_fixtures.py tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure.scoop --processes 1`、`python3 tools/run_fixtures.py tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_locals.scoop --processes 1` 均通过；`cargo fmt` 通过；`cargo build -p scoopc` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test -p scoop --test p7_default_pipeline` 通过；`cargo test --all --all-targets` 完整通过；`python3 tools/run_fixtures.py` 完整运行，剩余 15 个 failures 与下一任务 `T2-03A2` 清单一致（`effect_facts/dispatch_and_resume_call`、`dynamic_fallback_widening`、`handle_finally_boundary`、`handle_perform`、`nested_handle_self_contained_vs_outward`；`effect_lowered/dispatch_and_resume_call`、`dropped_continuation_abandons_remaining_work`、`effect_boundary_inside_expr_context`、`handle_finally_boundary`、`handle_perform`、`nested_handle_self_contained_vs_outward`；`mir_materialized/pass_pipeline_metadata`；`run-pass/delegated_property_observable_vetoable_basic`、`delegated_property_observable_vetoable_concurrency_ok`、`effect_handle_return_from_function_any_boxing`）。

### [DONE] T2-03A2：P4 只消费 published higher-order call-site facts

- 背景：当前修复尝试中，`mir_stage.rs` 对 callable provenance 的发布方向是正确的，但 `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs` 又新增了跨 body 反向解析：从 dynamic boundary carrier local 出发，扫描所有 body 的 `call_targets`，通过 `BoundarySourceContract.args` 追踪 param-carried callable，再把候选实例拼回 `CallSiteEffectFacts`。这虽然消费的是 MIR facts，不是 raw MIR shape，但本质上仍是 P4 在重建上游缺失的 callable target/provenance。
- T2-03A1 验证期间观察到的完整 fixture failures 也归入本任务，完成前必须修复或同步为语义正确的 golden，不能留下未排期失败：
  1. `tests/fixtures/effect_facts/dispatch_and_resume_call.scoop`
  2. `tests/fixtures/effect_facts/dynamic_fallback_widening.scoop`
  3. `tests/fixtures/effect_facts/handle_finally_boundary.scoop`
  4. `tests/fixtures/effect_facts/handle_perform.scoop`
  5. `tests/fixtures/effect_facts/nested_handle_self_contained_vs_outward.scoop`
  6. `tests/fixtures/effect_lowered/dispatch_and_resume_call.scoop`
  7. `tests/fixtures/effect_lowered/dropped_continuation_abandons_remaining_work.scoop`
  8. `tests/fixtures/effect_lowered/effect_boundary_inside_expr_context.scoop`
  9. `tests/fixtures/effect_lowered/handle_finally_boundary.scoop`
  10. `tests/fixtures/effect_lowered/handle_perform.scoop`
  11. `tests/fixtures/effect_lowered/nested_handle_self_contained_vs_outward.scoop`
  12. `tests/fixtures/mir_materialized/pass_pipeline_metadata.scoop`
  13. `tests/fixtures/run-pass/continuation_escape_binder_resume_effect_row_runtime_basic.scoop`
  14. `tests/fixtures/run-pass/continuation_resume_answer_replay_basic.scoop`
  15. `tests/fixtures/run-pass/delegated_property_observable_vetoable_basic.scoop`
  16. `tests/fixtures/run-pass/delegated_property_observable_vetoable_concurrency_ok.scoop`
  17. `tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure.scoop`
  18. `tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_locals.scoop`
  19. `tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop`
  20. `tests/fixtures/run-pass/effect_handle_return_from_function_any_boxing.scoop`
  21. `tests/fixtures/run-pass/effect_indirect_perform_nonresuming_function_value_higher_order_when_direct.scoop`
  22. `tests/fixtures/run-pass/effect_indirect_perform_nonresuming_function_value_wrapper_member_direct.scoop`
- 必须实现的内容：
  1. P4 `BodyFactsBuilder` 对 call-site target 的处理只读取 T2-03A1 发布的 per-site target/provenance/surface facts；允许做稳定 key 到 `InstanceKey` 的查表和 schema materialization，不允许做跨 body 参数反查或 points-to 求解。
  2. 删除或避免引入类似 `parameter_candidate_instances` / `fact_target_includes_callable` / “扫描 `mir_fact_index.bodies` 查 caller” 的逻辑。`provenance_target_for_boundary` 若保留，只能消费同一 site/local 已发布的 authoritative provenance，不得递归追 caller。
  3. `CallSiteTarget::CandidateSet` 只能表示闭合候选集合；单候选且闭合时应保持 `KnownInstance`，除非 fact 明确要求保留 candidate-set identity。unknown/open-param 必须走 `DynamicFallback` 或显式 open target 标记。
  4. 恢复测试断言强度：不能让原本期望具体 target 的测试接受 `CandidateSet([])`；不能把 `allowed_row` / `impl_plan` 等关键 contract 断言降级到只匹配字段名前缀，除非任务记录中说明语义变化并新增等价的强断言。
  5. 保持 P5 callee-suspend / cross-call continuation provenance 能为 function-value 和 closure indirect perform 恢复正确 route；不得通过禁用 fact-only call-site lowering、禁用 dynamic invoke、或绕过 continuation wrapper/projection 校验来通过 fixtures。
- 验证：`cargo fmt`；`cargo test -p scoop --test p7_default_pipeline single_pipeline_runs_higher_order_function_value_handled_effect_cli`；`cargo test -p scoop --test p7_default_pipeline single_pipeline_runs_indirect_perform_closure_resume_cli`；`cargo test -p scoop --test p7_default_pipeline`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。
- 完成条件：两个 p7 regressions 恢复通过；P4 代码中不存在 higher-order callable 的跨 body 反向重建；测试未通过空候选/弱断言掩盖 target contract 缺失；fact-only call-site lowering 保持开启。
- 依赖：T2-03A2a
- 完成记录：2026-06-02 完成。复核 P4 `BodyFactsBuilder` 当前只消费 MIR-published per-site target/provenance/surface facts，未保留 `parameter_candidate_instances` / `fact_target_includes_callable` / 扫描 `mir_fact_index.bodies` 反解 caller 参数的 higher-order 反向重建；`provenance_target_for_boundary` 仅消费当前 site/carrier local 已发布 provenance。修复 MIR fact 发布中 owner effect-param substitution 的完整性：`MaterializedCallableEffectTemplate` 携带 eff 参数声明顺序，MIR stage 在 callable rows、call-site surface/event rows 与 fallback function rows 中按 `InstanceKey.eff_args` 替换 effect row params，省略 eff args 时按 materializer 语义视为 Pure，避免 `ObservableProperty` / `VetoableProperty` 这类“type param + eff param” owner 把 `eff_param(...)` 泄漏给 P4；新增 `mir_callable_instance_effect_rows_substitute_owner_eff_param_after_type_params` 回归测试。恢复并验证 higher-order/function-value/closure continuation regressions；`effect_handle_return_from_function_any_boxing` 改为断言调用前后 heap object 增量为 1，避免 effect-step `main` 自身 frame/effect-context 基线污染原 boxed-return 保活断言。同步已确认语义正确的 effect facts、effect lowered、MIR metadata goldens，反映 DynamicFallback/has_suspend_boundary、Pure residual continuation surface row、call surface/provenance 发布数量变化。验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test -p scoop --test p7_default_pipeline -- --nocapture`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py` 完整通过（1664 checks）。

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
- 依赖：T2-03A2
- 完成记录：（待填）

### [TODO] T2-03R：Review T2-03
- 验证：`python3 tools/run_fixtures.py`
- 依赖：T2-03
- 完成记录：（待填）
