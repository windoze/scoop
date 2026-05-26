# Pipeline Cleanup 分析报告

## P2-T07 状态更新

当前代码已完成 P2 HIR barrier / `hir_facts` 收口：`HirStageOutput = { hir, hir_facts }` 是正式 HIR handoff，`scoopc_hir_facts` 是 declaration/entity/global/native/source-site facts 的 owner，`TypedHirEffectContracts`、`ProgramFacts`、source-site fallback 双轨和 HIR-carried MIR snapshot 已从生产代码移除。

## P3-T07 状态更新

当前代码已完成 P3 MIR boundary / MIR pass pipeline 收口：`MirStageOutput` 的 P4-ready handoff 发布 direct-style MIR、必选 canonical materialized snapshot 和 `MirFacts`，root inventories、snapshot binding、instance/callable family inventory、pass artifact metadata 与 MIR pass pipeline metadata 均由 `scoopc_mir_facts` 承载；ordinary dispatch 去虚化、summary-driven inlining、escape analysis、closure simplification 和必要 refresh 均由显式 MIR pass pipeline 调度。

## P4-T04 状态更新

当前代码已完成 P4 effect facts purity 收口：`EffectFactsStageOutput = { effect_facts }` 的窄输出成立，effect-facts stage 只读消费 P3 `MirStageOutput`，effect-owned type/context additions 写入 `MaterializedEffectFacts` / `scoopc_effect_facts` 数据产品，不再通过 `canonical_snapshot_mut()` 或 `&mut MaterializedMir` 修改 MIR snapshot。P5 的进入条件已经固定为显式分开传递 MIR handoff 与 effect facts handoff。

## P5-T05 状态更新

当前代码已完成 P5 LIR output / LIR facts / LIR opt family 收口：`LirStageOutput = { lir, lir_facts }` 是正式 P5 handoff，不嵌套 `MirStageOutput` 或 `EffectFactsStageOutput`；`scoopc_lir_facts` 发布 callable ABI、dynamic invoke、dispatch owner/slot、continuation/resume publication 与 LIR opt pipeline metadata；codegen-neutral ABI/query contract 已切到 `LIR + lir_facts`。P5 后保留的 global init/runtime residual 已由 P6 收口，LLVM stage handoff、reachability/body emission、physical ABI/layout 和 TypeStore bridge owner 已推进到 P7-T05 清场基线。

## P6-T04 状态更新

当前代码已完成 P6 global init/storage/entry order 收口：`scoopc_lir_facts` 发布 global root、object once、top-level eager init、per-cone init routine 与 final entry order contract；top-level `val` 和 annotated top-level `var` 由 LIR facts 驱动的 per-cone routine 在用户 `main` 前 eager 初始化；object singleton 继续独占 runtime once helper；`@Global` / `@ThreadLocal` storage policy 已从 frontend facts 贯通到 LLVM storage 与 runtime thread init hook。后续 P7 清理不得把 P6 global init/storage 语义当作未闭合项。

## P7-T05 状态更新

当前代码已完成 P7-T05 backend cleanup 清场基线：LLVM entry/global 查询、reachability、production body emission、stage handoff 和 physical ABI/layout 均已迁到 `LIR + lir_facts + LlvmStageBaseContext`。`LlvmCodegenStageOutput` / `StageEmitInput` 不再传播 `EffectLoweredStageOutput`、`LoweredHir` 或 `HirFacts` wrapper，`LirStageOutput` 不再携带 LLVM residual accessor，physical ABI/layout 只把 `LirFacts.physical_layout` / type context 映射成 LLVM-private layout。此前位于 LLVM stage 的 integer literal HIR body precheck 已迁到 typecheck/when-pattern 边界；LLVM stage 不再为该诊断遍历 HIR body。P8-T01 已完成最终 residual 搜索与文档冻结；`dependency-gate` 现在覆盖 LLVM stage handoff、emit handoff、reachability、旧 `comptime` keyword 和 LLVM `const_eval` helper 的 forbidden residual 搜索，防止重新引入 P5 wrapper、HIR/raw MIR scan、backend-local ordinary dispatch 去虚化或旧 comptime/const-eval surface。P10-T01 已用 portable `TypeStore` serialization 解决 `TypeId` cross-process stable wire format；P7/P8 的 deferred 记录现在只是历史背景，不再是当前 artifact 边界。

## P9-T09 状态更新

当前代码已完成 P9 stage crate split 清场：AST/HIR/MIR/effect-facts/LIR/codegen stage 实现分别由 `scoopc_ast`、`scoopc_hir`、`scoopc_mir`、`scoopc_effect_facts_stage`、`scoopc_lir` 与 `scoopc_codegen_llvm` 拥有；HIR/MIR/effect/LIR facts 由独立 fact crate 拥有；cone 操作位于 `scoopc_cone`，不得被 stage crate 反向依赖。`scoopc` umbrella 仅保留 facade re-export、`frontend.rs`、`pipeline/` 编排、session/CLI/driver helper 与测试审计模块；`crates/scoopc/src/` 下的 stage 名称引用只允许作为这些 wrapper 的兼容入口，不是新的 stage 实现 owner。

P9 之后，任何后续 stage 行为、stage handoff 或 stage fact 改动都必须在 owning crate 内完成，并通过基础 crate、前序 stage crate 与已发布 fact crate 显式传递；不得在 `scoopc` umbrella、下游 stage、cone 或 backend crate 中新增跨 crate fallback、facade escape 或临时 shim 来绕过 PLAN §1.2 的依赖方向。

因此，本报告中的 P1/P2/P3/P4/P5/P16/P17/P18/P19 是已解决或历史化的问题；保留它们是为了说明原始 cleanup 背景并验证阶段清场结果。P6、P7-T05、P8 与 P9 后，当前仍然成立、并进入 P10 的主结论是：

1. `scoopc_codegen_llvm` 已是独立 backend stage crate；`scoopc` 中的 LLVM 路径只剩 pipeline orchestrator 和 facade helper。
2. dependency gate 已覆盖 base、fact、stage、cone 与 umbrella/backend residual 检查；P10 不应重新引入 stage 间反向依赖或 umbrella fallback。
3. `TypeId` cross-process stable wire format 已由 P10-T01 落地为 portable `TypeStore` serialization；当前 P10 清场只需防止文档或 facts 再宣称该能力仍 deferred。

## P10-T07 状态更新

当前代码已完成 P10 per-cone build artifact 主线收口：`scoopc build-single-cone` 负责单 cone frontend/codegen/native build 与 `type_store.bin` / facts / LIR / object artifact 写盘，`scoopc link-cone` 只 forward 到 `scoopld`，`scoop` facade 只 materialize 输入、调度 `LocalDependency` + `Consumer` artifact 子进程、读取 artifact manifest 并派发 link 子进程。`scoop` crate 的内部依赖白名单只允许 `scoop_project_model`，dependency gate 同时扫描 `crates/scoop/src`，禁止 `scoopc::*` / stage crate / `scoopld` in-process residual 回流。

P10 清场后的剩余后续入口只有跨 cone generic body wire format：如果未来需要复用上游 generic HIR/MIR template，应作为 P11 单独设计并验证，不能在 P10 内通过重新读取上游源、保留 whole-DAG frontend fallback 或扩大 cone artifact schema 来绕行。

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
4. `scoopc_ids`：`SiteId`、`BodyVersionKey`、stable hash/key primitives，以及后续可提升为基础层的 stage-independent callable/body identity；当前 MIR materialization 的 `TemplateKey` / `InstanceKey` 仍是 stage-owned internal key，不能被 fact crate 当作基础 ID 直接依赖。
5. `scoop_project_model`：source-cone / project / compilation-unit membership 等与具体阶段无关的工程模型。

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

P1/P2 后仍需补齐什么：

1. `AstCompilationUnitOutput` 已作为 cone-level AST handoff 存在，`AstStageOutput` 只保留为单文件 worker / dump helper。
2. production frontend 仍让 resolver/typecheck/HIR 过渡路径消费 build closure；P2 已固定 HIR barrier 与 `hir_facts` handoff，彻底物理拆分 cone-level orchestration 仍属于后续 pipeline 收口。

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

P2 收口结果：

1. `HirStageOutput` 已公开为 HIR 本体 + `HirFacts`。
2. declaration/entity/global/native/source-site typed contracts 已迁入 `scoopc_hir_facts`，并由独立 verifier/dump 覆盖。
3. `LoweredHir` 不再携带 MIR 产物，也不再与 `TypedHirEffectContracts` / `ProgramFacts` 并行发布同一类事实。
4. 仍保留的 `LoweredHir` side table 是 HIR body inventory、type context、lowering helper 或 LLVM compatibility scaffold；跨阶段源码语义查询必须以 `HirFacts` 为 owner。

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

P3 收口结果：

1. `MirStageOutput` 已不再泄漏 HIR typed contracts，也不再以 optional snapshot 或并列 root-index map 作为 handoff。
2. `scoopc_mir_facts` 已发布 root inventories、materialized snapshot binding、instance/callable family inventory、pass artifact metadata 和 MIR pass pipeline metadata。
3. downstream 仍可经由 canonical `MaterializedMirPassView` 消费 pass 后 callable body / summary / escape facts；这现在是 MIR-owned pass query surface，而不是 HIR fallback。

P4/P5 收口结果与剩余边界：

1. `EffectFactsStageOutput` 已收口为 `{ effect_facts }`，不再嵌套或转发 `MirStageOutput` / pass view / MIR type store。
2. `LirStageOutput` 已收口为正式 `{ lir, lir_facts }` handoff；legacy `EffectLoweredStageOutput` 只是迁移别名，不再公开上游 pass view/effect facts/MIR facts accessor。
3. P5-owned codegen-neutral ABI/query contract 已迁入 `scoopc_lir_facts`；若 TODO-6/P7 仍发现 backend 需要新的 backend-neutral MIR-derived global facts，应补到 LIR/LIR facts 或明确回到 MIR-owned facts，而不能重新引入 P3/P4 已清理的重复 owner。

### effect facts stage

应发布的 IR/output：

1. 绑定到 canonical MIR snapshot 的 effect facts output。

应发布的专属 facts：

1. `snapshot_binding`：明确这批 facts 绑定到哪份 MIR snapshot。
2. `callable_facts`：每个 materialized callable/instance 的 outward-effect / step schema /
   impl plan / reentry 等 effect-owned 合同。
3. `bodies`：每个 body/site 的 effect/control 事实。
4. `step_schemas` 与 `continuation_schemas`。

P4 收口结果：

1. `EffectFactsStageOutput` 只保存 `MaterializedEffectFacts`，其 stable dump 只描述 effect facts、effect-owned type context 与 snapshot binding。
2. `MaterializedEffectFactsBuilder` 只读借用 `MaterializedMir`，并通过显式 `EffectOwnedTypeContext` 记录 runtime-error effect、tuple carrier、step/continuation schema 等 effect-owned additions。
3. 若下一阶段（LIR）还需要某些 MIR-derived 但非 effect-owned 的 facts，这些 facts 的 owner 应在 P5 的 LIR facts 中固定，不能通过 P4 output 重新回看上游整包。

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

P5 收口结果：

1. `LirStageOutput` 发布 `LateLoweredProgram` 与 `scoopc_lir_facts::LirFacts`，不嵌套 `MirStageOutput` 或 `EffectFactsStageOutput`。
2. `scoopc_lir_facts` 发布 plain callable ABI/source/call-site、本地 effect/control contract、effect-step state/frame/boundary/resume query、dynamic invoke、dispatch owner/slot、continuation object、surface resume dispatch、resume packing 与 LIR opt pipeline metadata；P6 又在同一 fact 产品中补齐 global init/storage/final-entry contract。
3. Program ABI materialization 的 codegen-neutral 输入已切到 `LateLoweredProgram + LirFacts + TypeStore`；P7-T05 后 physical ABI/layout 查询只读 `LirFacts.physical_layout` / type context / callable symbols，再映射成 LLVM-private `ProgramAbiQuery` 结果，并由 dependency gate 防止 emit/reachability/handoff 回退到上游 wrapper 或 HIR/MIR scan。
4. `effect_lowered::opt` 已固定为 LIR-owned 窄优化 family，显式记录 pass order / revision / verifier metadata，不读取 HIR、MIR pass view 或 effect solver 输入。

TODO-6/P7 收口结果：

1. LLVM entry/global、reachability 和 production body emission 已从 raw HIR/MIR fallback 迁到 LIR/LIR facts 与 base context 查询面。
2. `LlvmCodegenStageOutput` 与 `StageEmitInput` 已摊平成显式 `LIR + lir_facts + LlvmStageBaseContext`；不再携带 `EffectLoweredStageOutput` / `LoweredHir` / `HirFacts` wrapper。
3. `LirStageOutput` 只发布 `LateLoweredProgram + LirFacts`，不再暴露 LLVM-only `types()` 或 `llvm_residual_pass_view()` accessor。
4. legacy `effect_lowered` 命名仍可在边界稳定后继续清理，但它已不代表嵌套上游 output 的合法长期形状。

## 当前有效结构

当前生产路径已经不再让 HIR lowering 反向携带 MIR snapshot，P4/P5 stage output 也不再嵌套上游整包；P7-T05 后 LLVM stage 的 emit handoff 也已收窄为 `LIR + lir_facts + LlvmStageBaseContext`，并补上 source boundary gate。现在更接近：

```text
frontend/typecheck
  -> lower_hir_for_codegen_with_request_root_mode(...)
    -> CodegenLoweringOutput { lowered_hir, materialized_mir }
  -> llvm_codegen_stage::run(...)
    -> 从 CodegenLoweringOutput 显式构造 HIR/MIR/effect/LIR handoff
    -> 运行 LIR stage，产出 LirStageOutput { lir, lir_facts }
    -> 构造 LlvmStageBaseContext，记录同进程 TypeStore owner 与必要 backend base tables
  -> llvm::emit(...)
    -> 消费 LIR + LIR facts + LlvmStageBaseContext / ABI visibility LIR variant
```

这意味着当前实现里同时存在：

1. codegen stage 仍是 `scoopc` 内的 backend orchestrator；P9/P10 会继续拆独立 stage/codegen crate 与 per-cone artifact。
2. `MirStageOutput`、`EffectFactsStageOutput`、`LirStageOutput` 和 LLVM emit handoff 已分别收口为各自阶段/后端输入形状，不再通过 public output wrapper 嵌套上游整包。
3. `LlvmStageBaseContext` 仍承接同进程 `TypeStore` owner 和少量 backend base tables；P8 继续做最终 residual 搜索，当前关键入口已由 dependency gate 防止退化成新的 HIR/raw MIR/effect facts fallback。
4. `TypeId` cross-process stable wire format 已在 P10-T01 通过 portable `TypeStore` serialization 落地；P7-T05 的同进程 LLVM handoff 只是不需要提前解决该持久化问题。

## 问题清单

### P1. 生产 HIR lowering 反向依赖 MIR materialization（P2 已解决）

位置：

- `crates/scoopc/src/frontend.rs:521-580`
- `crates/scoopc/src/hir/lower/main/compilation_unit.rs:387-437`

历史现状：

- `lower_hir_for_codegen_with_request_root_mode(...)` 曾在 HIR lowering 入口内部先调用 MIR materialization，再把 snapshot 反灌到 `LoweredHir`。

P2 结果：

- production codegen handoff 已拆为 `CodegenLoweringOutput { lowered_hir, materialized_mir }`。
- canonical materialized MIR 作为单独 MIR-owned handoff 返回，不再挂回 `LoweredHir`。

为什么是问题：

- 这直接打破了 `HIR -> MIR` 的单向顺序。
- request-root 选择、opt-level、instance collection 这些本应属于 MIR 或更后面的 concern，已经泄漏到 HIR 生产路径。

---

### P2. `LoweredHir` 挂住了 canonical MIR snapshot / pass view（P2 已解决）

位置：

- `crates/scoopc/src/hir/lower/types.rs:329-337`
- `crates/scoopc/src/hir/lower/types.rs:421-467`
- `crates/scoopc/src/hir/lower/types.rs:470-499`

历史现状：

- `LoweredHir` 曾持有 `materialized_mir`，并公开 `materialized_mir()` / `materialized_pass_view()` 等接口。

P2 结果：

- 这些字段、accessor 与 `clone_hir_compat_scaffold_without_materialized_mir()` 均已删除。
- `materialized_mir` / `materialized_pass_view` 在当前代码中的合法命中属于 MIR/effect/LIR/codegen 之后的 handoff，不属于 HIR output API。

为什么是问题：

- HIR 输出不应继续携带 MIR 阶段产物。
- 这会让后续任何消费者都把 HIR 当成“多阶段 bundle”，而不是清晰的阶段边界。

---

### P3. `MirStageOutput` 同时暴露 HIR 合同和两套 MIR 视图（已由 P2/P3 清理）

历史位置：

- `crates/scoopc/src/pipeline/mir_stage.rs:13-27`
- `crates/scoopc/src/pipeline/mir_stage.rs:29-37`
- `crates/scoopc/src/pipeline/mir_stage.rs:68-84`
- `crates/scoopc/src/pipeline/mir_stage.rs:215-240`

P3 结果：

- `MirStageOutput` 现在以 direct-style MIR + mandatory canonical materialized snapshot + `MirFacts` 形成 P4-ready handoff。
- HIR source-site contracts 已在 P2 迁入 `HirFacts` 并由 MIR lowering 消费，`MirStageOutput` 不再继续向 P4/P5/P6 暴露旧 HIR typed contract payload。
- root inventories、snapshot binding、pass artifact metadata 和 MIR pass pipeline metadata 由 `scoopc_mir_facts` 发布；旧的并列 root-index map 与 optional snapshot gap 已移除。

剩余边界：

- P5 已收口 LIR output / LIR facts owner；TODO-6/P7 仍需继续清理 LLVM HIR compatibility scaffold、residual pass view 和 backend physical layout。
- LLVM reachability/codegen 里的去虚化 residual 属于 P7 backend cleanup；P3 不再依赖它们作为普通语义优化 fallback。

---

### P4. effect-facts stage purity 已完成

位置：

- `crates/scoopc/src/pipeline/effect_facts_stage.rs`
- `crates/scoopc/src/effect_facts/builder.rs`
- `crates/scoopc_effect_facts/`

收口结果：

- P3 已保证传入 P4 的 `MirStageOutput` 带有必选 canonical materialized snapshot，不再由 P4 自动补跑 MIR materialization。
- `EffectFactsStageOutput` 只保存 `MaterializedEffectFacts`，不暴露 `mir_stage_output()`、`materialized_mir()`、`materialized_pass_view()` 或 `mir_facts()` 等上游查询面。
- P4 stage 内部不再存在 `canonical_snapshot_mut()` 调用；`MaterializedEffectFactsBuilder` 接收只读 `&MaterializedMir` 与显式 `EffectOwnedTypeContext`。
- `scoopc_effect_facts` 是独立 fact crate，只依赖基础 crate，不依赖 `scoopc` facade、MIR/LIR stage、backend crate 或其它 fact crate。

为什么这满足 P4：

- P4 不再把上游 `MirStageOutput` 作为自己的输出形状嵌套给 P5/P6/P7。
- P4 只读消费完整 authoritative MIR handoff，并发布 effect-owned facts/context，而不是修改 MIR stage 本体。
- P5 需要 MIR pass view 或 MIR facts 时，必须通过显式 `EffectLoweringStageInput { mir_stage_output, effect_facts_stage_output }` 获取，不能通过 P4 output 回看。

---

### P5. `effect_lowering_stage` 已收口为 formal LIR stage（P5 已解决）

位置：

- `crates/scoopc/src/pipeline/mod.rs:133-149`
- `crates/scoopc/src/pipeline/effect_lowering_stage.rs:13-33`
- `crates/scoopc/src/effect_lowered/mod.rs:1-13`

收口结果：

- 当前生产主线里 `effect_lowering_stage` 是一个显式公开的独立阶段。
- 它会从显式 `EffectLoweringStageInput { mir_stage_output, effect_facts_stage_output }` 产出 `LirStageOutput { lir, lir_facts }`。
- `LirStageOutput` 不保存 `MirStageOutput` 或 `EffectFactsStageOutput` wrapper；legacy `EffectLoweredStageOutput` 只是迁移别名。
- P5-owned codegen-neutral ABI/query contract 已发布到 `scoopc_lir_facts`，LIR opt family 也有显式 pass metadata 与 verifier。

P7-T05 收口结果：

- `LirStageOutput` 不再保留 `types()` 或 crate-private `llvm_residual_pass_view()`；LLVM 所需 base context 由 `LlvmStageBaseContext` 显式承载。
- `scoopc_lir_facts` 已补齐 global init/storage/final-entry、physical layout、callable symbol 和 type context bridge facts；这些属于 LIR-owned backend-neutral 查询面，不是 P5 output wrapper 的隐藏 residual。
- LLVM stage 的 integer literal 诊断不再通过 HIR body precheck 兜底，范围检查已上移到 typecheck/when-pattern 边界。
- `dependency-gate` 覆盖 LLVM stage handoff、emit handoff 与 reachability 的 forbidden residual 搜索，阻止重新引入 P5 wrapper、HIR/raw MIR scan 或 backend-local ordinary dispatch 去虚化。

后续方向：

1. P8-T01 已完成最终 residual 搜索和文档冻结；P8-T02 继续做 release-readiness 验证。
2. P10 per-cone artifact serialization 已落实 `TypeId` cross-process stable wire format；后续只需保持 `type_store.bin` 与 facts/LIR schema 同步演进。

---

### P6. `llvm_codegen_stage` 会从 `LoweredHir` 重新驱动上游阶段

位置：

- `crates/scoopc/src/pipeline/llvm_codegen_stage.rs:139-167`
- `crates/scoopc/src/pipeline/llvm_codegen_stage.rs:208-235`

现状：

- `run_effect_lowered_stage_from_lowered_hir(...)` 会：
  `LoweredHir -> HirStageOutput::new(...) -> mir_stage::run(...) -> build_effect_facts_stage_output(...) -> build_effect_lowered_stage_output(...)`。
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
- `ProgramFacts::from_lowered(lowered)` 已删除；`emit.rs` / LLVM codegen 现在显式接收 `HirFacts`。
- 但 `hir_compat_scaffold` 仍为若干 LLVM layout、top-level init 和 HIR-only lowering path 提供过渡输入。

为什么是问题：

- 如果 codegen 真正只消费 `LIR + LIR facts + base context`，这里就不应再需要 HIR scaffold。
- P2 已清理 `ProgramFacts` 重建问题；剩余问题归属 P7 backend cleanup，而不是 HIR facts owner 重叠。

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

- `MainCodegen` 会从当前 function env 里的 `hir_ty`、`HirFacts` / `ExprFactResolver`、`materialized_pass_view()` 现场组装 `EffectAnalysisCtx`。

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

### P16. HIR stage 同时发布多套职责重叠的全局合同/side table（P2 已解决）

位置：

- `crates/scoopc/src/hir/lower/types.rs:360-405`
- `crates/scoopc/src/pipeline/hir_stage.rs:879-978`
- `crates/scoopc/src/mir/lower/mir_lowering_facts.rs:39-71`

历史现状：

- `LoweredHir` side table、HIR stage typed contracts 和后续 fact adapter 曾同时描述调用点、perform/resume/handle、assignment place、top-level init/extern root 等职责。

P2 结果：

- `scoopc_hir_facts::HirFacts.source_sites` 现在是 source-site typed contracts 的唯一发布面。
- HIR stage 内部 collector 只负责构建 `HirFacts`；后续 stage 不直接消费内部 collector 或 `LoweredHir` source-site side table。

为什么是问题：

- 这已经不是“同一阶段输出里有不同表”这么简单，而是多张表在描述同一类职责：
  调用点合同、perform/resume/handle 语义、assignment place、top-level init/extern root 等。
- 一旦下游需要同时读两张表，或者需要知道“优先信哪张表”，就说明阶段职责没有收口干净。

结论：

- 对下游发布的 authoritative 合同已经切到 `HirFacts`。
- `LoweredHir` 仍保留的表必须被视为 HIR lowering/helper 或 LLVM compatibility scaffold，不是跨阶段 fact owner。

---

### P17. MIR lowering 内部已经把两套合同当成“可替代输入”（P2 已解决）

位置：

- `crates/scoopc/src/mir/lower/mod.rs:56-86`
- `crates/scoopc/src/mir/lower/mir_lowering_facts.rs:8-37`
- `crates/scoopc/src/mir/lower/mir_lowering_facts.rs:74-128`
- `crates/scoopc/src/mir/lower/mir_lowering_facts.rs:272-283`
- `crates/scoopc/src/mir/lower/fn_lowering_call.rs:842-965`
- `crates/scoopc/src/mir/lower/fn_lowering_call.rs:978-1008`
- `crates/scoopc/src/mir/lower/fn_lowering_effect.rs:250-260`
- `crates/scoopc/src/mir/lower/fn_lowering_basic.rs:187-213`

历史现状：

- `MirLoweringFacts` 曾显式存在 typed/fallback 双轨，并允许 MIR lowering 在缺少 typed contract 时回扫 HIR side table。

P2 结果：

- `MirLoweringFacts::from_hir_facts(...)` 是 source-site input 的单一路径。
- 旧 `FallbackSideTables`、typed bridge、source-site migration adapter 和缺 contract fallback lowering 已删除。

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

### P18. `ProgramFacts` 与 `LoweredHir` side table 职责重叠，LLVM codegen 同时持有并混用两者（P2 已解决）

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

历史现状：

- `ProgramFacts` 曾从 `LoweredHir` 复制 top-level value、object property、struct/class field、class super、ctor/resume 等事实，并与 LLVM 直接读取 HIR side table 并存。

P2 结果：

- `ProgramFacts` 已删除。
- `ExprFactResolver` / `HirFactResolver`、effect analysis 和 LLVM codegen 的 declaration/entity 查询改为消费 `HirFacts`。
- LLVM 仍持有 `LoweredHir` compatibility scaffold，但该 scaffold 不再与 `ProgramFacts` 形成重复 authoritative 查询面。

为什么曾是问题：

- 这里已经形成两套都能回答“top-level 值类型/字段类型/ctor target/resume site”问题的事实层。
- 这会让下游自然产生“查哪张都行”的混用习惯，而不是依赖单一 authoritative fact source。

结论：

- P2 选择删除 `ProgramFacts`，以 `HirFacts` 作为共享事实层。
- 若后续必须继续保留 `LoweredHir` compatibility scaffold，不能再复制出第二套同职责 facts。

---

### P19. LIR stage output 曾继续整包暴露上游事实（P5 已解决）

历史位置：

- `crates/scoopc/src/pipeline/effect_lowering_stage.rs:30-79`
- `crates/scoopc/src/llvm/codegen/effect_lowered/layout/mod.rs:74-85`

历史现状：

- `EffectLoweredStageOutput` 不只发布 `program()`。
- 它还继续暴露：
  `effect_facts_stage_output()`、`materialized_mir()`、`materialized_pass_view()`、`types()`、`effect_facts()`。
- 这使得后续 codegen 很自然地同时消费：
  `LateLoweredProgram + MaterializedEffectFacts + MaterializedMirPassView + TypeStore`。这里的 nested bundle 已经是 P5 输出问题，而不是 P4 output 问题。

P5 结果：

- `LirStageOutput` 现在只发布 `lir()` / `program()`、`lir_facts()`、`types()` 和稳定 dump/parts API。
- 旧的 `materialized_pass_view()`、`materialized_mir()`、`mir_facts()`、`effect_facts()`、`effect_facts_stage_output()` public accessor 已删除；当前仅保留 crate-private `llvm_residual_pass_view()`。
- `ProgramAbiMaterializer` 的 codegen-neutral contract 输入已改为 `LateLoweredProgram + LirFacts + TypeStore`。

剩余边界：

- LLVM body emission、reachability、entry/global inventory physicalization、physical ABI layout 和多 `TypeStore` 桥接仍会使用 HIR scaffold 或 residual MIR pass view。
- 这些 residual 必须在 TODO-6/P7 中继续清理；P6 已发布 global init/storage/final-entry LIR facts，不能把剩余 HIR scaffold 当成新的语义 owner，也不能重新放回公开 stage output API。

## 非问题 / 允许的耦合

下面这些不应和上面的边界问题混为一谈：

1. MIR lowering 消费 HIR 导出的 fact table，这是正常的 `HIR -> MIR` 依赖。
2. effect-facts stage 消费 canonical MIR snapshot，这是正常的 `MIR -> effect facts` 依赖。
3. codegen 内部可以继续有自己的子阶段；问题不在于“有子阶段”，而在于当前 codegen 需要跨阶段回看多套上游表示。

## 建议的清理顺序

### 1. 先修正大方向上的反向依赖

1. 让生产 HIR lowering 不再依赖 MIR instance/materialization。（P2 已完成）
2. 从 `LoweredHir` 移除 `materialized_mir` / `materialized_pass_view`。（P2 已完成）
3. 让 `MirStageOutput` 只暴露 MIR 自己的 authoritative handoff，不再继续携带 HIR 合同或重复 root/pass owner。（P2/P3 已完成）

### 2. 让 stage 输入变严格

1. `effect_facts` 只接受完整的 MIR stage 输出，不再在边界内自动补挂 materialized MIR。
2. `codegen` 只接受 `LIR + lir_facts + base context` handoff，不再重跑 MIR / effect-facts / LIR。
3. 收口每一阶段发布的 fact table 职责；HIR/source-site 与 `ProgramFacts` 重叠已由 P2 清理，MIR/pass-view owner 已由 P3 收口，effect facts purity 已由 P4 收口，LIR facts/query owner 已由 P5 收口，TODO-6/P7 继续处理 backend residual。

### 3. 把 `effect_lowered` 收实为正式 LIR stage

1. 明确目标 pipeline 为 `... -> effect facts -> LIR -> codegen`。（P5 已完成）
2. 让 `EffectLoweredStageOutput` 变成真正的 LIR handoff，而不是 `program + nested upstream bundle`。（P5 已完成，当前公开 API 为 `LirStageOutput`）
3. 停止继续保留 HIR scaffold 作为长期过渡输入。（TODO-6/P7）

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

当前最核心的问题不是“文件太长”，而是必须继续防止已收口的 stage handoff 与 backend physicalization 回退成跨阶段回看。

P10-T07 后的三个结构性结论是：

1. `LlvmCodegenStageOutput` / `StageEmitInput` 已收口为 `LIR + lir_facts + LlvmStageBaseContext`，不再传播 P5 wrapper 或 HIR scaffold wrapper。
2. production backend 的 entry/global、reachability、body emission 与 physical ABI/layout 已迁到 LIR/LIR facts/base context；dependency gate 已覆盖 LLVM handoff/emit/reachability、旧 `comptime` keyword 和 LLVM `const_eval` helper 的关键防回退检查，P8 只剩 release-readiness 验证。
3. `TypeId` cross-process stable wire format 已由 P10-T01 落地为 portable `TypeStore` serialization；per-cone artifact 必须同步携带 `type_store.bin`、facts/LIR schema version 与 `LirFacts.type_context.stable_wire_format = implemented`。

其中一个贯穿全线的次级根因是：backend physical layout、reachability、body emission 和 type bridging 必须始终建立在 `LIR + lir_facts + base context` 上。P2 已经把 HIR/source-site facts 收口到 `HirFacts`，P3 已经把 MIR root/snapshot/pass facts 收口到 `MirFacts` 和 MIR pass query surface，P4 已经把 effect facts 收口为只读窄输出，P5 已经把 LIR facts/query owner 收口，P6 已把 global init/storage/final-entry owner 收口到 LIR facts，P7-T05 已把 LLVM handoff 与 physical ABI/layout 收口到同一边界并加上 gate；后续不得重新引入 HIR fallback、MIR fallback、effect facts fallback 或重复 owner。

后续无论是继续拆 LLVM codegen、还是引入新的 backend，都必须复用这条 backend-neutral 输入边界，而不是复制旧的跨阶段回看路径。
