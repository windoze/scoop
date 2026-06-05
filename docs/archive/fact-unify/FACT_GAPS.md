# FACT_GAPS

本报告记录当前编译器 pipeline 中观察到的 fact gap：上游阶段已经知道或应该能稳定发布的语义事实没有进入 handoff/fact artifact，导致下游阶段只能重新扫描 AST/HIR/MIR/LIR、通过 FQN/path/span/字符串签名反查，或从 IR 形状再次推导。

## Scope

当前 production pipeline 大致为：

1. P1 AST：`crates/scoopc/src/pipeline/ast_stage.rs`。
2. P2 typed HIR：`HirStageOutput { LoweredHir, HirFacts }`，见 `crates/scoopc_hir/src/stage.rs:1929-1972`。
3. P3 direct-style MIR + materialized MIR：`MirStageOutput { lowered_mir, mir_facts, materialized_mir }`，见 `crates/scoopc/src/pipeline/mir_stage.rs:60-78`。
4. P4 effect facts：`EffectFactsStageOutput { MaterializedEffectFacts, published_effect_facts }`，见 `crates/scoopc/src/pipeline/effect_facts_stage.rs:11-23`。
5. P5 late-lowered LIR + LIR facts：`LirStageOutput { LateLoweredProgram, LirFacts }`，见 `crates/scoopc/src/pipeline/effect_lowering_stage.rs:38-60`。
6. P6 LLVM codegen：`LlvmCodegenStageInput/Output`，见 `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`。

判断标准：如果下游只是把已有 IR 翻译成自己的事实产品，不算 gap；如果下游必须重新跑前端索引、重新遍历更早 IR、通过字符串/跨度/唯一匹配反查，或注释明确说是在 fallback/recover/reconstruct，则计入。

## Findings

### FG-01: materializer 重新从 AST/HIR 建 generic template、body、site binding 目录

证据：

- `crates/scoopc_mir/src/mir/materialize/entry.rs:63-75`：materialization 入口从 `compilation_unit` 调 `collect_generic_template_infos_with_source_cones`、`collect_callable_body_infos`、`collect_site_instance_bindings`，随后又 `lower_generic_for_compilation_unit_multi_files_with_type_env`。
- `crates/scoopc_mir/src/mir/materialize/templates.rs:295-377`：遍历 AST item/type/object body 收集 generic template。
- `crates/scoopc_mir/src/mir/materialize/templates.rs:406-505`：用 AST function/property 名称和 owner FQN 重建 callable body lookup key。

上游缺失 fact：HIR handoff 没有发布 materializer-ready 的 `GenericTemplateFact`、`CallableBodyFact`、`SiteInstanceBindingFact`，尤其缺少 canonical template identity、stable template key、body root、source owner 和 type/effect arg provenance。

下游重建方式：materializer 持有 AST side table，再扫完整 compilation unit，按 FQN + source path + decl span 生成 template/body catalog，并重新 lower generic HIR。

风险：AST/HIR/HirFacts 三套来源容易漂移；span 或 owner FQN 规则变化会破坏 materialization；cache-hit dependency 的事实无法只靠 artifact 消费。

建议：在 HIR facts 中发布稳定的 template/body/site binding inventory，materializer 只消费 fact artifact，不再扫描 AST 或重新构造 lookup key。

### FG-02: `MonomorphRequest` 缺 canonical template identity

证据：

- `crates/scoopc_hir/src/monomorph.rs:9-15`、`47-53`：request 只携带 `fqn`、`decl_file`、`decl_span`、`request_source_path`、`call_span`。
- `crates/scoopc_hir/src/typecheck/lower.rs:1081-1119`：typecheck 记录 monomorph call 时写入 FQN/file/span 和 type/effect args。
- `crates/scoopc_mir/src/mir/materialize/seed.rs:6-25`：materializer 先精确用 FQN/file/span 找 template，失败后退到同 FQN/file 的唯一匹配。
- `crates/scoopc_mir/src/mir/materialize/seed.rs:70-79`：再跨 `TypeStore` re-intern type/effect args。

上游缺失 fact：request 中缺少 `TemplateKey` / `StableTemplateKey` / declaration identity，也缺少 type/effect args 所属 type universe 的显式 provenance。

下游重建方式：用 `(fqn, decl_file, decl_span)` 查 `request_templates`，失败时用 `(fqn, decl_file)` 唯一性兜底，并把 type/effect args 重新 intern 到 materialized MIR 的 `TypeStore`。

风险：重载、span 漂移、同文件同名声明、import/sysroot 移动都会让 fallback 误配或漏配。

建议：`MonomorphRequest` 直接携带 stable template/definition key；type/effect args 使用可验证的 cross-store encoding 或显式 source type-context id。

### FG-03: generic direct-call instance inventory 由下游遍历 HIR 推导

证据：

- `crates/scoopc_mir/src/mir/materialize/hir_calls.rs:1`：文件说明是 HIR direct-call analysis，会走 HIR blocks/statements/expressions 来发现实例。
- `crates/scoopc_mir/src/mir/materialize/hir_calls.rs:14-23`：从 HIR fun/member fun 建 `templates_by_fqn`。
- `crates/scoopc_mir/src/mir/materialize/hir_calls.rs:66-95`：逐个 body 收集 direct-call instances。
- `crates/scoopc_mir/src/mir/materialize/hir_calls.rs:259-367`：从 AST `top_level_fun_call_bindings` 或从实参类型/参数类型匹配中推导 `InstanceKey`。

上游缺失 fact：P2/P3 未发布 per-body/per-call-site 的 generic direct-call instance inventory。

下游重建方式：materializer 重走 HIR 表达式树，按 call span 查 AST binding，或按参数名/位置匹配签名再从实参类型反推出 type args。

风险：复制 typecheck/overload/generic inference 的逻辑；新增 HIR 节点或 call sugar 时容易漏扫。

建议：HIR facts 发布 `CallSiteInstanceFact { source_site, template_key, stable_instance_key, type_args, eff_args }`，MIR materializer 只做实例队列调度。

### FG-04: MIR direct call 只保存字符串 FQN，materializer 再解析真实 callee/instance

证据：

- `crates/scoopc_mir/src/mir/mod.rs:2648-2657`：`CallKind::Direct` 只有 `callee_fqn: String`。
- `crates/scoopc_mir/src/mir/materialize/dispatch.rs:152-204`：先按 overlapping direct call binding，再按 FQN，再按 receiver type 查 non-generic callee。
- `crates/scoopc_mir/src/mir/materialize/rewrite.rs:1400-1431`：从 result type 反推 top-level ref instance。
- `crates/scoopc_mir/src/mir/materialize/rewrite.rs:1472-1628`：direct call instance 推断使用 site binding、FQN candidates、receiver type、arg/result type bindings。

上游缺失 fact：MIR call metadata 没有携带 resolved declaration key、template key、stable instance key、overload identity。

下游重建方式：用 FQN 查 roots，再用 receiver type、参数/返回类型、source span binding 和唯一候选筛选补出 instance。

风险：同名 overload、owner-specialization、effect-only generic、同形签名都可能让唯一匹配失败或选错。

建议：MIR `CallKind::Direct` 或附属 metadata 直接保存 selected callee definition key 与 concrete `InstanceKey`；FQN 只作为 display/debug。

### FG-05: owner-effect dispatch target 需要从 receiver type 恢复 effect args

证据：

- `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs:1816-1833`：注释说明 AST-only dispatch itable 给的是 bare base FQN，owner-`eff` class 需要从 receiver type 恢复 concrete dispatch target 的 owner effect row。
- `crates/scoopc_mir/src/mir/materialize/reachable.rs:868-952`：遍历 `TypeStore` 的所有 nominal 实例，为 owner-effect member seed dispatch candidate instances。
- `crates/scoopc/src/pipeline/lir_facts_builder.rs:37-48`、`152-176`：LIR facts builder 为 owner-eff instances 建 loose instance signature index。
- `crates/scoopc/src/pipeline/lir_facts_builder.rs:1974-1999`：先用 bare `template.fqn` 查 target，失败后用 loose signature，否则生成 unpublished callable key。

上游缺失 fact：dispatch facts 没有直接发布带 owner type/effect args 的 candidate `InstanceKey` 集合。

下游重建方式：P4 从 receiver type 补 `eff_args`；materializer 扫所有 concrete nominal types seed candidates；LIR facts 再用 `(base fqn, type args, eff args)` 的字符串签名匹配已发布 callable。

风险：bare FQN 与 eff-mangled root 双轨；TypeId 字符串签名对 type universe 敏感；unpublished key 会让 reachability/dispatch facts 不完整。

建议：HIR/P3 dispatch contract 发布 `DispatchCandidateFact { site, dispatch_kind, receiver_ty, stable_instance_keys }`，并把 owner effect args 纳入 canonical target identity。

### FG-06: hidden initializer effects 在 MIR lowering 中扫描 HIR 重算

证据：

- `crates/scoopc_mir/src/mir/lower/mir_lowering_facts.rs:266-295`：`with_class_ctor_hidden_effects` 为 ctor/object/top-level value 调 `HiddenInitEffectAnalyzer`。
- `crates/scoopc_mir/src/mir/lower/hidden_init.rs:17-37`：analyzer 暴露 class/object/top-level hidden effect row 查询。
- `crates/scoopc_mir/src/mir/lower/hidden_init.rs:39-108`：递归扫描 super ctor args、default args、delegation、property init、init block、ctor body。

上游缺失 fact：HIR facts 或 effect analysis 没有发布 class ctor/object init/top-level init 的 hidden-effect summary。

下游重建方式：MIR lowering 回看 `LoweredHir`，递归扫描 init 结构和表达式，自己收集 effect terms。

风险：与 P4 effect solver 的 declared/resolved effect 语义可能分叉；递归/循环/保守性规则散落在 MIR lowering 内。

建议：P2/HIR facts 发布 `HiddenInitializerEffectFact`，P3 只搬运到 MIR site metadata，P4 用同一 fact 构造 class-ctor/top-level-ref effect site。

### FG-07: P4 effect facts 重新构建前端 `Index` / `TypeEnv` / dispatch tables

证据：

- `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs:39-41`：注释说明 cache-hit dep 不在 compilation sources 时，重建 `Index/TypeEnv` 要重放 cached frontend imports。
- `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs:2209-2215`：build 时调用 `EffectFactsTypeContext::build`。
- `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs:2313-2352`：收集 sources、parse、`build_top_level_index`、`TypeEnv::from_sysroot`、`extend_from_file`。
- `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs:2354-2364`：把 cached cone imports 注入重建出的 index/env。
- `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs:2376-2380`：重新 collect vtable/itable/direct subclasses。
- `crates/scoopc/src/pipeline/llvm_codegen_stage.rs:59-62`：LLVM input 也记录 cached imports 是为了 effect-facts stage 重建时可见。

上游缺失 fact：前端/HIR handoff 没有可复用的 declaration/type environment artifact，也没有 per-cone public API、vtable、itable、direct subclass facts 的稳定产品。

下游重建方式：P4 重新 parse sources、build index/env、注入 cached dep API、再 collect dispatch/type hierarchy tables。

风险：effect facts 与前端解析/索引逻辑耦合；cache-hit path 一旦漏注入就会缺 public API；P4 不是纯消费 P3/P4 handoff。

建议：把 declaration/type environment、dispatch inventory、public API import 作为 HIR facts 或 per-cone artifact 显式传递给 P4。

### FG-08: P4 effect facts 从 materialized MIR shape 重建 site/block effect event stream

证据：

- `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs:729-766`：逐 block 调 `scan_block_sites`，再构造 block/site solver facts。
- `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs:800-945`：遍历 MIR statements/terminators，识别 `Call`、`ClassCtor`、hidden `TopLevelRef`/`MemberAccess`、`Perform`、`Handle`。
- `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs:776-798`：从 CFG successor 递归标记 handled region。

上游缺失 fact：P3 MIR/MirFacts 未发布结构化 effect event stream、block-to-site inventory、handled-region/cleanup/suspend boundary facts。

下游重建方式：P4 扫 materialized MIR 的 statement/terminator enum，维护 block scan cache、handled tags、region DFS。

风险：effect facts 与 materialized MIR 具体 shape 和 pass 顺序强耦合；新增 MIR rvalue/terminator 时 P4 容易漏处理。

建议：MIR lowering 或 materialization 发布 `MirEffectEventFact`、`MirBlockEffectRegionFact`、`MirSiteInventoryFact`，P4 solver 只消费这些 facts。

### FG-09: P4 call-site target/declared row 使用 FQN、arg count、receiver type 和 fallback 重建

证据：

- `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs:1645-1801`：`build_direct_like_call_site` 先尝试 known callable key，否则从 raw fun、callable value surface、property accessor、surface callable contract 推 declared row；失败时可能 `DynamicFallback` / `SignatureFallback`。
- `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs:1804-1873`：dispatch call site 先解析 candidate keys，否则回 direct-like fallback。
- `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs:1875-1912`：candidate rows 也按 FQN/arg count/receiver type union。
- `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs:1954-1966`：dynamic callable value fallback 直接使用 owner callable step schema 的 full case set。

上游缺失 fact：MIR call site 缺少 authoritative selected callable instance、declared effect row、surface signature、candidate rows 和 precision source。

下游重建方式：P4 通过 FQN + explicit arg count + has receiver/receiver type 查询 overload/env，再用 raw MIR/HIR-derived surface contract 补 declared row；无法确定时降级为 dynamic/signature fallback。

风险：overload/extension/member/property accessor 规则重复；fallback 会扩大 effect precision，可能掩盖上游漏事实。

建议：MIR site metadata 或 HIR facts 发布 `CallSiteTargetFact` 和 `CallSiteSurfaceEffectFact`，P4 不再进行 overload 选择。

### FG-10: callable value / closure provenance 在 P4 中重新做数据流恢复

证据：

- `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs:1410-1460`：按 local 扫所有 assignments，要求唯一 provenance。
- `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs:1462-1510`：从 `Use`、`Transport`、`TopLevelRef`、`MakeClosure`、`MemberAccess`、direct-call result 恢复 provenance。
- `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs:1513-1544`：direct-call result provenance 通过 `summarize_pass_rewritten_fun` 缓存补出。
- `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs:1546-1613`：如果 result 是 param，又回到 call args 继续追踪。

上游缺失 fact：MIR/MirFacts 没有发布 function value points-to/provenance fact，也没有 pass-rewritten summary 的稳定查询面。

下游重建方式：P4 做局部 assignment 扫描和简化数据流，递归追踪 local/param/direct-call result。

风险：非 SSA、多赋值、pass rewrite、transport/closure 新形态都会把 provenance 变成 unknown，进而触发 dynamic fallback。

建议：MIR pass artifacts 发布 `CallableValueProvenanceFact` / `ResultProvenanceFact`，P4 只校验与 call site fact 一致。

### FG-11: plain local control 的 owner `StepSchema` 可由 P5 从 site facts 反推

证据：

- `crates/scoopc_effect_facts_stage/src/effect_facts/solver.rs:348-363`：solver 仅在 plain callable 且 body 需要 local control 时写 `local_control_step_schema`。
- `crates/scoopc_lir/src/effect_lowered/builder.rs:722-724`：P5 入口调用 `discover_plain_local_effect_control_step_schema`。
- `crates/scoopc_lir/src/effect_lowered/builder.rs:860-922`：如果 body fact 没有 local-control schema，则从 class-ctor/perform/handle continuation owner step schema 收集唯一候选。

上游缺失 fact：`BodyEffectFacts.local_control_step_schema` 不是强制 contract；缺失时 P5 还能容忍并反推。

下游重建方式：P5 扫 body site facts 和 continuation schema，寻找唯一 owner `StepSchema`。

风险：单候选 fallback 会掩盖 P4 漏发；多候选才报错，错误延迟到 lowering。

建议：将 local control owner schema 设为 P4 必发 fact；P5 遇缺失直接 fail-fast，删除反推 fallback。

### FG-12: P5 boundary operand/result source contracts 从 MIR source slice 中恢复

证据：

- `crates/scoopc_lir/src/effect_lowered/materialize/dispatch_plan.rs:7-34`：扫描 MIR body statements，把 `site_id -> result_local` 收集出来。
- `crates/scoopc_lir/src/effect_lowered/materialize/main.rs:147-208`：boundary materialization 查 result local，再调用 operand contract builder。
- `crates/scoopc_lir/src/effect_lowered/materialize/contract_op.rs:33-169`：在 owner state 的 `source_slices` 中找 call statement anchor，并从 MIR call kind/args 构造 carrier/arg sources。
- `crates/scoopc_lir/src/effect_lowered/materialize/contract_step.rs:690-929`：从 operand/local decl、local assignment、closure env tuple、expected carrier components 推 source type 和 source list。

上游缺失 fact：P4/P5 segmentation 没有显式发布 boundary statement anchor、result local、carrier operand source、arg source、closure env decomposition contract。

下游重建方式：P5 回看 direct-style MIR body 与 state graph source slices，按 `site_id` 找 statement，再从 local decl/assignment 和 carrier type 推导 source list。

风险：source slice 调整、优化移动、常量 carrier、同类型多个 local 都可能导致 contract 不稳定或 unsupported。

建议：在 boundary discovery/segmentation 阶段发布结构化 `BoundarySourceContract`，LIR materialization 只消费该 contract。

### FG-13: MIR facts 的 materialized instance/backend contract 不完整，后续仍依赖 `MaterializedMir` side tables

证据：

- `crates/scoopc_mir_facts/src/families.rs:22-30`：`InstanceInventoryEntry` 只发布 `type_args` 和 body reference，没有 `eff_args`、layout、dispatch/backend contract。
- `crates/scoopc/src/pipeline/mir_stage.rs:358-369`：MIR stage 构建 instance inventory 时也只写 `family.key().type_args`。
- `crates/scoopc_mir/src/mir/materialize/mod.rs:88-101`：`MaterializedBackendContracts` 额外保存 enum/class init/vtable/itable/extern/native/top-level/object init 等 backend contracts。
- `crates/scoopc_mir/src/mir/materialize/run.rs:243-253`：这些 backend contracts 在 materializer 结束时从 HIR-derived side tables 克隆进 `MaterializedMir`。

上游缺失 fact：MirFacts 未完整发布 materialized instance identity 和 backend ABI/layout/dispatch/global contracts。

下游重建方式：LIR/LLVM 不只消费 MirFacts，还回看 `MaterializedMir.backend_contracts()`、`source_callable_signatures()` 和 pass view。

风险：MirFacts 不是自包含 artifact；facts 与 side tables 可能成为双 source of truth，跨进程/缓存消费成本高。

建议：扩展 MIR facts family/metadata/root detail，发布 `eff_args`、layout、vtable/itable、extern/native linkage、global init contract，并把 `MaterializedBackendContracts` 收口为 fact artifact。

### FG-14: LIR facts builder 重新索引 callable key，并用 loose signature/unpublished key 兜底

证据：

- `crates/scoopc/src/pipeline/lir_facts_builder.rs:68-80`：build facts 时现场构造 `callable_keys_by_root`、`callable_keys_by_instance`、`body_versions_by_key`。
- `crates/scoopc/src/pipeline/lir_facts_builder.rs:152-176`：用 `(base fqn, type-arg ids, eff-term ids)` 的 loose string signature 解决 dispatch target 与 published callable 的 `TemplateKey` source/span 不一致。
- `crates/scoopc/src/pipeline/lir_facts_builder.rs:1974-1999`：target callable 先 root FQN，后 loose signature，再 synthetic `unpublished(...)`。
- `crates/scoopc/src/pipeline/lir_facts_builder.rs:2170-2188`：owner body version key 缺 stable owner 时返回 `missing-owner` 或 unpublished key 派生值。

上游缺失 fact：LIR IR / P5 lowering 未把 stable `LirCallableKey`、body version key、target callable key 作为每个 callable/call target 的显式字段。

下游重建方式：facts builder 扫 LIR callables 生成 key index，遇 owner-eff/generic target 时用 loose string signature 近似匹配。

风险：TypeId local numbering、TemplateKey span、root FQN mangling 变化会影响匹配；unpublished/missing-owner key 可能让 reachability 和 ABI facts 残缺。

建议：LateLoweredProgram 在构造 callable/call target 时即保存 stable callable/body-version key，EffectFacts 的 `CallSiteTarget` 也携带对应 LIR key 或可无损映射的 stable instance key。

### FG-15: LIR facts 从 `MaterializedMir` 重新发布 source signature 和 dynamic invoke call-site metadata

证据：

- `crates/scoopc/src/pipeline/lir_facts_builder.rs:1066-1162`：从 `materialized.source_callable_signatures()`、`materialized.file.items`、caller-side pass bodies、call statements 重新发布 source signatures。
- `crates/scoopc/src/pipeline/lir_facts_builder.rs:1652-1760`：遍历 state graph source slices 和 materialized MIR statements，发布 dynamic invoke / dispatch contracts。
- `crates/scoopc/src/pipeline/lir_facts_builder.rs:2335-2463`：按 root FQN/site id 在 materialized MIR 中查 callable body、signature、call-site kind/arg count/carrier source type。

上游缺失 fact：P5 LIR handoff 没有自带 source callable signature、call-site materialized metadata、dynamic invoke source contract。

下游重建方式：LIR facts builder 回到 materialized MIR pass view、raw file items、caller-side pass candidates 和 source slices 做扫描/查找。

风险：LIR facts 依赖 P3/P4/P5 三层结构同步；pass rewrite 或 source slice 变化会影响 fact 发布。

建议：P5 在生成 dynamic invoke 和 boundary lowering 时直接发布 source signature、call-site metadata、dispatch source contract，LIR facts builder 只序列化。

### FG-16: LLVM stage 仍从 HIR/HirFacts/MaterializedMir 重建 codegen base context

证据：

- `crates/scoopc/src/pipeline/llvm_codegen_stage.rs:112-147`：从 HIR fun/member fun 和 HIR dispatch call sites 构造 callable source contract / dispatch call contract。
- `crates/scoopc/src/pipeline/llvm_codegen_stage.rs:149-245`：从 `HirFacts` 转换 ordinary callee effect analysis facts。
- `crates/scoopc/src/pipeline/llvm_codegen_stage.rs:247-351`：合并 `LoweredHir` 和 `MaterializedMir.backend_contracts()` 里的 layouts、globals、vtable/itable、ctor/effect/source-site side tables，构造 `LlvmStageBaseContext`。
- `crates/scoopc/src/pipeline/llvm_codegen_stage.rs:366-412`：LLVM stage 重新生成 HIR facts、MIR stage output、effect facts、LIR output，然后再构造 base context。

上游缺失 fact：LIR/LirFacts 还不足以独立供 LLVM 消费 ordinary callee、source-site、layout/global/dispatch/base contracts。

下游重建方式：LLVM stage 保留 `LoweredHir` clone、HirFacts 和 MaterializedMir side tables，重新组装 codegen context。

风险：P6 并非只消费 LIR handoff；source span/FQN side table 与 LIR facts 可能漂移；缓存/跨进程 artifact 边界不清晰。

建议：把 LLVM base context 所需信息收口到 HIR/MIR/LIR facts artifact，P6 输入只保留 LIR program + LIR facts + type/context artifacts。

### FG-17: LLVM call lowering 用 FQN 字符串和多级 signature fallback 恢复 exact LIR root

证据：

- `crates/scoopc_codegen_llvm/src/llvm/codegen/call/lowering.rs:16-25`：从 `foo::<Bar>` / `$overload$` 字符串剥出 dispatch FQN。
- `crates/scoopc_codegen_llvm/src/llvm/codegen/call/lowering.rs:1766-1806`：扫描 published LIR callable symbols，按 dispatch FQN、arity、return type display 匹配 exact LIR root。
- `crates/scoopc_codegen_llvm/src/llvm/codegen/call/lowering.rs:1809-1856`：signature 先查 callable root，再退到 dispatch FQN。
- `crates/scoopc_codegen_llvm/src/llvm/codegen/call/abi.rs:396-431`：published callable signature 再按 callable facts、source signatures、symbol facts 多级 fallback。
- `crates/scoopc_codegen_llvm/src/llvm/codegen/gc.rs:6-36`：GC dispatch target 用 FQN prefix 扫 candidate。

上游缺失 fact：LIR call site 没有携带 exact target root、ABI symbol、source/codegen signature owner。

下游重建方式：LLVM 解析 FQN string，扫描 published LIR facts，按 arity/return type display/唯一 prefix 匹配。

风险：generic/owner-eff/overload 名字规则或 display 文本变化会影响 codegen target；多级 fallback 增加行为分叉。

建议：LIR facts 发布 per-call `ExactCalleeBinding { target_callable_key, root_fqn, abi_symbol, signature_key }`，LLVM 不再从字符串恢复。

### FG-18: LLVM stable layout/symbol/closure path 仍由 canonical text 与 FQN 规则重建

证据：

- `crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/stable_naming.rs:24-49`：LLVM 根据 key text 重新 mangle private symbol/type name。
- `crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/stable_naming.rs:52-97`：callable version key text 在 LLVM 内重新由 program、effect row、impl plan 构造。
- `crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/layout/state_machine.rs:15-35`：step layout 名称由 stable naming helper 现场生成。
- `crates/scoopc_codegen_llvm/src/llvm/codegen/main/identity.rs:370-400`：closure lexical path 通过 `.$lambda` FQN 字符串从 owner 恢复。
- `crates/scoopc_codegen_llvm/src/llvm/codegen/mod.rs:1139-1149`：注释说明要 walk HIR lexical order 才能让 materialized-MIR closure helpers 恢复同一 `$lambdaN.$lambdaM` path。

上游缺失 fact：LIR facts 没有完整发布 ABI-visible stable symbol/layout names、closure lexical path、callable version naming contract。

下游重建方式：LLVM 用 canonical text、private mangler、effect row/impl plan/step cases 现场生成名字，并从 closure FQN convention 还原 lexical path。

风险：ABI name drift 难以发现；closure FQN convention 与 HIR lexical walk 强耦合；缓存产物需要重复实现 naming 逻辑。

建议：LIR facts 发布 `AbiSymbolFact`、`LayoutNameFact`、`ClosureIdentityFact`，LLVM 只校验并使用这些名字。

## Cross-Cutting Patterns

1. 字符串身份过多：FQN、`::<...>`、`$overload$`、`.$lambda`、TypeId display 被用于语义匹配。建议把 display identity 与 semantic identity 分离，semantic path 全部使用 stable keys。
2. Span/path fallback 仍在核心路径：monomorph、template lookup、call-site binding 都依赖 source path/span。建议把 path/span 只作为 diagnostic anchor，事实主键使用 stable declaration/site key。
3. Fact artifact 不自包含：MirFacts/LirFacts 仍需要 `MaterializedMir`、`LoweredHir`、AST side tables、backend contracts 补全。建议定义每个 stage 的 artifact-complete contract，并用 verifier 禁止回看更早 side tables。
4. Fallback 容忍掩盖缺 fact：`DynamicFallback`、`SignatureFallback`、`unpublished(...)`、`missing-owner`、唯一候选推断会让上游漏发延迟到后端或被保守放宽。建议按阶段逐步改为 fail-fast。
5. TypeStore 本地 id 泄漏到跨阶段匹配：loose instance signature 和 LLVM return type display 都受本地 type universe 影响。建议 stable type/effect encoding 成为跨 artifact 主键的一部分。

## Suggested Priorities

1. 先修 identity：`MonomorphRequest`、MIR direct call、dispatch candidate、LIR callable/call target 都应携带 stable semantic keys。
2. 再修 self-contained artifacts：把 `MaterializedBackendContracts`、source signatures、dynamic invoke/source contracts 移入 MIR/LIR facts。
3. 最后删除 downstream fallback：按 P4、P5、P6 分阶段把唯一候选、signature fallback、unpublished key 改为 verifier error。
