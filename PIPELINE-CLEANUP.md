# Pipeline Cleanup 分析报告

## 本次复核结论

本次按当前代码重新抽查后，报告需要做的修改主要有两类：

1. 把“多个阶段发布的 fact table 职责重叠，并被下游混合/替代使用”提升为显式问题，而不是只作为边界模糊的附带现象。
2. 把“effect 命名已经覆盖非 effect 语义”的问题单独列出来，避免命名继续掩盖真实职责边界。

除此之外，原报告关于以下主结论在当前代码上仍然成立：

1. 生产 HIR lowering 仍反向依赖 MIR materialization。
2. `llvm_codegen_stage` 仍会从 `LoweredHir` 重新驱动 MIR/effect-facts/effect-lowering。
3. codegen 仍在主线上混用 HIR、raw MIR、late-lowered program 和多套 fact/type 查询面。

## 目标边界

在“`effect_lowered` 就是正式 LIR”这个前提下，期望的生产 pipeline 形状是：

```text
AST -> HIR -> MIR -> effect facts -> LIR -> codegen
```

这里默认的边界约束是：

1. 每个阶段只消费前一阶段的输出，以及明确发布的 fact table。
2. 阶段输出不应把后续阶段的产物“顺手挂回去”。
3. 下游阶段不应回头重跑上游阶段。
4. codegen 应有单一的 authoritative IR 输入（这里就是 LIR）；若仍需要更早阶段的 fact table，这些 fact 的职责必须明确且互不重叠。

## 目标 crate 划分与依赖规则

下面用 `scoopc_*` 作为占位命名；重点是层次和依赖方向，不是最终 crate 名字。

### 基础 crate

这些 crate 不属于任何阶段，也不属于任何 fact；它们是所有阶段/fact 都可依赖的公共基础：

1. `scoopc_span`：`Span`、位置/诊断基础类型。
2. `scoopc_source`：`SourceId`、`SourceMap`、source identity。
3. `scoopc_types`：`TypeId`、`TypeStore`、`EffectRow`、builtin type universe。
4. `scoopc_ids`：`SiteId`、`InstanceKey`、`BodyVersionKey`、各种稳定 identity key。
5. `scoopc_project_model`：source-cone / project / compilation-unit membership 等与具体阶段无关的工程模型。

如果某个 fact 需要引用别的阶段/事实里的实体，它只能通过这些基础 ID/type crate 间接表达，不能直接依赖别的 stage/fact crate。

### 目标 stage crate / fact crate

1. `scoopc_ast`：AST stage crate。
2. `scoopc_ast_facts`：AST stage 发布的 parser-owned global facts。若 AST stage 没有真正的 global facts，这个 crate 可以不存在。
3. `scoopc_hir`：HIR stage crate。
4. `scoopc_hir_facts`：HIR stage facts crate。
5. `scoopc_mir`：MIR stage crate。
6. `scoopc_mir_facts`：MIR stage facts crate。
7. `scoopc_effect_facts_stage`：effect facts 计算 stage crate。
8. `scoopc_effect_facts`：effect facts crate。
9. `scoopc_lir`：LIR stage crate。当前语义上对应 `effect_lowered`。
10. `scoopc_lir_facts`：LIR stage 发布给 codegen 的 backend-neutral global facts crate。
11. `scoopc_codegen_llvm` / `scoopc_codegen_c`：后端 crate。
12. `scoopc`：薄 facade/orchestrator crate，只负责把这些 stage crate 串起来，不再承载具体阶段实现。

### 允许的依赖方向

#### stage crate

阶段 crate `StageN` 只允许依赖：

1. 基础 crate。
2. 它自己的输出 fact crate `FactN`。
3. 前一个阶段 crate `StageN-1`。
4. 所有更早阶段发布的 fact crate `Fact0..FactN-1`。

阶段 crate 明确禁止依赖：

1. 更早但非直接前驱的 stage crate。
2. 任何后续 stage crate。
3. 任何后续 fact crate。

例子：

1. `scoopc_mir` 可以依赖 `scoopc_hir`、`scoopc_hir_facts`、可选的 `scoopc_ast_facts`，以及自己的 `scoopc_mir_facts`。
2. `scoopc_lir` 可以依赖 `scoopc_mir`、`scoopc_mir_facts`、`scoopc_effect_facts`、`scoopc_hir_facts`，以及自己的 `scoopc_lir_facts`。
3. `scoopc_codegen_llvm` / `scoopc_codegen_c` 原则上只应依赖 `scoopc_lir`、`scoopc_lir_facts` 和基础 crate；若仍需要 `mir/effect_facts/hir_facts`，说明 LIR 还不完整。

#### fact crate

fact crate `FactN` 只允许依赖基础 crate。

fact crate 明确禁止依赖：

1. 任何 stage crate。
2. 任何其它 fact crate。

这条规则的直接含义是：

1. fact crate 里的类型必须自包含。
2. fact crate 不能通过 re-export 或嵌套 wrapper 的方式“顺手带出”别的阶段/别的 fact 的内容。
3. fact crate 若要引用上游实体，只能引用基础 crate 中定义的稳定 ID / type key。

### stage output wrapper 规则

每个阶段对外暴露的 `StageOutput` 应只包装本阶段自己的产物，不包装上一阶段的完整输出。

推荐形状：

1. `AstStageOutput = { ast }`
2. `HirStageOutput = { hir, hir_facts }`
3. `MirStageOutput = { mir, mir_facts }`
4. `EffectFactsStageOutput = { effect_facts }`
5. `LirStageOutput = { lir, lir_facts }`

禁止形状：

1. `CurrentStageOutput { previous_stage_output, my_output }`
2. `CurrentStageOutput` 继续直接暴露 `previous_stage_output()` / `materialized_pass_view()` / `effect_facts()` 这类上游整包查询面。

### 对 fact crate 的语义要求

每个 fact crate 里的事实都必须满足：

1. 有独立含义，不是“为了兼容旧下游先搬一份”。
2. 不与同阶段其它事实表真正重叠。
3. 不与前一阶段或后一阶段的事实表共享职责。
4. 可独立校验；不能必须依赖某个 stage crate 内部 helper 才知道自己是否合法。
5. 下游不应把它与另一个 fact table 当成可替代输入；如果出现替代关系，要么是事实不全，要么是下游设计错误。

## 分阶段输出设计

下面的“应发布的 facts”只讨论每个阶段自己拥有的职责；不包含“顺手把上一阶段整包暴露给下游”的 nested bundle。

### AST stage

应发布的 IR/output：

1. 解析完成的 AST compilation unit。
2. 每个 AST 文件的稳定 source identity、文件顺序和 compilation-unit membership。

应发布的专属 facts：

1. 原则上不需要额外的全局 semantic facts。
2. 如果为了 project/multi-file pipeline 需要轻量 facts，它们也只能是 parser-owned 的 header/stub facts：
   package/import surface、顶层声明 stub、源文件归属的 source-cone/project 信息。

P1 后仍需补齐什么：

1. `AstCompilationUnitOutput` 已作为 cone-level AST handoff 存在，`AstStageOutput` 只保留为单文件 worker / dump helper。
2. production frontend 仍让 resolver/typecheck/HIR 过渡路径消费 build closure；P2 需要在 cone-level AST handoff 上建立正式 HIR barrier 与 `hir_facts`。

### HIR stage

应发布的 IR/output：

1. resolved + typechecked 的 HIR compilation unit。
2. 统一的 `TypeStore` / builtins context。

应发布的专属 facts：

1. 声明/实体事实：
   nominal kind/variance/direct supertypes、struct/enum/class/object/interface metadata、
   extern/native declaration metadata（包括 `@Extern` 与有 body 的 `@CallingConvention`）、
   top-level value metadata、source path -> owning cone metadata、top-level init dependency roots。
2. source-site typed 合同：
   direct/member/extension/virtual/interface/constructor/closure/funptr/intrinsic/
   perform/resume/handle call-site contracts，arg binding，assign/update place contracts，
   extern global root/init root contracts。
3. 若下游分析仍需要 HIR 级共享事实，应由 HIR stage 正式发布一套不与上面重复的
   declaration/program facts，而不是由更后面的阶段从 `LoweredHir` 现场重建。

当前“完整输出”还少什么：

1. 缺少统一、非重叠的 HIR facts 输出面；当前事实被拆散在 `LoweredHir` side table、
   `TypedHirEffectContracts` 和后续重建的 `ProgramFacts` 之间。
   见 `crates/scoopc/src/hir/lower/types.rs:360-409`、
   `crates/scoopc/src/pipeline/hir_stage.rs:1233-1255,1301-1313`、
   `crates/scoopc/src/program_facts.rs:18-31,39-147`。
   另外，当前 `LoweredHir` 已经开始显式发布 `source_cones` 与 `native_callable_funs` 这类更完整的 cone/decl metadata，
   但它们仍只是 side table，还没有收口成独立 `hir_facts`。
   见 `crates/scoopc/src/hir/lower/types.rs:322-350`。
2. `TypedHirStageOutput` 仍以单个 `source_path` 为锚，而不是显式的 compilation-unit context。
   见 `crates/scoopc/src/pipeline/hir_stage.rs:1234-1238,1273-1275`。
3. HIR stage 输出不应再携带 MIR 产物；当前 `LoweredHir` 仍挂着 `materialized_mir`。
   见 `crates/scoopc/src/hir/lower/types.rs:329-337,421-467`。

### MIR stage

应发布的 IR/output：

1. generic direct-style MIR compilation unit。
2. canonical materialized MIR snapshot（针对 request roots / opt level）。
3. canonical MIR pass view / pass artifacts query surface。

应发布的专属 facts：

1. MIR-owned root inventories：callable bodies、initializer roots、extern/global roots、metadata roots。
2. materialization/pass facts：instance family inventory、snapshot binding、summary/escape/pass artifacts。
3. 一切供下游继续使用的语义，都应当已经落在 MIR 节点 metadata 或 MIR-owned facts 上；
   不应再依赖 HIR typed contract fallback。
4. 任何 LIR stage 会稳定复用的 MIR-derived facts，也应由 MIR stage 明确发布，
   例如 nominal direct supertype index 这类从 MIR file 可确定导出的 facts。

当前“完整输出”还少什么：

1. 缺少单一 authoritative MIR handoff；当前 `MirStageOutput` 仍混有
   `LoweredMir + optional MaterializedMir + leaked TypedHirEffectContracts`。
   见 `crates/scoopc/src/pipeline/mir_stage.rs:13-27,29-37,68-84`。
2. 缺少正式的 stage-owned `materialized_pass_view()` / snapshot-binding 查询面；
   当前下游需要从 `MaterializedMir` 或更后阶段包装里回拿。
3. 有些 MIR-derived facts 仍未作为 MIR stage 输出发布，而是在 LIR stage 里重算。
   见 `crates/scoopc/src/pipeline/effect_lowering_stage.rs:85-97,119-131`。

### effect facts stage

应发布的 IR/output：

1. 绑定到 canonical MIR snapshot 的 effect facts output。

应发布的专属 facts：

1. `snapshot_binding`：明确这批 facts 绑定到哪份 MIR snapshot。
2. `callable_facts`：每个 materialized callable/instance 的 outward-effect / step schema /
   impl plan / reentry 等 effect-owned 合同。
3. `bodies`：每个 body/site 的 effect/control 事实。
4. `step_schemas` 与 `continuation_schemas`。

当前“完整输出”还少什么：

1. 缺少一个窄的、stage-owned 的 MIR snapshot reference/context；当前 `EffectFactsStageOutput`
   是通过嵌套整份 `MirStageOutput` 来满足下游需要。
   见 `crates/scoopc/src/pipeline/effect_facts_stage.rs:22-59`。
2. 若下一阶段（LIR）还需要某些 MIR-derived 但非 effect-owned 的 facts，这些 facts 的拥有者
   目前没有被明确下来；当前实现往往在 LIR stage 重算它们。

### LIR stage（当前即 `effect_lowered`）

应发布的 IR/output：

1. formal LIR program（当前对应 `LateLoweredProgram`）。
2. 与该 LIR program 严格配套的 stage-owned context，而不是对上游 stage 输出的整包回看。

应发布的专属 facts：

1. callable inventory 与 body version identity。
2. plain callable facts：ordinary ABI、source slices、本地 effect/control contract。
3. effect-step callable facts：step schema、state graph、frame schema、boundary map、resume state map、
   source statement classifications、dynamic invoke entry。
4. continuation/resume facts：continuation object、surface resume publication、resume packing inventory。
5. 若 codegen 需要额外 query surface，应当是 backend-neutral 的 LIR facts，例如：
   callable surface signature、source-slice lowering contract、dynamic call carrier contract、
   dispatch owner/slot selection结果，而不是要求后端再去扫描 raw MIR/HIR。
6. LIR 使用到的 `TypeId` 若仍依赖上游 `TypeStore`，那么这套 type context 也应成为 LIR output
   的显式组成部分，而不是通过 nested upstream bundle 间接取得。

当前“完整输出”还少什么：

1. 缺少自足的 stage-owned output；当前 `EffectLoweredStageOutput` 本质上仍是
   `EffectFactsStageOutput + LateLoweredProgram` 的包装。
   见 `crates/scoopc/src/pipeline/effect_lowering_stage.rs:30-79`。
2. plain callable codegen 仍需要回读 materialized MIR signature/body，说明 LIR 没有把普通 callable
   surface 发布完整。
   见 `crates/scoopc/src/llvm/codegen/effect_lowered/layout/callable.rs:333-345,418-447`、
   `crates/scoopc/src/llvm/codegen/effect_lowered/body/emitter.rs:13-18`。
3. dynamic-invoke / dispatch contract 仍需要扫描 MIR body 和 HIR declaration tables，说明这些
   backend-neutral 合同尚未正式进入 LIR output。
   见 `crates/scoopc/src/llvm/codegen/effect_lowered/layout/lookup.rs:56-225`。
4. codegen-neutral ABI/query facts 目前仍缺位；现在真正存在的是 LLVM-specific 的
   `ProgramAbiQuery`，而不是所有 backend 都能复用的 LIR-to-backend contract layer。
   见 `crates/scoopc/src/llvm/codegen/effect_lowered/layout/mod.rs:74-85`、
   `crates/scoopc/src/llvm/codegen/effect_lowered/types.rs:2221-2254`。

## 当前有效结构

当前生产路径并不是线性的 `AST -> HIR -> MIR -> effect facts -> LIR -> codegen`，而更接近：

```text
frontend/typecheck
  -> lower_hir_for_codegen_with_request_root_mode(...)
    -> HIR lowering 期间先做 MIR instance/materialization
    -> LoweredHir { materialized_mir: Some(...) }
  -> llvm_codegen_stage::run(...)
    -> 从 LoweredHir 重新构造 TypedHirStageOutput
    -> 重新运行 MIR stage
    -> 重新运行 effect-facts stage
    -> 重新运行 effect-lowering stage
    -> 同时保留一份 hir_compat_scaffold
  -> llvm::emit(...)
    -> 同时消费 HIR scaffold / pass-view / effect facts / late-lowered program / ABI visibility variant
```

这意味着当前实现里同时存在：

1. HIR 反向依赖 MIR。
2. MIR stage 输出继续携带 HIR 合同。
3. codegen stage 重新驱动上游阶段。
4. codegen 同时消费多套跨阶段输入。
5. 仍有一批生产路径直接在 codegen 中跑 HIR-only lowering。
6. 多个阶段/阶段输出发布的全局 fact table 职责重叠，并被下游混合使用。

## 问题清单

### P1. 生产 HIR lowering 反向依赖 MIR materialization

位置：

- `crates/scoopc/src/frontend.rs:521-580`
- `crates/scoopc/src/hir/lower/main/compilation_unit.rs:387-437`

现状：

- `lower_hir_for_codegen_with_request_root_mode(...)` 并不是“先得 HIR，再往后走”。
- 它会进入 `lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_request_sources(...)`。
- 这个 HIR lowering 入口在内部先调用 `mir::materialize_compilation_unit_from_typechecked_inputs_with_options(...)`，拿到 materialized MIR 和 instance keys，再把这些信息反灌到 `LoweredHir`。

为什么是问题：

- 这直接打破了 `HIR -> MIR` 的单向顺序。
- request-root 选择、opt-level、instance collection 这些本应属于 MIR 或更后面的 concern，已经泄漏到 HIR 生产路径。

---

### P2. `LoweredHir` 挂住了 canonical MIR snapshot / pass view

位置：

- `crates/scoopc/src/hir/lower/types.rs:329-337`
- `crates/scoopc/src/hir/lower/types.rs:421-467`
- `crates/scoopc/src/hir/lower/types.rs:470-499`

现状：

- `LoweredHir` 内部持有 `materialized_mir: Option<crate::mir::MaterializedMir>`。
- 它还公开了 `materialized_mir()`、`materialized_callable_view()`、`materialized_pass_view()`、`materialized_mir_mut()` 等接口。
- 同时又保留了 `clone_hir_compat_scaffold_without_materialized_mir()`，说明当前实现自己也承认这是一个过渡性混合对象。

为什么是问题：

- HIR 输出不应继续携带 MIR 阶段产物。
- 这会让后续任何消费者都把 HIR 当成“多阶段 bundle”，而不是清晰的阶段边界。

---

### P3. `MirStageOutput` 同时暴露 HIR 合同和两套 MIR 视图

位置：

- `crates/scoopc/src/pipeline/mir_stage.rs:13-27`
- `crates/scoopc/src/pipeline/mir_stage.rs:29-37`
- `crates/scoopc/src/pipeline/mir_stage.rs:68-84`
- `crates/scoopc/src/pipeline/mir_stage.rs:215-240`

现状：

- `MirStageOutput` 里既有 `lowered_mir: LoweredMir`，又有 `materialized_mir: Option<MaterializedMir>`。
- 它还继续对下游暴露 `effect_contracts: TypedHirEffectContracts`。
- 这些 HIR 合同虽然是 MIR lowering 合法输入，但它们不该继续穿过 MIR stage 暴露给 P4/P5/P6。

为什么是问题：

- MIR stage 输出现在不是一个单一的 MIR handoff，而是：
  `direct-style MIR + optional canonical materialized MIR + leaked HIR contracts`。
- 这使得 P4 之后的阶段天然面临多 source-of-truth。

说明：

- MIR stage 消费 HIR-derived fact table 本身不是问题。
- 问题在于这些 HIR 合同没有被 MIR stage 吃干抹净，而是继续向下泄漏。

---

### P4. effect-facts stage 会在边界处“自动补跑” MIR materialization

位置：

- `crates/scoopc/src/pipeline/mod.rs:101-121`
- `crates/scoopc/src/pipeline/mod.rs:159-163`
- `crates/scoopc/src/pipeline/effect_facts_stage.rs:85-144`

现状：

- `build_effect_facts_stage_output_with_compilation_sources(...)` 会检查 `mir_stage_output.materialized_mir()`。
- 如果没有，它会直接调用 `materialize_direct_style_mir_for_dump(session, source)` 去补挂 canonical materialized MIR。
- P4 stage 内部还会通过 `materialized_mir_mut()` 直接修改输入里的 snapshot。

为什么是问题：

- P4 不应该在 stage 边界回头依赖 `session/source` 去重新 materialize 一次 MIR。
- 正确形状应该是：P3 保证交给 P4 的就是完整 authoritative MIR handoff；P4 不应该自己修输入。

---

### P5. `effect_lowering_stage` 已经是 formal LIR stage，但它的输出形状仍不完整

位置：

- `crates/scoopc/src/pipeline/mod.rs:133-149`
- `crates/scoopc/src/pipeline/effect_lowering_stage.rs:13-33`
- `crates/scoopc/src/effect_lowered/mod.rs:1-13`

现状：

- 当前生产主线里 `effect_lowering_stage` 是一个显式公开的独立阶段。
- 它会从 `EffectFactsStageOutput` 产出 `LateLoweredProgram`。
- 在“`effect_lowered` = 正式 LIR”这个前提下，这条 stage 本身不是问题；问题是它当前发布的
  `EffectLoweredStageOutput` 仍然只是 `program + nested upstream bundle`。

为什么是问题：

- 如果它是正式 LIR stage，那么它就应该成为 codegen 的 authoritative IR handoff。
- 但当前实现里，后续 codegen 仍需要经由 `EffectLoweredStageOutput` 回看
  `effect_facts`、`materialized_pass_view`、`materialized_mir`、`types` 等上游对象，说明
  LIR output 还没有把自己的职责收实。

需要决策：

1. 正式把目标 pipeline 固定为 `AST -> HIR -> MIR -> effect facts -> LIR -> codegen`。
2. 继续补齐 LIR stage output，让 codegen 不再依赖 nested upstream bundle 和 HIR scaffold。

---

### P6. `llvm_codegen_stage` 会从 `LoweredHir` 重新驱动上游阶段

位置：

- `crates/scoopc/src/pipeline/llvm_codegen_stage.rs:139-167`
- `crates/scoopc/src/pipeline/llvm_codegen_stage.rs:208-235`

现状：

- `run_effect_lowered_stage_from_lowered_hir(...)` 会：
  `LoweredHir -> TypedHirStageOutput::new(...) -> mir_stage::run(...) -> build_effect_facts_stage_output(...) -> build_effect_lowered_stage_output(...)`。
- 然后 `llvm_codegen_stage::run(...)` 再把 `hir_compat_scaffold` 和 `effect_lowered_stage_output` 一起打包。

为什么是问题：

- codegen stage 本应消费上游 stage 输出，而不是自己重跑上游 stage。
- 这使得 codegen stage 实际上成了一个“后半条 pipeline 的 orchestrator”，边界非常模糊。

---

### P7. LLVM 项目构建路径会做两次 backend-specific lowering

位置：

- `crates/scoopc/src/pipeline/mod.rs:207-241`
- `crates/scoopc/src/frontend.rs:521-580`
- `crates/scoopc/src/pipeline/llvm_codegen_stage.rs:216-225`

现状：

- `emit_project_llvm_artifact_to_file(...)` 先用 `MirRequestRootMode::EntryMain` 做一次 lowering。
- 再用 `MirRequestRootMode::RequestSources` 做第二次 lowering，专门服务 ABI visibility。
- 后面还会把这两套 `LoweredHir` 分别跑进 late-lowering。

为什么是问题：

- 这是明显的 backend-specific concern 反向影响 frontend/HIR/MIR 输入。
- ABI shell 发布逻辑本应属于 codegen 的 contract/materialization 问题，不该逼着上游阶段重复跑两份路径。

---

### P8. LLVM emit 的 authoritative 输入不是单一 handoff，而是多头拼装

位置：

- `crates/scoopc/src/llvm/emit.rs:30-47`
- `crates/scoopc/src/pipeline/llvm_codegen_stage.rs:68-87`

现状：

- `LoweredCodegenEntry` 同时持有：
  `LoweredHir`、`materialized_pass_view`、`late_lowered_program`、`late_lowered_types`、
  `abi_program`、`abi_types`、`abi_materialized_pass_view`、`abi_effect_facts`。
- `StageEmitInput` 又另外保留 `hir_compat_scaffold` 和两份 `EffectLoweredStageOutput`。

为什么是问题：

- 这说明 codegen 没有一个清晰的“唯一输入对象”。
- 当前输入形态更像把多个阶段产物重新摊平，再让 emit 自己决定到底该看哪一份。

---

### P9. codegen 仍依赖 HIR scaffold，并在 emit 现场从 HIR 重建事实层

位置：

- `crates/scoopc/src/pipeline/llvm_codegen_stage.rs:74-77`
- `crates/scoopc/src/pipeline/llvm_codegen_stage.rs:208-235`
- `crates/scoopc/src/llvm/emit.rs:42-47`
- `crates/scoopc/src/llvm/emit.rs:486-487`

现状：

- `LlvmCodegenStageOutput` 明确保留 `hir_compat_scaffold`。
- `emit.rs` 在真正建 module 的时候，还会做 `ProgramFacts::from_lowered(lowered)`。

为什么是问题：

- 如果 codegen 真正只消费 `MIR + effect facts + codegen-owned facts`，这里就不应再从 HIR 现场拼 `ProgramFacts`。
- 这说明 HIR 仍然是 codegen 的真实输入之一，而不仅是历史包袱。

---

### P10. raw MIR 辅助路径反向依赖 `published_late_lowered_program`

位置：

- `crates/scoopc/src/llvm/codegen/mir_body/operand.rs:56-74`
- `crates/scoopc/src/llvm/codegen/mir_body/call.rs:511-518`
- `crates/scoopc/src/llvm/codegen/call/abi.rs:299-342`

现状：

- MIR helper 会查询 `published_late_lowered_program()`，用它决定：
  `callable` 是否 outward effect、是否需要 explicit hidden ABI、以及已发布 signature。

为什么是问题：

- 这是一个方向反了的依赖：较早表示层的 lowering helper 在回看较晚阶段的 published contract。
- 即使这些逻辑都发生在 LLVM backend 内部，这仍说明 backend 内部的层次不是单向的。

---

### P11. 生产路径里仍有大量 HIR-only lowering

#### P11.1 顶层 immutable value / initializer 仍直接走 HIR

位置：

- `crates/scoopc/src/llvm/codegen/main/immut_value.rs:8-27`
- `crates/scoopc/src/llvm/codegen/main/immut_value.rs:259-332`
- `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:498`

现状：

- top-level immutable initializer 仍直接通过 `codegen_initializer_expr(...)` / `codegen_expr_in_expected_context(...)` 降 HIR expr。
- 即使从 effect-lowered/value 路径访问顶层值，最后也会回到 `codegen_top_level_value_ref(...)`。

#### P11.2 object init 仍直接走 HIR

位置：

- `crates/scoopc/src/llvm/codegen/object_init.rs:246-288`
- `crates/scoopc/src/llvm/codegen/mir_body/member.rs:16-25`
- `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:486-492`

现状：

- object property init 和 `InitBlock` 仍直接遍历 `hir::ObjectInitStep`。
- raw MIR 和 effect-lowered 路径在遇到 object property 时都会回跳到这套 HIR lowering。

#### P11.3 class ctor 仍直接走 HIR init step / ctor body

位置：

- `crates/scoopc/src/llvm/codegen/mir_body/args.rs:44-138`
- `crates/scoopc/src/llvm/codegen/class_ctor.rs:503-525`
- `crates/scoopc/src/llvm/codegen/class_ctor.rs:696-756`
- `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:470-477`

现状：

- 即使入口已经是 MIR/effect-lowered call site，最终仍会回到 `codegen_class_ctor_invoke(...)`。
- 该实现里继续直接跑 `hir::ClassInitStep::PropertyInit`、`hir::ClassInitStep::InitBlock`、secondary ctor body 等。

#### P11.4 closure lowering 仍直接消费 `hir::ClosureExpr`

位置：

- `crates/scoopc/src/llvm/codegen/closure/mod.rs:78-216`
- `crates/scoopc/src/llvm/codegen/main/immut_value.rs:15-17`

现状：

- closure object、closure body、capture env 等仍在 LLVM backend 里从 HIR closure 直接生成。
- 顶层 initializer 也会打到这条路径。

#### P11.5 ordinary callee suspend analysis 仍在 codegen 现场拼 HIR 级分析上下文

位置：

- `crates/scoopc/src/llvm/codegen/ordinary_callee.rs:30-68`

现状：

- `MainCodegen` 会从当前 function env 里的 `hir_ty`、`ProgramFacts`、`materialized_pass_view()` 现场组装 `EffectAnalysisCtx`。

为什么是问题：

- 上面这些都说明：虽然普通 callable body 的主干已经迁到 MIR/late-lowered，但整个 production backend 还没有摆脱 HIR-only lowering。

---

### P12. codegen 同时维护多套 `TypeStore`，并在现场做“等价类型”匹配

位置：

- `crates/scoopc/src/llvm/codegen/ty.rs:34-40`
- `crates/scoopc/src/llvm/codegen/mir_body/types.rs:293-405`

现状：

- `codegen_type_store_for_type_id(...)` 会在 `self.types` 和 `materialized_pass_view().materialized().types` 之间切换。
- `equivalent_codegen_type_id(...)` 会根据 `TypeKind`、mangled nominal args、甚至 display text 去寻找“等价” type id。

为什么是问题：

- 这不是正常的“阶段输入 -> 阶段输出”关系，而是 backend 在自己弥合多套 type universe。
- 它说明 HIR、MIR、late-lowered/codegen 之间没有收敛到单一 authoritative type graph。

---

### P13. frontend / pipeline 公共 API 仍是 backend-specific 的

位置：

- `crates/scoopc/src/lib.rs:47-51`
- `crates/scoopc/src/frontend.rs:521-580`
- `crates/scoopc/src/pipeline/mod.rs:166-243`

现状：

- `lower_hir_for_codegen_with_request_root_mode(...)` 只在 `#[cfg(feature = "llvm")]` 下存在。
- pipeline 公开的是 `LlvmArtifactKind`、`run_llvm_codegen_stage(...)`、`emit_*llvm*`。

为什么是问题：

- 这让 frontend 和 pipeline surface 默认站在 LLVM 视角组织，而不是站在中立阶段边界组织。
- 将来无论是引入 C backend，还是只想把 stage 边界收干净，都会继续受这个 API 形状限制。

---

### P14. codegen 内部的 ABI materialization 混合了逻辑合同与 LLVM 物理类型

位置：

- `crates/scoopc/src/llvm/codegen/effect_lowered/layout/mod.rs:74-85`
- `crates/scoopc/src/llvm/codegen/effect_lowered/types.rs:2221-2254`
- `crates/scoopc/src/llvm/codegen/effect_lowered/body/emitter.rs:9-32`

现状：

- `ProgramAbiQuery` 同时持有：
  `step/frame/boundary/dynamic-invoke/local-runtime-error/handle-dispatch` 这类逻辑合同，
  以及 `StructType`、`FunctionType`、`BasicTypeEnum` 这类 LLVM 物理类型。
- `CallableEmitter` 也同时持有 `LateLoweredProgram`、`MaterializedMirPassView`、`mir::Body`、`ProgramAbiQuery`。

为什么是问题：

- 这不一定是“跨 stage 违规”，但它是一个重要的结构问题。
- 逻辑 ABI 与 LLVM 物理 ABI 混在一起，会让 codegen 难以收口成真正的最后一层，也会阻碍未来的非 LLVM backend。

---

### P15. `effect` 命名已经明显超出 effect 语义边界，污染了模块和类型命名

位置：

- `crates/scoopc/src/effect_lowered/mod.rs:1-29`
- `crates/scoopc/src/effect_lowered/ir.rs:430-507`
- `crates/scoopc/src/pipeline/effect_lowering_stage.rs:15-29`
- `crates/scoopc/src/pipeline/llvm_codegen_stage.rs:71-86`
- `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:1-8`
- `crates/scoopc/src/llvm/codegen/effect_lowered/layout/callable.rs:1-7`
- `crates/scoopc/src/llvm/codegen/effect_lowered/body/main_entry.rs:1-25`

现状：

- `effect_lowered` 这个子系统名字看起来像“只负责 effect lowering”，但它现在承载的是更广义的 late-lowered/LIR 语义。
- `LateLoweredProgram` 明确同时包含 `LateLoweredPlainCallable` 和 `LateLoweredEffectStepCallable`。
- `pipeline/effect_lowering_stage.rs` 也明确说它的输出里既有 plain callable ordinary ABI/source slices，也有 effect-step callable 的 state-machine 合同。
- `llvm/codegen/effect_lowered/value.rs` 文件头直接写着它是 `effect-neutral` value/expression primitives。
- `llvm/codegen/effect_lowered/layout/callable.rs` 与 `body/main_entry.rs` 里有大量 plain callable / ordinary shell / effect-neutral callable 的逻辑，但仍挂在 `effect_lowered` 这条命名树下。
- `EffectLoweredStageOutput` / `effect_lowered_stage_output` 这些名字也在暗示“这是一份 effect 专用 handoff”，但它实际已经是 codegen 的总 handoff 之一，而不是仅 effect 相关信息。

为什么是问题：

- 这会持续误导维护者对边界的理解：看名字会以为某些逻辑是 effect-specific，实际上它们已经是通用 codegen/LIR 设施。
- 这会放大上一轮 refactor 的历史偶然性，让 `effect` 这个词变成“只要是新中间层就都放这里”的桶。
- 这会反过来阻碍真正的 pipeline 清理，因为命名本身在掩盖事实上的职责扩张。

建议的命名原则：

1. 只把真正 effect-specific 的概念保留 `effect` 前缀。
2. 对已经承载 plain callable / ordinary ABI / effect-neutral source slice 的层，优先使用 `late_lowered`、`lir`、`backend_ir`、`published_program` 这类中性名字。
3. 对 value/layout/body 这类已经明显通用化的子模块，避免再把目录名或 stage 名叫成 `effect_lowered/*`，除非其职责确实只服务 effect-step callable。
4. `EffectLoweredStageOutput` 这类 stage/output 命名应与最终确认的阶段边界一致：
   - 若它是正式 LIR handoff，就改成 LIR/late-lowered 风格命名；
   - 若它只是 codegen 内部的 effect 子阶段，就不应继续承载 plain callable 和普通 ABI 信息。

说明：

- 这项问题不是单纯的术语洁癖。
- 它和前面的边界问题是相互强化的：边界一旦模糊，命名就开始漂移；命名一旦漂移，又会让后续继续沿着错误边界堆代码。

---

### P16. HIR stage 同时发布多套职责重叠的全局合同/side table

位置：

- `crates/scoopc/src/hir/lower/types.rs:360-405`
- `crates/scoopc/src/pipeline/hir_stage.rs:879-978`
- `crates/scoopc/src/mir/lower/mir_lowering_facts.rs:39-71`

现状：

- `LoweredHir` 自己就发布了很多全局 side table：
  `top_level_fun_call_sites`、`call_arg_bindings`、`with_update_contracts`、`assign_place_contracts`、
  `ctor_call_sites`、`dispatch_call_sites`、`effect_op_call_sites`、`continuation_resume_call_sites`、
  `when_pat_binding_tys` 等。
- 同时，HIR stage 又额外发布 `TypedHirEffectContracts`，其中包含：
  `continuation_resume_sites`、`perform_sites`、`handle_sites`、`call_site_contracts`、
  `with_update_contracts`、`assign_place_contracts`、`top_level_init_roots`、`extern_global_contracts`。
- MIR lowering 的 `MirLoweringFacts::from_typed_handoff(...)` 明确会同时从 `LoweredHir` side table 和 `TypedHirEffectContracts` 组装事实层。

为什么是问题：

- 这已经不是“同一阶段输出里有不同表”这么简单，而是多张表在描述同一类职责：
  调用点合同、perform/resume/handle 语义、assignment place、top-level init/extern root 等。
- 一旦下游需要同时读两张表，或者需要知道“优先信哪张表”，就说明阶段职责没有收口干净。

结论：

- `LoweredHir` 和 `TypedHirEffectContracts` 之间需要明确切分：
  哪些是 HIR 本体/语法 lowering 必需的 side table，哪些是 HIR stage 对下游发布的 authoritative 合同。
- 不应继续让两边都发布同一职责的事实。

---

### P17. MIR lowering 内部已经把两套合同当成“可替代输入”

位置：

- `crates/scoopc/src/mir/lower/mod.rs:56-86`
- `crates/scoopc/src/mir/lower/mir_lowering_facts.rs:8-37`
- `crates/scoopc/src/mir/lower/mir_lowering_facts.rs:74-128`
- `crates/scoopc/src/mir/lower/mir_lowering_facts.rs:272-283`
- `crates/scoopc/src/mir/lower/fn_lowering_call.rs:842-965`
- `crates/scoopc/src/mir/lower/fn_lowering_call.rs:978-1008`
- `crates/scoopc/src/mir/lower/fn_lowering_effect.rs:250-260`
- `crates/scoopc/src/mir/lower/fn_lowering_basic.rs:187-213`

现状：

- `MirLoweringFacts` 里显式存在 `MirSiteContractSource::{FallbackSideTables, Typed}`。
- `from_lowered_hir(...)` 先从 HIR side table 构造 fallback facts，再调用 `with_typed_contracts(...)` 覆盖它。
- `with_typed_contracts(...)` 会清空 `fallback_resume_site_spans`、`fallback_perform_sites` 等旧表，再把 typed 合同拷进去。
- `canonicalize_perform_args(...)` 先尝试 `perform_metadata(...)`，typed 缺失时再退到 `fallback_perform_site_info(...)`。
- `lower_handle_expr(...)` 在没有 typed handle site 且未启用 typed contracts 时，会退回 `lower_handle_contract_from_hir(...)`。
- `continue_after_placeholder_effect_terminator_if_needed(...)` 也按 `uses_typed_contracts()` 走两种不同语义路径。

为什么是问题：

- 这正是“某个 fact table 有时可以替代另一个 fact table”的明确信号。
- 这意味着当前并没有一个唯一 authoritative 的 MIR-lowering fact source；typed 合同和 fallback side table 都在承担同一职责。

你的判断在这里完全成立：

1. 要么 typed 合同信息不全，所以才需要 fallback side table。
2. 要么 MIR lowering 的设计错误，让下游必须知道“两种合同模式”。

无论哪种，最终都应收敛到：

1. MIR lowering 只接受一套职责完备的事实输入。
2. 不再保留 typed/fallback 双轨语义分支。

---

### P18. `ProgramFacts` 与 `LoweredHir` side table 职责重叠，LLVM codegen 同时持有并混用两者

位置：

- `crates/scoopc/src/program_facts.rs:18-31`
- `crates/scoopc/src/program_facts.rs:39-147`
- `crates/scoopc/src/llvm/codegen/mod.rs:461-507`
- `crates/scoopc/src/llvm/codegen/mod.rs:768-806`
- `crates/scoopc/src/llvm/emit.rs:492-531`
- `crates/scoopc/src/llvm/codegen/main/call.rs:145-152`
- `crates/scoopc/src/expr_facts.rs:19-24`
- `crates/scoopc/src/expr_facts.rs:52-58`
- `crates/scoopc/src/expr_facts.rs:107-118`
- `crates/scoopc/src/expr_facts.rs:141-158`

现状：

- `ProgramFacts` 是从 `LoweredHir` 复制出来的一套共享事实，内容包括：
  `ctor_call_targets`、`continuation_resume_call_sites`、top-level value types、object property types、
  struct/class field types、class super keys 等。
- 但 LLVM codegen 的 `CompilationUnitCodegenCx` 又同时直接持有原始 `LoweredHir` side table：
  `top_level_vars`、`object_inits`、`class_inits`、`ctor_call_sites`、`continuation_resume_call_sites` 等，
  还额外持有一个 `program_facts: Rc<ProgramFacts>`。
- 下游里一部分逻辑直接查原始 HIR side table，另一部分逻辑通过 `ExprFactResolver` 查 `ProgramFacts`。

为什么是问题：

- 这里已经形成两套都能回答“top-level 值类型/字段类型/ctor target/resume site”问题的事实层。
- 这会让下游自然产生“查哪张都行”的混用习惯，而不是依赖单一 authoritative fact source。

结论：

- 如果 `ProgramFacts` 是共享事实层，就应逐步替代重叠的 HIR side table 读取，而不是并存。
- 如果某些信息必须保留在 `LoweredHir`，就应避免 `ProgramFacts` 再复制同一职责。

---

### P19. `EffectLoweredStageOutput` 继续整包暴露上游事实，而不是只发布本阶段专属 handoff

位置：

- `crates/scoopc/src/pipeline/effect_lowering_stage.rs:30-79`
- `crates/scoopc/src/llvm/codegen/effect_lowered/layout/mod.rs:74-85`

现状：

- `EffectLoweredStageOutput` 不只发布 `program()`。
- 它还继续暴露：
  `effect_facts_stage_output()`、`materialized_mir()`、`materialized_pass_view()`、`types()`、`effect_facts()`。
- 这使得后续 codegen 很自然地同时消费：
  `LateLoweredProgram + MaterializedEffectFacts + MaterializedMirPassView + TypeStore`。

为什么是问题：

- 这相当于把 P4/P5 的多张事实表继续打包成“可随手回看”的 nested bundle。
- 一旦下游既能读 `program()`，又能直接回读 `effect_facts()` / `pass_view()`，阶段边界就很难真正收口。

结论：

- 若某阶段输出仍需要把前一阶段整包暴露给下游，说明本阶段 handoff 还不自足。
- 正确方向应是：
  由本阶段只发布自己专属职责的事实；若还需要回看上游事实，要么说明本阶段信息不全，要么说明下游设计还没完成切换。

## 非问题 / 允许的耦合

下面这些不应和上面的边界问题混为一谈：

1. MIR lowering 消费 HIR 导出的 fact table，这是正常的 `HIR -> MIR` 依赖。
2. effect-facts stage 消费 canonical MIR snapshot，这是正常的 `MIR -> effect facts` 依赖。
3. codegen 内部可以继续有自己的子阶段；问题不在于“有子阶段”，而在于当前 codegen 需要跨阶段回看多套上游表示。

## 建议的清理顺序

### 1. 先修正大方向上的反向依赖

1. 让生产 HIR lowering 不再依赖 MIR instance/materialization。
2. 从 `LoweredHir` 移除 `materialized_mir` / `materialized_pass_view`。
3. 让 `MirStageOutput` 只暴露 MIR 自己的 authoritative handoff，不再继续携带 HIR 合同。

### 2. 让 stage 输入变严格

1. `effect_facts` 只接受完整的 MIR stage 输出，不再在边界内自动补挂 materialized MIR。
2. `codegen` 只接受 `effect facts` 之后的单一 authoritative handoff，不再重跑 MIR / effect-facts / effect-lowering。
3. 收口每一阶段发布的 fact table 职责，禁止 typed/fallback、HIR/program_facts、program/effect_facts/pass_view 这类可替代或并行查询面。

### 3. 把 `effect_lowered` 收实为正式 LIR stage

1. 明确目标 pipeline 为 `... -> effect facts -> LIR -> codegen`。
2. 让 `EffectLoweredStageOutput` 变成真正的 LIR handoff，而不是 `program + nested upstream bundle`。
3. 停止继续保留 HIR scaffold 作为长期过渡输入。

### 4. 清理 production path 中的 HIR-only lowering

1. top-level immutable init
2. object init
3. class ctor
4. closure lowering
5. ordinary callee suspend analysis

这些路径至少要做到：

1. 要么前移到 MIR / fact table。
2. 要么明确成为 codegen 输入的一部分，而不是在 backend 里临时回看 HIR AST/HIR body。

### 5. 最后收口 codegen 内部结构

1. 消灭多 `TypeStore` 并存与等价类型桥接。
2. 把逻辑 ABI 合同和 LLVM 物理 ABI 查询面拆开。
3. 把 frontend / pipeline 的公共 surface 从 LLVM-specific 命名和 feature gate 里解耦。
4. 把重复职责的全局 fact table 收敛成单一 authoritative 查询面。

### 6. 在边界稳定后统一做命名清理

1. 保留 `effect_*` 给真正 effect-specific 的分析、facts、effect-step callable 和 effect family packing。
2. 把已经通用化的 `effect_lowered` stage/module/output 改成与真实职责一致的名字。
3. 先做边界收口，再做大规模 rename；否则容易在错误边界上固化新名字。

## 结论

当前最核心的问题不是“文件太长”，而是 pipeline graph 不是单向图。

最大的三个结构性问题是：

1. 生产 HIR lowering 已经反向依赖 MIR materialization。
2. codegen stage 会重新驱动 MIR / effect-facts / effect-lowering，而不是只消费它们的输出。
3. production backend 仍在主线上混用 HIR、raw MIR、late-lowered program 和多套 fact/type universe。

其中一个贯穿全线的次级根因是：多张 fact table 的职责没有被切开，导致 fallback、复制、嵌套打包和下游混用都在持续发生。

只要这三件事不先收口，后面无论是继续拆 LLVM codegen、还是引入新的 backend，都会继续被“跨阶段回看”和“多 authoritative 输入”拖住。
