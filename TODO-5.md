# TODO-5：effect facts purity + LIR output

> 生成时间：2026-05-21
> 计划基线：[`PLAN.md`](./PLAN.md) §4/P4-P5
> 设计基线：[`PIPELINE_REFACTOR.md`](./PIPELINE_REFACTOR.md)
> 审计基线：[`PIPELINE-CLEANUP.md`](./PIPELINE-CLEANUP.md)
> 索引：[`TODO.md`](./TODO.md)
> 顺序约束：严格按本文件任务顺序推进；每个实现任务后必须执行紧随其后的 review 任务。
> 本包目标：先让 effect facts stage 成为只读分析输出，再把当前 `effect_lowered` 收实为正式 `LIR + lir_facts` handoff，并只保留 effect/control 相关窄 LIR optimization family。

## 全局约束

- 本包覆盖 `PLAN.md` 的 P4 和 P5；不得提前处理 P6 global init model、P7 LLVM backend cleanup 或 P8 final verification。P5 可以切换 codegen-neutral LIR query surface，但不得把 TODO-6 中的 HIR scaffold 清理、LLVM backend 输入最终收口、entry/global init 语义闭合塞进本包。
- P4 完成后，effect facts stage 必须只读消费 P3 `MirStageOutput` / canonical MIR pass view。任何 effect-owned type/context/schema 追加都必须作为 effect facts 自己的产物发布，不能写回 `MaterializedMir` 或 `MirStageOutput`。
- P4 完成后，`EffectFactsStageOutput = { effect_facts }` 或等价窄输出必须成立；它不得嵌套 `MirStageOutput`，不得继续暴露 `mir_stage_output()` / `materialized_mir()` / `materialized_pass_view()` 这类上游整包查询面。
- P5 完成后，`EffectLoweredStageOutput` 必须退化或替换为正式 `LirStageOutput = { lir, lir_facts }`。`LateLoweredProgram` 可以继续作为当前 LIR 本体，但输出命名、字段和查询面必须表达 LIR 职责，而不是 effect-facts wrapper。
- `scoopc_effect_facts` 与 `scoopc_lir_facts` 若被新建为 fact crate，必须遵守 fact crate DAG：只能依赖基础 crate，不得依赖 `scoopc` facade、stage/backend crate 或其它 fact crate。若某些 stage-owned runtime key 暂不能放入 fact crate，必须先提升为基础 stable identity，不能泄漏 MIR/LIR 内部 key。
- LIR optimization 只能覆盖 effect/control 相关窄 pass：local state-machine elimination、简单 higher-order wrapper 定向 inline/devirt、wrapper state folding、dead state / dead slot cleanup。不得把 LIR 扩展成普通调用图、全程序 devirtualization 或通用 inlining owner。
- 每个任务完成后，在该任务的“完成记录”下写明改动范围、核心决策、验证命令和残余风险。

## 触碰面基线

本节是 `TODO-5-INIT` 的仓库搜索记录，后续 P4/P5 任务应优先从这些位置开始，不再重复做开放式仓库搜索。

### effect facts stage 当前 nested output、MIR 修改点与重算点

`EffectFactsStageOutput` 定义在 `crates/scoopc/src/pipeline/effect_facts_stage.rs:23-26`，当前字段是 `mir_stage_output: MirStageOutput` 与 `effect_facts: MaterializedEffectFacts`。

| 位置 / 查询面 | 当前职责 | 当前问题 | P4 处理方向 |
| --- | --- | --- | --- |
| `EffectFactsStageOutput::mir_stage_output()` | 直接暴露整份 P3 `MirStageOutput` | P4 output 嵌套上游 stage output，违反 stage output wrapper 规则 | 删除或降为 stage-private 输入；P5/P6 不得通过 P4 output 回看整包 MIR |
| `mir_facts()` / `file()` / `materialized_mir()` / `materialized_pass_view()` | 从嵌套 `MirStageOutput` 转发 MIR facts、MIR file、canonical snapshot、pass view | 把 P3/P4/P5 多张查询面打包成可替代输入；P5 输出也继续继承这些转发方法 | P5 如需 MIR IR / MIR facts，应显式从自己的 LIR stage input 或 `lir_facts` 获取，不经 P4 wrapper 转发 |
| `types()` | 返回 `self.materialized_mir().types` | effect facts builder 会向 snapshot-owned `TypeStore` 追加 effect/control helper types，导致 P4 修改 MIR 本体 | 改为 effect-owned type context / published type extension；不写回 MIR snapshot |
| `stable_dump()` | 使用 `effect_facts.stable_dump(self.types(), self.materialized_pass_view())` | dump 也依赖 nested MIR/pass view 和 mutated snapshot types | 改为 dump effect-owned facts + explicit snapshot binding / type display context |

`run_with_compilation_sources(...)` 位于 `crates/scoopc/src/pipeline/effect_facts_stage.rs:88-140`。当前输入参数是 `mut mir_stage_output: MirStageOutput`，并在两处调用 `mir_stage_output.canonical_snapshot_mut()`：第一次构造 provisional facts，第二次在需要 compiler-generated continuation runtime error 上界时重建 facts。

`MaterializedEffectFactsBuilder` 位于 `crates/scoopc/src/effect_facts/builder.rs:2016-2146`。当前 builder 的 `from_materialized_snapshot(...)` / `from_materialized_snapshot_in_compilation_unit(...)` 接受 `&mut MaterializedMir`，并在 `build(...)` 中：

1. 从 `self.materialized.pass_view()` 构造 `MirSnapshotBinding`。
2. 调用 `EffectFactsTypeContext::build(...)` 重新基于 session/source/compilation sources 构造 top-level index、parse files、`TypeEnv`、class vtables、interfaces/itables 和 direct subclasses（`builder.rs:2149-2220`）。
3. 通过 `find_or_intern_raise_runtime_error_effect(&mut self.materialized.types)` 向 MIR snapshot type context 追加 compiler-generated runtime error effect type。
4. 通过 `schema_pool.intern_callable_step_schema(...)`、`canonical_tuple_carrier_ty(...)`、`BodyFactsBuilder::build(types)` 等路径继续使用 `&mut self.materialized.types` 构造 step schema、invoke tuple、continuation schema 和 body/site facts。
5. 从 materialized MIR 现场收集 callable owner map、raw fun map、top-level value surface contracts 与 property accessor surface contracts（`builder.rs:2073-2081`）。

P4 必须把上述行为拆清：effect facts 可以读取 canonical MIR pass view 和 MIR facts；可以发布 effect-owned schema/type/context；但不能把这些追加结果写回 `MaterializedMir`，也不能把 MIR-derived owner/root/query 信息重新发布成第二个 authoritative owner。

### effect_lowered 当前输出结构、构造点与 LIR fact/query 候选项

`EffectLoweredStageOutput` 定义在 `crates/scoopc/src/pipeline/effect_lowering_stage.rs:31-35`，当前字段是 `effect_facts_stage_output: EffectFactsStageOutput` 与 `program: LateLoweredProgram`。

| 位置 / 查询面 | 当前职责 | 当前问题 | P5 处理方向 |
| --- | --- | --- | --- |
| `effect_facts_stage_output()` | 继续暴露整份 P4 output | P5 output 嵌套 P4 output，间接嵌套 P3 `MirStageOutput` | 删除；LIR stage output 只发布 LIR 与 `lir_facts` |
| `mir_facts()` / `snapshot_binding()` / `materialized_mir()` / `materialized_pass_view()` / `types()` / `effect_facts()` | 从 nested P4/P3 handoff 转发 MIR facts、snapshot、pass view、type store 和 effect facts | codegen 可以同时读 LIR、effect facts、MIR pass view、MIR snapshot 和 type store | 将 codegen-neutral contract 迁入 LIR/LIR facts；需要保留的 base context 作为显式 LIR output context，而不是上游 wrapper |
| `program()` | 当前 LIR 本体，类型为 `LateLoweredProgram` | 本体方向正确，但命名和 output 形状仍暗示 effect 子阶段 | 保留或 rename 为 `lir()` / `program()`，但输出名和 facts 应表达正式 LIR stage |
| `run(...)` / `run_with_opt_options(...)` | 调用 `LateLoweredProgramBuilder::from_canonical_inputs(pass_view, effect_facts, types, mir_facts).build()` 后运行 `optimize_program*` | 构造输入仍从 nested P4 output 获取；优化 pass 没有正式 pipeline/facts refresh 边界 | 改为显式 LIR stage input，并把 opt family 变成命名 pipeline |

`LateLoweredProgram` 定义在 `crates/scoopc/src/effect_lowered/ir.rs:23-31`。当前已经承载的 LIR 本体字段包括：

- `step_types`：`StepSchema` 对应的 `Step_F` shell。
- `resume_packings`：effect-family resume packing helper。
- `continuation_objects`：continuation object surface/internal resume publication。
- `surface_resume_dispatch_inventory`：`ContinuationSchemaId` 到 authoritative dispatch source inventory 的 published entry。
- `callables`：plain callable 与 effect-step callable 的 LIR callable inventory。
- `stable_instance_keys`、`dump_type_texts`、`dump_body_labels`：stable identity / dump metadata 过渡信息。

`LateLoweredProgramBuilder` 位于 `crates/scoopc/src/effect_lowered/builder.rs:33-60`，当前输入是 canonical MIR pass view、`MaterializedEffectFacts`、`TypeStore` 与 `MirFacts`。主要构造点包括：

- `materialize_step_and_resume_interfaces(effect_facts)` 生成 step shell 与 resume packing。
- `plan_continuation_route_owners(...)`、`build_cross_callable_continuation_provenance(...)` 基于 pass view + effect facts 规划 continuation ownership。
- 遍历 `pass_view.instances()` 构造每个 callable 的 LIR representation。
- plain callable 分支通过 `build_plain_callable_abi(...)` 发布普通函数签名、source slice、plain call-site contract 和本地 effect/control contract。
- effect-step 分支发布 dynamic invoke entry、state graph、frame schema、boundary map、resume state map、continuation object 和 source statement classifications。
- `nominal_direct_supertypes_from_mir_facts(...)` 从 `MirFacts` 复制 direct supertype index，供 LIR materialize contract 使用。

当前应优先迁入 `lir_facts` 或 LIR query layer 的候选项：

1. callable inventory、body version identity、stable callable/instance identity。
2. plain callable ordinary ABI、source slices、plain call-site facts、本地 effect/control contract。
3. effect-step callable ABI、step schema binding、state graph、frame schema、boundary map、resume state map。
4. dynamic invoke contract：carrier/source kind、arg carrier type、entry/complete state、target layout source。
5. dispatch owner/slot selection：virtual/interface owner、slot、itable/vtable selection所需 backend-neutral contract。
6. continuation/resume publication：continuation object、surface resume inventory、resume packing completeness、wrapper projection。
7. LIR-owned type display/context、dump body labels，以及 codegen-neutral ABI query 所需但不属于 LLVM physical layout 的 contract。

### 当前 codegen 读取点

P5 不负责最终移除所有 LLVM/HIR scaffold，但必须知道哪些读取点证明 LIR output 还不完整。

| 位置 | 当前读取内容 | 归属判断 |
| --- | --- | --- |
| `crates/scoopc/src/pipeline/llvm_codegen_stage.rs:126-160` | LLVM stage 从 `LoweredHir + MaterializedMir` 重新构造 `HirStageOutput -> MirStageOutput -> EffectFactsStageOutput -> EffectLoweredStageOutput` | TODO-6/P7 负责最终让 codegen 不再重跑上游；P5 不得新增依赖此路径的 LIR contract |
| `llvm_codegen_stage.rs:80-90` | `LlvmCodegenStageOutput` 保留 `hir_compat_scaffold`、`hir_facts`、primary/ABI visibility `EffectLoweredStageOutput` | P7 backend cleanup 范围；P5 只提供足够 LIR/LIR facts 供后续移除 scaffold |
| `crates/scoopc/src/llvm/emit.rs:30-40` | `LoweredCodegenEntry` 同时保存 HIR、HirFacts、MIR pass view、LIR program、TypeStore、ABI LIR program、ABI pass view、effect facts | P5 要消除 codegen-neutral contract 对 pass view/effect facts 的读取；P7 再清理 HIR scaffold 和 backend entry shape |
| `emit.rs:134-157` | `from_stage_output(...)` 通过 `EffectLoweredStageOutput` 读取 `materialized_pass_view()`、`program()`、`types()`、`effect_facts()` | P5 应让 `LirStageOutput` 直接发布 `lir`、`lir_facts` 和必要 base context |
| `emit.rs:448-650` | module build 把 HIR side tables、MIR pass view、LIR program、TypeStore、effect facts 共同传入 codegen；`materialize_program_abi(...)` 仍需要 pass view/effect facts | P5 负责把 ProgramAbiQuery 的逻辑合同输入迁到 LIR facts；P7 再做 LLVM physical backend 输入清场 |
| `llvm/codegen/effect_lowered/layout/mod.rs:74-85` | `materialize_program_abi(program, source_types, pass_view, effect_facts)` | `ProgramAbiQuery` 混合 codegen-neutral ABI 合同与 LLVM physical type；P5 先发布 neutral LIR facts，P7 再拆 backend physical layout |
| `llvm/codegen/effect_lowered/layout/lookup.rs:10-224` | ABI materializer 从 effect facts 查 body/site facts，从 pass view 查 MIR body/call kind，从 HIR vtable/itable 查 dispatch slot | dynamic invoke 与 dispatch owner/slot 是 P5 LIR contract 缺口 |
| `llvm/codegen/effect_lowered/layout/callable.rs:315-447` | plain callable layout 回读 materialized MIR signature/body/caller-side candidate | plain callable ABI/source body contract 是 P5 LIR contract 缺口 |
| `llvm/codegen/effect_lowered/body/main_entry.rs:7-68` | body lowering仍接收 primary/ABI `MaterializedMirPassView` | P5 应收口 P5-owned source-slice/body contract；P7 再最终移除 backend raw MIR dependency |
| `llvm/codegen/{ordinary_callee.rs,call/abi.rs,mir_body/*,ty.rs,main/*}` | 多处通过 `materialized_pass_view()` 或 `published_late_lowered_program()` 弥合 ordinary callable、ABI 和 type query | P5 不应扩大这些兼容读取；能归入 LIR facts 的 neutral query 必须在 P5 中发布 |

### LIR optimization 当前入口

`crates/scoopc/src/effect_lowered/opt.rs` 已经是事实上的 LIR opt 实现：`optimize_program(...)` / `optimize_program_with_options(...)` 只消费 `LateLoweredProgram`，当前注释明确禁止重新读取 HIR/P3 MIR/P4 solver 结果。

当前实现覆盖：

- `collect_state_redirects(...)` + `rewrite_state_graph(...)`：wrapper state folding / local state-machine 收缩。
- `rewrite_boundary_map(...)`、`rewrite_frame_schema(...)`、`rewrite_captures(...)`：dead boundary、dead slot、dead capture cleanup。
- `prune_resume_interface(...)`、`prune_object_interfaces(...)`：internal resume interface pruning/devirtualization。
- `rewrite_dynamic_invoke_entry(...)`：跟随 state redirect 更新 dynamic invoke entry。

P5 需要把这条隐式 opt helper 提升为正式 LIR optimization family：命名 pass、固定顺序、限定输入输出、增加 verifier/dump 或 pass metadata，并明确它不是普通调用图 optimizer。

## [DONE] TODO-5-INIT：初始化并细化本任务包

- 目标：
  - 分析 `PLAN.md` §4/P4-P5、`PIPELINE_REFACTOR.md` 和当前 `effect_facts` / `effect_lowered` / codegen 输入依赖的真实边界；
  - 生成本任务包的详细任务列表，覆盖 effect facts 只读化、`EffectFactsStageOutput` 收口、正式 LIR 输出、`lir_facts` 和 LIR optimization family；
  - 更新 `TODO.md` 的具体任务索引，用新生成的任务替换或扩展 `TODO-5-INIT` 所在索引行。
- 必须实现的内容：
  1. 列出 effect facts stage 当前嵌套 `MirStageOutput`、修改 MIR 输出本体或重算 MIR-derived facts 的位置。
  2. 列出 `effect_lowered` 当前输出结构、构造点、facts/query 候选项和 codegen 读取点。
  3. 把 P4-P5 拆成数量适中的实现小阶段，每个阶段必须有明确目标、修改范围、验证命令和完成条件。
  4. 在每个实现小阶段后插入独立 review 任务，review 任务必须复审前一阶段是否满足 effect facts purity 或 LIR owner 约束。
  5. 同步更新 `TODO.md` 中的具体任务索引，确保任务 ID、状态和顺序与本文件一致。
- 完成条件：
  - `TODO-5.md` 不再只是范围说明，而是包含可直接执行的详细任务列表；
  - `TODO.md` 的具体任务索引已经同步反映 `TODO-5.md` 的新任务和 `[TODO]` 状态；
  - 本任务完成记录说明为何 P4/P5 可以在同一包内推进，以及阶段间验收门禁。
- 依赖：P3-T07R
- 完成记录：
  - 拆分依据：P4/P5 可以在同一包内推进，因为 P5 的 LIR 输入完整性直接依赖 P4 的 effect facts purity。只有先让 effect facts output 不再嵌套或修改 MIR，LIR stage 才能显式消费 `MIR handoff + effect facts` 并发布自足的 `LIR + lir_facts`。
  - 触碰面记录：已在本文件“触碰面基线”中记录 `EffectFactsStageOutput` 当前 nested `MirStageOutput`、`canonical_snapshot_mut()` 修改点、`MaterializedEffectFactsBuilder` 对 `&mut MaterializedMir` / snapshot `TypeStore` 的写入路径、`EffectFactsTypeContext` 重建点、`EffectLoweredStageOutput` 当前 wrapper 形状、`LateLoweredProgramBuilder` 构造输入、LIR fact/query 候选项、LLVM codegen 读取点和现有 `effect_lowered::opt` 入口。
  - 任务结构：新增 9 个实现阶段和 9 个 review 阶段：`scoopc_effect_facts` crate、effect facts builder 只读化、`EffectFactsStageOutput` 收口、P4 清场、`scoopc_lir_facts` 与 `LirStageOutput` 壳层、LIR contracts/facts 发布、codegen-neutral query 切换、正式 LIR opt pipeline、P5 全包清场。
  - 阶段间验收门禁：进入 P5 前必须满足 effect facts stage 不修改 MIR、P4 output 不嵌套 P3 output、P5 input 显式区分 MIR handoff 与 effect facts handoff；进入 TODO-6 前必须满足 LIR 是 codegen 的唯一 authoritative IR 输入候选，`lir_facts` 覆盖 P5-owned codegen-neutral contracts，剩余 HIR scaffold / LLVM physical layout / global init 闭合明确留给 P6-P8。
  - 验证命令：文档/计划任务仅需检查 markdown/TODO 一致性；本次执行使用 `git diff --check`。

## [DONE] P4-T01：建立独立 `scoopc_effect_facts` crate 与事实数据模型

- 参考：
  - `PLAN.md` §1.2、§1.3、§4/P4
  - `PIPELINE_REFACTOR.md` “fact crate 必须自包含”“分析阶段必须只读输入”“effect facts stage”
  - 本文件“effect facts stage 当前 nested output、MIR 修改点与重算点”
- 目标：
  - 将 effect facts 数据产品从 `scoopc` stage/facade 内部模块中拆出独立 fact crate 或等价独立数据产品；
  - 固定 `EffectFacts` 顶层结构、snapshot binding、callable/body/site facts、step schema、continuation schema、dump/verifier skeleton；
  - 为 P4-T02/T03 的只读 stage 与窄 output 提供落点。
- 必须检查和修改的主要位置：
  - `Cargo.toml`
  - `crates/scoopc/Cargo.toml`
  - 新增 `crates/scoopc_effect_facts/` 或等价独立 fact crate 目录
  - `crates/scoopc/src/effect_facts/`
  - `tools/scoop_tools/src/dependency_gate.rs`
  - `README.md`
- 必须实现的内容：
  1. 新建 `scoopc_effect_facts` crate，至少包含 crate-level 职责文档、`#![forbid(unsafe_code)]`、顶层 facts 类型、dump/verifier skeleton 和单元测试。
  2. 将当前 `MaterializedEffectFacts`、`MirSnapshotBinding`、callable/body/site facts、`StepSchema`、`ContinuationSchema` 等数据模型迁入 fact crate，或先以最小兼容 wrapper 固定公开数据产品边界。
  3. fact crate 只能使用基础 crate 中的 stable identity、span/source/type/effect-row 类型；不得引用 `crate::mir`、`MaterializedMirPassView`、`FunDecl`、`Body`、`InstanceKey`、`TemplateKey` 或 LLVM/backend 类型。
  4. 如当前 `InstanceKey` / `SiteId` / schema identity 还不是基础 crate 可表达的 stable key，先在 `scoopc_ids` 或合适基础 crate 中新增 stage-independent key，再让 fact crate 引用该 key。
  5. 更新 dependency gate，使 `scoopc_effect_facts` 被当作 fact crate 检查，拒绝依赖 `scoopc` facade、stage/backend crate 或其它 fact crate。
  6. `scoopc` facade 只保留必要 re-export / adapter；不得让新的 fact crate 反向依赖 `scoopc` 内部实现。
- 禁止事项：
  - 禁止把 MIR node、MIR pass view、`MaterializedMir` 或 `TypeStore` 放入 fact crate。
  - 禁止让 `scoopc_effect_facts` 依赖 `scoopc_mir_facts`；如果需要引用 MIR fact identity，必须通过基础 stable key 间接表达。
  - 禁止为兼容旧调用点在 fact crate 中 re-export stage output wrapper。
- 验证：
  1. `cargo fmt`
  2. `cargo check -p scoopc_effect_facts`
  3. `cargo test -p scoopc_effect_facts`
  4. `cargo run -p scoop_tools -- dependency-gate`
  5. `cargo clippy --all-targets -- -D warnings`
- 完成条件：
  - effect facts 数据产品有独立 crate / 独立边界；
  - dependency gate 能证明该 fact crate 未依赖 facade、stage crate、backend crate 或其它 fact crate；
  - 现有 `scoopc` effect facts builder/solver 仍可通过 adapter 构造同等事实输出。
- 依赖：TODO-5-INIT
- 完成记录：
  - 改动范围：新增 `crates/scoopc_effect_facts/` fact crate，纳入 workspace、`scoopc` 依赖和 `scoop_tools dependency-gate`；新增 `scoopc_ids::BodyBlockId` 与 `StableEffectInstanceKey` 作为 stage-independent effect facts identity；更新 README 和 `scoopc` facade anchor。
  - 数据模型：独立 crate 现在发布 `EffectFacts = { snapshot_binding, step_schemas, continuation_schemas, callables, bodies }`，覆盖 callable/body/site facts、`StepSchema`、`ContinuationSchema`、dump 和 verifier skeleton，且只依赖 `scoopc_ids` / `scoopc_types` 基础 crate。
  - Adapter 决策：当前生产 builder/solver 仍构造 MIR-keyed `MaterializedEffectFacts`；本任务新增 `MaterializedEffectFacts::to_published_effect_facts(...)`，把现有输出转换并验证为独立 `scoopc_effect_facts::EffectFacts`，为 P4-T02/T03 的只读 builder 和窄 output 提供落点。
  - 验证命令：`cargo fmt`；`cargo check -p scoopc_effect_facts`；`cargo test -p scoopc_effect_facts`；`cargo test -p scoopc --no-default-features effect_facts_stage`；`cargo run -p scoop_tools -- dependency-gate`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
  - 残余风险：`EffectFactsStageOutput` 仍嵌套 `MirStageOutput`，且 effect facts builder 仍可变借用 MIR snapshot；这些是已排序的 P4-T02/P4-T03 范围，本任务未把该过渡形状描述为长期合法边界。

## [TODO] P4-T01R：Review `scoopc_effect_facts` crate 与事实模型

- 参考：P4-T01。
- 重点：
  - `scoopc_effect_facts` 是否满足 fact crate 自包含约束；
  - 数据模型是否覆盖 snapshot binding、callable/body/site facts、step schema、continuation schema；
  - dependency gate 是否阻止 fact crate 依赖 MIR/LIR/codegen stage 或其它 fact crate。
- 必须复查的范围：
  - `Cargo.toml`
  - `crates/scoopc_effect_facts/`
  - `crates/scoopc/src/effect_facts/`
  - `crates/scoopc_ids/` 中本任务新增的 stable identity
  - `tools/scoop_tools/src/dependency_gate.rs`
  - `README.md`
- 验证：
  - 重新运行 P4-T01 的所有验证；
  - 额外运行 `cargo tree -p scoopc_effect_facts`，确认只出现允许的基础依赖。
- 完成条件：
  - review 结论明确写出：effect facts crate 壳层满足 P4/Pipeline fact DAG 约束，或列出阻塞项并在本 review 内修复。
- 依赖：P4-T01
- 完成记录：
  - 待填写。

## [TODO] P4-T02：只读化 effect facts builder 与 effect-owned type context

- 参考：
  - 本文件“effect facts stage 当前 nested output、MIR 修改点与重算点”
  - `crates/scoopc/src/pipeline/effect_facts_stage.rs`
  - `crates/scoopc/src/effect_facts/builder.rs`
- 目标：
  - 移除 effect facts stage 对 `MirStageOutput` / `MaterializedMir` 的可变借用；
  - 将 compiler-generated runtime error effect、step schema tuple carrier、continuation schema 等 P4-owned type additions 收口到 effect-owned context；
  - 保证 `MaterializedMir` 和 `MirFacts` 在 P4 前后不被修改。
- 必须检查和修改的主要位置：
  - `crates/scoopc/src/pipeline/effect_facts_stage.rs`
  - `crates/scoopc/src/effect_facts/builder.rs`
  - `crates/scoopc/src/effect_facts/solver.rs`
  - `crates/scoopc/src/effect_facts/dump.rs`
  - `crates/scoopc/src/effect_lowered/builder.rs`
  - `tests/fixtures/effect_facts/`、`tests/fixtures/effect_lowered/`
- 必须实现的内容：
  1. 将 `MaterializedEffectFactsBuilder::from_materialized_snapshot*` 的输入从 `&mut MaterializedMir` 改为只读 canonical MIR pass view / snapshot reference + explicit effect-owned mutable type context。
  2. 删除 `effect_facts_stage.rs` 中对 `canonical_snapshot_mut()` 的调用；stage 输入可以按值持有或按引用读取 MIR handoff，但不得修改 canonical snapshot。
  3. 将 `find_or_intern_raise_runtime_error_effect(...)`、`canonical_tuple_carrier_ty(...)`、step/continuation schema type interning 产生的新增类型写入 effect-owned type context，并在 effect facts output 中显式发布该 context 或其 stable binding。
  4. 明确 `EffectFactsTypeContext::build(...)` 重建的 index/typeenv/vtable/itable/direct-subclass 信息是 analysis-owned context，不得被重新发布为 MIR/HIR facts 的替代 owner。
  5. 增加 verifier 或测试，证明 P4 运行前后 `MirStageOutput` snapshot binding、pass artifacts metadata 与 MIR `TypeStore` 不发生修改。
  6. 保持 two-pass solver 语义，但第二次构建必须复用只读 MIR 输入和 effect-owned context，不得重新打开 MIR mutable borrow。
- 禁止事项：
  - 禁止通过 `RefCell`、clone-and-write-back 或其它绕路继续修改 `MaterializedMir`。
  - 禁止把 effect-owned type additions 伪装成 MIR-owned type context。
  - 禁止降低 facts 精度来避开 runtime error continuation 或 step schema type interning。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features effect_facts_stage`
  3. `cargo test -p scoopc --no-default-features effect_facts`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/effect_facts`
  5. `cargo clippy --all-targets -- -D warnings`
- 完成条件：
  - effect facts stage 不再可变借用 MIR stage output 或 materialized snapshot；
  - effect-owned type/context additions 有明确 owner 和 dump/verifier；
  - 现有 effect facts solver 精度和 fixtures 不回退。
- 依赖：P4-T01R
- 完成记录：
  - 待填写。

## [TODO] P4-T02R：Review effect facts 只读化结果

- 参考：P4-T02。
- 重点：
  - P4 是否彻底移除 `canonical_snapshot_mut()` / `&mut MaterializedMir` 输入；
  - effect-owned type context 是否不再写回 MIR；
  - two-pass solver 是否仍保持原有 facts 精度。
- 必须复查的范围：
  - `crates/scoopc/src/pipeline/effect_facts_stage.rs`
  - `crates/scoopc/src/effect_facts/`
  - `crates/scoopc/src/effect_lowered/`
  - effect facts fixtures / dumps
- 验证：
  - 重新运行 P4-T02 的所有验证；
  - 额外搜索 `canonical_snapshot_mut\(|&mut MaterializedMir|from_materialized_snapshot\(`，确认活跃生产路径没有 P4 mutable MIR 输入。
- 完成条件：
  - review 结论明确写出：effect facts stage 对 MIR 输入只读，或列出阻塞项并在本 review 内修复。
- 依赖：P4-T02
- 完成记录：
  - 待填写。

## [TODO] P4-T03：收口 `EffectFactsStageOutput` 与 P5 输入边界

- 参考：
  - `PIPELINE_REFACTOR.md` “stage output wrapper 规则”
  - 本文件“effect facts stage 当前 nested output、MIR 修改点与重算点”
  - 本文件“effect_lowered 当前输出结构、构造点与 LIR fact/query 候选项”
- 目标：
  - 让 `EffectFactsStageOutput` 只发布 effect facts 及 effect-owned context/binding；
  - 删除 P4 output 对 `MirStageOutput`、`MaterializedMir`、pass view、MIR facts、MIR type store 的转发访问器；
  - 改造 LIR stage input，使 P5 显式消费 `MirStageOutput` / MIR pass view / MIR facts 与 `EffectFactsStageOutput`，而不是通过 nested P4 wrapper 回看。
- 必须检查和修改的主要位置：
  - `crates/scoopc/src/pipeline/effect_facts_stage.rs`
  - `crates/scoopc/src/pipeline/effect_lowering_stage.rs`
  - `crates/scoopc/src/pipeline/mod.rs`
  - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`
  - `crates/scoopc/src/effect_lowered/{builder.rs,ir.rs,dump.rs}`
  - `crates/scoopc/src/effect_lowered/materialize/tests.rs`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/layout/tests/mod.rs`
- 必须实现的内容：
  1. 将 `EffectFactsStageOutput` 字段收口为 `effect_facts` 和必要 effect-owned context/binding；删除 `mir_stage_output()`、`file()`、`materialized_mir()`、`materialized_pass_view()`、`mir_facts()`、`types()` 等上游转发方法。
  2. 引入显式 P5 input 形状，例如 `LirStageInput { mir_stage_output, effect_facts_stage_output }` 或等价参数，保证 LIR stage 依赖是显式的，不通过 P4 output 嵌套。
  3. 更新 `build_effect_lowered_stage_output(...)`、dump helpers、LLVM stage orchestration 和 tests，使 MIR handoff 与 effect facts handoff 分开传递。
  4. 更新 stable dump，使 P4 dump 只描述 effect facts / snapshot binding，P5 dump 才描述 MIR+effect facts 共同构造出的 LIR。
  5. 删除任何把 `EffectFactsStageOutput` 当成 P3/P4/P5 多阶段 bundle 的测试 helper 或 production helper。
- 禁止事项：
  - 禁止把 `MirStageOutput` 换个字段名继续塞进 `EffectFactsStageOutput`。
  - 禁止让 P5 为了少改调用点重新从 effect facts output 查 pass view 或 MIR facts。
  - 禁止删除真实所需的 LIR 构造输入；需要 MIR IR 时必须显式建模。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features effect_facts_stage`
  3. `cargo test -p scoopc --no-default-features effect_lowering_stage`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/effect_facts`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`
  6. `cargo clippy --all-targets -- -D warnings`
- 完成条件：
  - `EffectFactsStageOutput` 不再嵌套或转发 P3 `MirStageOutput`；
  - P5 input 明确表达 MIR handoff + effect facts handoff；
  - effect facts dump/test 证明 P4 output 是窄产物。
- 依赖：P4-T02R
- 完成记录：
  - 待填写。

## [TODO] P4-T03R：Review `EffectFactsStageOutput` 收口结果

- 参考：P4-T03。
- 重点：
  - P4 output 是否不再嵌套或转发 P3 output；
  - P5 input 是否显式建模 MIR handoff 与 effect facts handoff；
  - tests/helpers 是否没有保留旧 nested bundle 习惯。
- 必须复查的范围：
  - `crates/scoopc/src/pipeline/effect_facts_stage.rs`
  - `crates/scoopc/src/pipeline/effect_lowering_stage.rs`
  - `crates/scoopc/src/pipeline/mod.rs`
  - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`
  - effect facts / effect lowered tests
- 验证：
  - 重新运行 P4-T03 的所有验证；
  - 额外搜索 `mir_stage_output\(|materialized_mir\(|materialized_pass_view\(|mir_facts\(` 在 `EffectFactsStageOutput` impl 上不再存在上游转发。
- 完成条件：
  - review 结论明确写出：`EffectFactsStageOutput = { effect_facts }` 或等价窄输出成立，或列出阻塞项并在本 review 内修复。
- 依赖：P4-T03
- 完成记录：
  - 待填写。

## [TODO] P4-T04：P4 全包清场、文档同步与依赖审计

- 参考：P4-T01 至 P4-T03R。
- 目标：
  - 对 effect facts purity 做全仓清场；
  - 更新文档、README、dump 说明和依赖门禁；
  - 为 P5 正式 LIR output 建立稳定进入条件。
- 必须检查和修改的主要位置：
  - `TODO-5.md`
  - `PIPELINE-CLEANUP.md`
  - `PIPELINE_REFACTOR.md`（仅在设计边界实际变化时）
  - `README.md`
  - `crates/scoopc/src/pipeline/`
  - `crates/scoopc/src/effect_facts/`
  - `crates/scoopc_effect_facts/`
  - `tools/scoop_tools/src/dependency_gate.rs`
- 必须实现的内容：
  1. 搜索确认 effect facts stage 没有 mutable MIR input、没有 nested `MirStageOutput` output、没有通过 P4 output 转发 MIR/pass view/type store。
  2. 更新 cleanup/design 文档中 P4 状态：哪些问题已解决，哪些 codegen/HIR scaffold residual 明确留给 P5/P7。
  3. 更新 README / crate overview，说明 `scoopc_effect_facts` 的职责和依赖规则。
  4. 更新 TODO-5 中 P4 任务完成记录，明确 P5 的进入门禁：MIR handoff 与 effect facts handoff 分开、effect facts output 窄化、MIR snapshot 不被 P4 修改。
- 禁止事项：
  - 禁止在清场任务中提前实施 P5 LIR facts 或 P7 backend cleanup。
  - 禁止把文档改成承认 nested output 为长期合法形状。
- 验证：
  1. `cargo fmt`
  2. `cargo run -p scoop_tools -- dependency-gate`
  3. `cargo test -p scoopc_effect_facts`
  4. `cargo test -p scoopc --no-default-features effect_facts_stage`
  5. `cargo clippy --all-targets -- -D warnings`
  6. `git diff --check`
- 完成条件：
  - P4 purity 的代码、tests、docs 和 dependency gate 状态一致；
  - P5 任务可以在不依赖 P4 nested wrapper 的前提下开始。
- 依赖：P4-T03R
- 完成记录：
  - 待填写。

## [TODO] P4-T04R：Review P4 全包完成度

- 参考：P4-T04。
- 重点：
  - P4 的只读 analysis 输出约束是否全仓成立；
  - docs 是否没有把 P4 过渡 wrapper 描述为合法长期边界；
  - P5 进入条件是否清晰。
- 必须复查的范围：
  - P4-T01 至 P4-T04 的全部改动
  - `PIPELINE-CLEANUP.md`
  - `PIPELINE_REFACTOR.md`
  - `README.md`
  - dependency gate 输出
- 验证：
  - 重新运行 P4-T04 的所有验证；
  - 额外搜索 `canonical_snapshot_mut\(|EffectFactsStageOutput.*MirStageOutput|mir_stage_output\(`，确认没有 P4 输出嵌套或修改 MIR 的活跃路径。
- 完成条件：
  - review 结论明确写出：P4 effect facts purity 已完成，或列出阻塞项并在本 review 内修复。
- 依赖：P4-T04
- 完成记录：
  - 待填写。

## [TODO] P5-T01：建立 `scoopc_lir_facts` crate 与正式 `LirStageOutput` 壳层

- 参考：
  - `PLAN.md` §4/P5
  - `PIPELINE_REFACTOR.md` “LIR 是 codegen 的唯一 IR 输入”“stage output wrapper 规则”
  - 本文件“effect_lowered 当前输出结构、构造点与 LIR fact/query 候选项”
- 目标：
  - 新建独立 `scoopc_lir_facts` crate 或等价独立 facts 数据产品；
  - 将当前 `EffectLoweredStageOutput` 退化或替换为正式 `LirStageOutput = { lir, lir_facts }` 壳层；
  - 固定 `LateLoweredProgram` 是当前正式 LIR 本体，而不是 effect-lowering 私有临时产物。
- 必须检查和修改的主要位置：
  - `Cargo.toml`
  - `crates/scoopc/Cargo.toml`
  - 新增 `crates/scoopc_lir_facts/`
  - `crates/scoopc/src/pipeline/effect_lowering_stage.rs`
  - `crates/scoopc/src/effect_lowered/`
  - `tools/scoop_tools/src/dependency_gate.rs`
  - `README.md`
- 必须实现的内容：
  1. 新建 `scoopc_lir_facts` crate，包含 crate-level 职责文档、`#![forbid(unsafe_code)]`、`LirFacts` 顶层结构、dump/verifier skeleton 和单元测试。
  2. 将 `EffectLoweredStageOutput` rename / wrap 为 `LirStageOutput`；若保留旧模块名过渡，公开 API 必须表达 LIR stage 语义。
  3. `LirStageOutput` 字段只能包含 LIR 本体、`LirFacts` 和必要 base context / type context；不得保存 `EffectFactsStageOutput` 或其它上游整包输出。
  4. 更新 pipeline facade、dump 命令和 tests，使 P5 输出名、dump 标题和任务文档一致。
  5. 更新 dependency gate，把 `scoopc_lir_facts` 纳入 fact crate 检查。
- 禁止事项：
  - 禁止让 `scoopc_lir_facts` 依赖 `scoopc_effect_facts`、`scoopc_mir_facts`、`scoopc` facade、MIR/LIR stage 或 LLVM backend。
  - 禁止只改名字但继续在 output 中嵌套 P4 output。
  - 禁止在本任务中大规模改 LLVM backend；本任务只建立 LIR output/facts 壳层。
- 验证：
  1. `cargo fmt`
  2. `cargo check -p scoopc_lir_facts`
  3. `cargo test -p scoopc_lir_facts`
  4. `cargo test -p scoopc --no-default-features effect_lowering_stage`
  5. `cargo run -p scoop_tools -- dependency-gate`
  6. `cargo clippy --all-targets -- -D warnings`
- 完成条件：
  - `LirStageOutput = { lir, lir_facts }` 壳层成立，且不嵌套 P4 output；
  - `scoopc_lir_facts` 可独立编译、测试并通过 dependency gate；
  - 当前 `LateLoweredProgram` 被文档和 API 视为正式 LIR 本体。
- 依赖：P4-T04R
- 完成记录：
  - 待填写。

## [TODO] P5-T01R：Review `lir_facts` crate 与 LIR output 壳层

- 参考：P5-T01。
- 重点：
  - `scoopc_lir_facts` 是否满足 fact crate DAG；
  - `LirStageOutput` 是否不嵌套 P4/P3 output；
  - `LateLoweredProgram` 是否被稳定作为 LIR 本体发布。
- 必须复查的范围：
  - `Cargo.toml`
  - `crates/scoopc_lir_facts/`
  - `crates/scoopc/src/pipeline/effect_lowering_stage.rs`
  - `crates/scoopc/src/effect_lowered/`
  - `tools/scoop_tools/src/dependency_gate.rs`
  - `README.md`
- 验证：
  - 重新运行 P5-T01 的所有验证；
  - 额外运行 `cargo tree -p scoopc_lir_facts`，确认只出现允许的基础依赖。
- 完成条件：
  - review 结论明确写出：LIR output 壳层和 `lir_facts` crate 满足 P5/Pipeline 边界，或列出阻塞项并在本 review 内修复。
- 依赖：P5-T01
- 完成记录：
  - 待填写。

## [TODO] P5-T02：发布 LIR callable、dynamic invoke、dispatch 与 resume contracts

- 参考：
  - 本文件“effect_lowered 当前输出结构、构造点与 LIR fact/query 候选项”
  - `crates/scoopc/src/effect_lowered/{ir.rs,builder.rs,materialize/*}`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/layout/`
- 目标：
  - 补齐 LIR/LIR facts 中 codegen-neutral 合同，使 plain callable、effect-step callable、dynamic invoke、dispatch owner/slot、continuation/resume publication 不再需要 LLVM 侧回扫 raw MIR/effect facts/HIR tables 才能理解；
  - 将现有 `LateLoweredProgramBuilder` 已经构造但未作为 facts/query 固定的内容发布到 `LirFacts`。
- 必须检查和修改的主要位置：
  - `crates/scoopc_lir_facts/`
  - `crates/scoopc/src/effect_lowered/ir.rs`
  - `crates/scoopc/src/effect_lowered/builder.rs`
  - `crates/scoopc/src/effect_lowered/materialize/`
  - `crates/scoopc/src/pipeline/effect_lowering_stage.rs`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/layout/{lookup.rs,callable.rs,carrier.rs}`
- 必须实现的内容：
  1. 在 `LirFacts` 中发布 callable inventory、body version identity、stable callable/instance identity 和 source slice identity。
  2. 为 plain callable 发布 ordinary ABI、parameter/return type refs、body source slices、plain call-site contracts 和本地 effect/control contract。
  3. 为 effect-step callable 发布 step schema binding、dynamic invoke entry、state graph/frame/boundary/resume state query keys 和 continuation object binding。
  4. 发布 dynamic invoke carrier contract：source kind、carrier/source type、arg count、target callable/body version、entry/complete state。
  5. 发布 virtual/interface dispatch owner/slot selection 的 backend-neutral contract；LLVM 不应再在 layout 阶段扫描 HIR vtable/itable 或 raw MIR call metadata 来决定 slot。
  6. 发布 continuation/resume facts：surface resume dispatch inventory、resume packing completeness、wrapper projection、one-shot runtime error publication。
  7. 增加 LIR facts verifier/dump，验证 `LateLoweredProgram` 与 `LirFacts` 一一对应，不存在 callable / schema / continuation orphan。
- 禁止事项：
  - 禁止把 LLVM `StructType`、`FunctionType`、`BasicTypeEnum` 等 physical ABI 放入 `LirFacts`。
  - 禁止把 raw MIR body 或 HIR declaration table 塞进 LIR facts。
  - 禁止为了快速通过测试只覆盖 effect-step callable 而遗漏 plain callable。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc_lir_facts`
  3. `cargo test -p scoopc --no-default-features effect_lowered`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`
  5. `cargo clippy --all-targets -- -D warnings`
- 完成条件：
  - LIR/LIR facts 能表达 plain callable、effect-step callable、dynamic invoke、dispatch owner/slot 和 continuation/resume publication；
  - verifier/dump 覆盖上述 contract；
  - 后续 codegen-neutral query 切换不需要重新设计 LIR 数据模型。
- 依赖：P5-T01R
- 完成记录：
  - 待填写。

## [TODO] P5-T02R：Review LIR contract 与 facts 完整度

- 参考：P5-T02。
- 重点：
  - LIR facts 是否覆盖本文件列出的候选项；
  - plain callable 与 effect-step callable 是否同等完整；
  - dynamic invoke、dispatch owner/slot、resume publication 是否不依赖 LLVM 侧临时扫描才能成立。
- 必须复查的范围：
  - `crates/scoopc_lir_facts/`
  - `crates/scoopc/src/effect_lowered/`
  - LIR facts dump/verifier
  - effect lowered fixtures
- 验证：
  - 重新运行 P5-T02 的所有验证；
  - 额外搜索 `LateLoweredProgramBuilder::from_canonical_inputs` 的输出是否总是同步构造 `LirFacts`。
- 完成条件：
  - review 结论明确写出：LIR facts/query layer 足以支撑 P5/P7 codegen handoff，或列出阻塞项并在本 review 内修复。
- 依赖：P5-T02
- 完成记录：
  - 待填写。

## [TODO] P5-T03：切换 codegen-neutral ABI/query surface 到 `LIR + lir_facts`

- 参考：
  - 本文件“当前 codegen 读取点”
  - `crates/scoopc/src/llvm/emit.rs`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/layout/`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/body/`
- 目标：
  - 让 ProgramAbi materialization 和 effect-lowered body lowering 中属于 codegen-neutral 的查询改为读取 `LirStageOutput` / `LirFacts`；
  - 删除 `LirStageOutput` 上为 codegen 提供的 `materialized_pass_view()`、`effect_facts()`、`mir_facts()` 等上游转发；
  - 保留 TODO-6/P7 才清理的 HIR scaffold 和 LLVM physical layout，但不再让 P5-owned contract 依赖 raw MIR/effect facts/HIR scan。
- 必须检查和修改的主要位置：
  - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`
  - `crates/scoopc/src/llvm/emit.rs`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/layout/{mod.rs,lookup.rs,callable.rs,carrier.rs}`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/body/{main_entry.rs,main_carrier.rs,mod.rs}`
  - `crates/scoopc/src/llvm/codegen/{ordinary_callee.rs,call/abi.rs,mir_body/*,ty.rs}`
- 必须实现的内容：
  1. 将 `materialize_program_abi(...)` 的 logical contract 输入从 `program + source_types + pass_view + effect_facts` 改为 `lir + lir_facts + base/type context`；LLVM physical type/layout 仍留在 backend 层。
  2. 将 dynamic call-site kind、carrier source type、dispatch slot、plain callable signature/body source slice 等 codegen-neutral 查询改为读取 LIR facts。
  3. 将 continuation/resume publication、resume packing completeness 和 wrapper projection 查询改为读取 LIR facts。
  4. 删除或降级 `LirStageOutput` 上暴露上游 pass view/effect facts/MIR facts 的 public accessors；测试 helper 如仍需上游输入，必须改为显式 fixture builder，不得伪装为 stage output API。
  5. 标注仍属于 TODO-6/P7 的 residual：HIR scaffold、global init direct HIR paths、LLVM physical ABI layout、多 TypeStore 桥接、backend-specific reachability。
- 禁止事项：
  - 禁止在本任务中把 LLVM backend 最终输入改成看似干净但实际通过 hidden global/state 回读上游。
  - 禁止把 `ProgramAbiQuery` 直接搬进 `lir_facts`，因为它包含 LLVM physical 类型。
  - 禁止用 fixture-only LIR facts 填洞；production builder 必须发布同样 contract。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --features llvm effect_lowered`
  3. `cargo test -p scoopc --features llvm llvm::tests::late_lower`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  5. `cargo clippy --all-targets -- -D warnings`
- 完成条件：
  - Program ABI materialization 的 codegen-neutral contract 来源是 `LIR + lir_facts`；
  - `LirStageOutput` 不再公开上游 pass view/effect facts/MIR facts accessors；
  - TODO-6/P7 residual 被明确记录，且不是 P5-owned contract 缺失。
- 依赖：P5-T02R
- 完成记录：
  - 待填写。

## [TODO] P5-T03R：Review codegen-neutral query 切换结果

- 参考：P5-T03。
- 重点：
  - P5-owned ABI/query 是否从 LIR facts 读取；
  - `LirStageOutput` 是否不再为 codegen 转发 pass view/effect facts/MIR facts；
  - TODO-6/P7 residual 是否被准确隔离。
- 必须复查的范围：
  - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`
  - `crates/scoopc/src/llvm/emit.rs`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/`
  - `crates/scoopc_lir_facts/`
- 验证：
  - 重新运行 P5-T03 的所有验证；
  - 额外搜索 `LirStageOutput` / successor output impl 中是否仍有 `materialized_pass_view|effect_facts|mir_facts|effect_facts_stage_output` public accessors。
- 完成条件：
  - review 结论明确写出：P5 codegen-neutral query 已切到 `LIR + lir_facts`，或列出阻塞项并在本 review 内修复。
- 依赖：P5-T03
- 完成记录：
  - 待填写。

## [TODO] P5-T04：建立正式 LIR optimization family 与 pass pipeline

- 参考：
  - `PLAN.md` §1.7、§5
  - `PIPELINE_REFACTOR.md` “优化框架” / “LIR opt”
  - 本文件“LIR optimization 当前入口”
- 目标：
  - 将 `effect_lowered::opt` 从隐式 helper 提升为正式 LIR optimization family；
  - 固定 local state-machine elimination、简单 higher-order wrapper 定向 inline/devirt、wrapper state folding、dead state / dead slot cleanup 的 pass 名称、顺序、输入输出和 verifier；
  - 明确禁止 LIR opt 承担普通调用图/全程序优化职责。
- 必须检查和修改的主要位置：
  - `crates/scoopc/src/effect_lowered/opt.rs`
  - `crates/scoopc/src/pipeline/effect_lowering_stage.rs` 或 successor LIR stage
  - `crates/scoopc/src/effect_lowered/ir.rs`
  - `crates/scoopc_lir_facts/`
  - `tests/fixtures/effect_lowered/`
- 必须实现的内容：
  1. 建立显式 LIR opt pipeline driver，列出 pass 顺序和 opt-level / preservation options。
  2. 将现有 state redirect、boundary/frame/capture rewrite、resume packing pruning、dynamic invoke rewrite 分配到命名 pass。
  3. 为 simple higher-order wrapper 定向 inline/devirt 留出明确 pass owner；若当前能力不足，必须添加最小可执行 pass skeleton、verifier 和后续任务记录，不能把普通 MIR devirt 挪到 LIR。
  4. 在 pipeline metadata / `LirFacts` 中记录 LIR opt 运行结果或 revision binding，保证 facts 与 post-opt LIR 对齐。
  5. 增加 verifier，确认 opt 后 state graph、boundary map、frame schema、continuation object、resume packing、dynamic invoke entry 不存在 dangling references。
  6. 更新 dump/fixtures，使 LIR opt family 的运行边界可见。
- 禁止事项：
  - 禁止在 LIR opt 中重新读取 HIR、MIR pass view 或 effect solver 输入。
  - 禁止做普通调用图 devirtualization、全程序 inlining 或 source semantic checks。
  - 禁止只更新 comments 而不建立可验证 pipeline metadata / verifier。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features effect_lowered::opt`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`
  4. `cargo test -p scoopc_lir_facts`
  5. `cargo clippy --all-targets -- -D warnings`
- 完成条件：
  - LIR optimization family 有显式 pass driver、metadata/dump/verifier；
  - pass 范围被限定在 effect/control LIR-owned 窄优化；
  - post-opt LIR 与 `LirFacts` 对齐。
- 依赖：P5-T03R
- 完成记录：
  - 待填写。

## [TODO] P5-T04R：Review LIR optimization family

- 参考：P5-T04。
- 重点：
  - LIR opt 是否只消费 LIR/LIR facts；
  - pass family 是否没有变成普通调用图 optimizer；
  - post-opt verifier 是否覆盖 dangling state/boundary/frame/resume references。
- 必须复查的范围：
  - `crates/scoopc/src/effect_lowered/opt.rs`
  - LIR stage output / pipeline metadata
  - `crates/scoopc_lir_facts/`
  - effect lowered fixtures
- 验证：
  - 重新运行 P5-T04 的所有验证；
  - 额外搜索 LIR opt 中是否出现 HIR/MIR/effect solver 输入读取。
- 完成条件：
  - review 结论明确写出：LIR opt family 符合 P5 窄优化约束，或列出阻塞项并在本 review 内修复。
- 依赖：P5-T04
- 完成记录：
  - 待填写。

## [TODO] P5-T05：P5 全包清场、文档同步与依赖审计

- 参考：P5-T01 至 P5-T04R。
- 目标：
  - 对 P4/P5 整包边界做最终清场；
  - 同步文档、README、TODO 索引和 dependency gate；
  - 为 TODO-6/P6-P8 提供稳定进入门禁。
- 必须检查和修改的主要位置：
  - `TODO.md`
  - `TODO-5.md`
  - `TODO-6.md`（仅在进入门禁说明需要同步时）
  - `PIPELINE-CLEANUP.md`
  - `PIPELINE_REFACTOR.md`（仅在设计边界实际变化时）
  - `README.md`
  - `crates/scoopc_effect_facts/`
  - `crates/scoopc_lir_facts/`
  - `tools/scoop_tools/src/dependency_gate.rs`
- 必须实现的内容：
  1. 搜索确认没有 `EffectFactsStageOutput` / `LirStageOutput` 嵌套上游整包 stage output 的生产路径。
  2. 搜索确认 effect facts stage 不修改 MIR，LIR opt 不读取 HIR/MIR/effect solver 输入。
  3. 更新 `PIPELINE-CLEANUP.md`，把 P4/P5 已解决项和 TODO-6/P7 residual 分开记录。
  4. 更新 README / crate overview，说明 `scoopc_effect_facts`、`scoopc_lir_facts` 和 LIR output 的职责。
  5. 更新 `TODO.md` 和本文件完成记录，明确进入 TODO-6 的门禁：effect facts 不修改 MIR 输出本体，LIR 是 codegen 唯一 authoritative IR 输入候选，backend residual 只剩 P6/P7/P8 范围。
- 禁止事项：
  - 禁止把未完成的 P7 backend cleanup 描述为已完成。
  - 禁止用文档承认 LLVM 直接读取 HIR/raw MIR/effect facts 是长期合法边界。
  - 禁止新增下一包任务来规避本包完成条件；除非发现真实阻塞项，必须在当前任务内清场。
- 验证：
  1. `cargo fmt`
  2. `cargo run -p scoop_tools -- dependency-gate`
  3. `cargo test -p scoopc_effect_facts`
  4. `cargo test -p scoopc_lir_facts`
  5. `cargo test -p scoopc --no-default-features effect_facts_stage`
  6. `cargo test -p scoopc --no-default-features effect_lowered`
  7. `cargo clippy --all-targets -- -D warnings`
  8. `git diff --check`
- 完成条件：
  - P4/P5 的 code、tests、docs 和 dependency gate 状态一致；
  - `TODO.md` 与 `TODO-5.md` 状态同步；
  - TODO-6 的 P6/P7/P8 初始化可以从清晰的 `LIR + lir_facts + base context` residual 边界开始。
- 依赖：P5-T04R
- 完成记录：
  - 待填写。

## [TODO] P5-T05R：Review P5 全包完成度

- 参考：P5-T05。
- 重点：
  - P4/P5 输出边界是否满足 `EffectFactsStageOutput = { effect_facts }` 与 `LirStageOutput = { lir, lir_facts }`；
  - LIR facts/query layer 是否足以作为 P7 backend cleanup 的输入基础；
  - P6/P7/P8 residual 是否没有被错误归入已完成范围。
- 必须复查的范围：
  - P4-T01 至 P5-T05 的全部改动
  - `TODO.md`
  - `TODO-5.md`
  - `PIPELINE-CLEANUP.md`
  - `PIPELINE_REFACTOR.md`
  - `README.md`
  - dependency gate 输出
- 验证：
  - 重新运行 P5-T05 的所有验证；
  - 额外搜索 `StageOutput` 嵌套上游整包、`EffectLoweredStageOutput` legacy public API、LIR opt 读取上游输入等 residual。
- 完成条件：
  - review 结论明确写出：P4/P5 全包完成，TODO-6 可以开始初始化，或列出阻塞项并在本 review 内修复。
- 依赖：P5-T05
- 完成记录：
  - 待填写。
