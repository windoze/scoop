# TODO-3：HIR barrier + hir_facts

> 生成时间：2026-05-21
> 计划基线：[`PLAN.md`](./PLAN.md) §4/P2
> 设计基线：[`PIPELINE_REFACTOR.md`](./PIPELINE_REFACTOR.md)
> 审计基线：[`PIPELINE-CLEANUP.md`](./PIPELINE-CLEANUP.md)
> 索引：[`TODO.md`](./TODO.md)
> 顺序约束：严格按本文件任务顺序推进；每个实现任务后必须执行紧随其后的 review 任务。
> 本包目标：把 `AST -> HIR` 固定为 cone-level semantic frontend barrier，发布独立 `hir_facts`，并消除 `LoweredHir` / `TypedHirEffectContracts` / `ProgramFacts` 的职责重叠。

## 全局约束

- 本包只处理 `PLAN.md` 的 P2：HIR semantic barrier、`hir_facts`、declaration legality gate 和后续阶段错误边界；不得提前把 P3 MIR pass pipeline、P4 effect facts purity、P5 LIR handoff 或后端清理塞进本包。
- `hir_facts` 必须是独立 fact crate 或等价独立数据产品；fact crate 只能依赖基础 crate，不得依赖 `scoopc` facade、HIR/MIR/effect/LIR/codegen stage crate 或其它 fact crate。
- `HirStageOutput = { hir, hir_facts }` 语义必须成立；后续阶段需要 HIR 语义时应显式消费 `hir_facts`，不能从 `LoweredHir`、`ProgramFacts` 或 fallback side table 重建同一事实。
- `LoweredHir` 不得长期携带 MIR 产物、request-root materialization 结果、pass view 或后端专用 bundle；HIR 阶段可以保留 HIR-owned 内部 helper，但不能作为跨阶段 authoritative fact surface。
- 通过 HIR 屏障后，MIR/effect facts/LIR/codegen 不得再以普通 Scoop 源码语义错误的形式失败；后续阶段只能报告 compiler bug/impossible state、output drift、环境/toolchain/link/runtime 类失败。
- declaration legality 必须在 HIR 屏障内收口：`@CallingConvention` 函数不能是 generic；global object/top-level `val`/top-level `var` 这类 global roots 必须是 monomorphic；top-level `var` 必须显式选择 `@Global` 或 `@ThreadLocal` storage policy。
- 若实现中发现某类语义只能靠 MIR/codegen 兜底才能报错，必须把它前移到 HIR barrier，或在本文件当前任务前插入最小必要前置任务并停止。
- 每个任务完成后，在该任务的“完成记录”下写明改动范围、核心决策、验证命令和残余风险。

## 触碰面基线

本节是 `TODO-3-INIT` 的仓库搜索记录，后续 P2 任务应优先从这些位置开始，不再重复做开放式仓库搜索。

### `LoweredHir` 当前字段、side tables 与构造点

`LoweredHir` 定义在 `crates/scoopc/src/hir/lower/types.rs:310-410`。主要构造点是 `crates/scoopc/src/hir/lower/main/entry.rs:310-346`、`crates/scoopc/src/hir/lower/main/compilation_unit.rs:229-265`、`crates/scoopc/src/hir/lower/main/compilation_unit.rs:986-1022`；production via-MIR 路径还在 `crates/scoopc/src/hir/lower/main/compilation_unit.rs:414-448` 先 materialize MIR，再把 `materialized_mir` 挂回 `LoweredHir`。

| 字段 | 当前职责 | 主要读取点 | P2 处理方向 |
| --- | --- | --- | --- |
| `file` | HIR 本体 | HIR dump/completeness、MIR lowering、LLVM HIR scaffold、ScoopIR export | 保留为 HIR stage IR |
| `stable_cone_key` | cone identity/base context | RTTI、LLVM identity/codegen、stable owner key | 保留在 HIR output context 或 `hir_facts` 中的 base identity 引用 |
| `source_cones` | source path -> cone metadata | LLVM codegen、stable type param key/context | 改为 HIR output context / project-model-backed fact，不由 backend 回猜 |
| `stable_type_param_keys` | type/effect param stable owner/index key | LLVM codegen stable identity | 归入 declaration/entity `hir_facts` 或 output context |
| `member_funs` | lowered member/computed getter HIR bodies | HIR completeness、MIR lowering/materialization、ProgramFacts、LLVM fun index | 明确作为 HIR IR body inventory，事实只发布 callable metadata |
| `materialized_mir` | canonical MIR snapshot/pass artifacts | `materialized_mir()` / `materialized_pass_view()`、MIR stage、effect facts/LIR/codegen/tests | 移出 HIR，改由 MIR stage 发布 |
| `types` | unified `TypeStore` | HIR stage、MIR lowering/materialization、effect analysis、RTTI、LLVM、ScoopIR export | 作为 HIR stage type context 明确发布，避免多 `TypeStore` 桥 |
| `struct_layouts` | struct field/layout metadata | ProgramFacts、MIR lowering facts、LLVM layout/codegen | 归入 declaration/entity `hir_facts` |
| `enum_layouts` | enum variant/layout metadata | MIR lowering facts、LLVM layout/codegen | 归入 declaration/entity `hir_facts` |
| `extern_funs` | `@Extern` function metadata | HIR contracts、LLVM declarations/codegen | 归入 native/extern declaration `hir_facts` |
| `native_callable_funs` | body-bearing `@CallingConvention` metadata | MIR materialization inputs、LLVM declarations/codegen | 归入 native callable `hir_facts`，并由 barrier 固定 non-generic 合法性 |
| `extern_globals` | `@Extern` global metadata | Typed HIR contracts、MIR roots、LLVM globals | 归入 global root `hir_facts` |
| `extern_libs` | driver/link metadata | pipeline artifact emission | 作为 HIR/build contract metadata 显式输出，不混入 HIR IR body |
| `top_level_vars` | annotated top-level mutable globals | HIR contracts、ProgramFacts、MIR roots、LLVM globals/init | 归入 global root/storage policy `hir_facts` |
| `top_level_immutable_values` | top-level immutable values | HIR contracts、ProgramFacts、MIR roots、RTTI/LLVM init | 归入 top-level init/root `hir_facts` |
| `top_level_fun_call_sites` | resolved direct/operator call targets | MIR lowering facts、LLVM call lowering | 归入 source-site typed contract `hir_facts` |
| `call_arg_bindings` | canonical argument slot binding | MIR lowering facts | 归入 source-site typed contract `hir_facts` |
| `with_update_contracts` | copy/update aggregate contract | TypedHirEffectContracts、MIR lowering facts/preflight | 归入 source-site typed contract `hir_facts` |
| `assign_place_contracts` | assignment LHS typed place contract | HIR completeness、TypedHirEffectContracts、MIR lowering facts | 归入 source-site typed contract `hir_facts` |
| `object_inits` | object singleton init/properties | ProgramFacts、MIR hidden init/materialize、RTTI/LLVM | 归入 global root/object metadata `hir_facts`；object body仍属 HIR |
| `class_inits` | class init/field/ctor metadata | ProgramFacts、MIR hidden init/materialize、RTTI/LLVM | 归入 declaration/entity `hir_facts` |
| `class_vtables` | class vtable slots | MIR materialize、LLVM layout/reachability | 归入 dispatch metadata `hir_facts`，后续 MIR-derived 派生事实再由 MIR facts 接管 |
| `interfaces` | interface metadata/method slots | MIR materialize、LLVM layout/reachability | 归入 dispatch metadata `hir_facts` |
| `class_itables` | class interface table entries | MIR materialize、LLVM layout/reachability | 归入 dispatch metadata `hir_facts` |
| `ctor_call_sites` | constructor call target sites | ProgramFacts、MIR lowering facts、LLVM call/reachability | 归入 source-site typed contract `hir_facts` |
| `dispatch_call_sites` | virtual/interface dispatch sites | MIR lowering facts、LLVM reachability/codegen | 归入 source-site dispatch contract `hir_facts` |
| `effect_op_call_sites` | perform/effect op binding fallback | TypedHirEffectContracts、MIR lowering facts、LLVM codegen | 归入 source-site effect contract `hir_facts`，消除 fallback |
| `handle_payload_tuple_tys` | handler payload tuple type side table | HIR lowering/effect tests and typed contracts | 归入 source-site effect contract `hir_facts` if still needed downstream |
| `continuation_resume_call_sites` | resume site spans/contracts | TypedHirEffectContracts、ProgramFacts、MIR lowering facts、LLVM/effect analysis | 归入 source-site continuation contract `hir_facts` |
| `non_pure_continuation_resume_call_sites` | outward/non-pure resume sites | ProgramFacts、MIR lowering facts/effect analysis | 归入 source-site continuation contract `hir_facts` |
| `when_pat_binding_tys` | precise pattern binder types | MIR lowering facts、LLVM codegen | 归入 source-site typed contract `hir_facts` |
| `nominal_kinds` | nominal kind index | MIR lowering facts、LLVM codegen | 归入 declaration/entity `hir_facts` |
| `nominal_variances` | nominal variance index | declaration/type metadata consumers | 归入 declaration/entity `hir_facts` |
| `direct_supertypes` | nominal direct supertypes | MIR materialize, LLVM reachability/codegen | 归入 declaration/entity `hir_facts` |
| `builtins` | builtin `TypeId` set for this `TypeStore` | HIR completeness/tests, LLVM codegen | 与 `types` 一起作为 HIR output type context |

主要 downstream 读取面：`crates/scoopc/src/pipeline/hir_completeness.rs`、`crates/scoopc/src/pipeline/hir_stage.rs`、`crates/scoopc/src/mir/lower/mir_lowering_facts.rs`、`crates/scoopc/src/mir/lower/entry.rs`、`crates/scoopc/src/mir/materialize/{entry.rs,inputs.rs,hir_calls.rs}`、`crates/scoopc/src/pipeline/{mir_stage.rs,effect_facts_stage.rs,effect_lowering_stage.rs}`、`crates/scoopc/src/program_facts.rs`、`crates/scoopc/src/effect/state_machine/analysis/*`、`crates/scoopc/src/rtti/type_desc.rs`、`crates/scoopc/src/cone/scoopir/export.rs`、`crates/scoopc/src/llvm/{emit.rs,reachability.rs,codegen/**}`。

### `TypedHirEffectContracts` 当前字段与使用点

`TypedHirEffectContracts` 定义在 `crates/scoopc/src/pipeline/hir_stage.rs:867-880`，由 `TypedHirEffectContracts::from_lowered_hir(...)` / `ContractCollector` 从 `LoweredHir` 重新收集，并在 `TypedHirStageOutput::new_checked(...)` 中生成。

| 字段 | 当前职责 | 主要读取点 | P2 处理方向 |
| --- | --- | --- | --- |
| `function_effects` | 函数 allowed effect row contract | HIR dump/tests、preflight | 归入 `hir_facts` 的 callable/function contract |
| `continuation_resume_sites` | `Continuation.resume` typed contract | HIR preflight、MIR lowering facts | 归入 `hir_facts` source-site contract |
| `perform_sites` | `perform` typed contract | HIR preflight、MIR lowering facts | 归入 `hir_facts` source-site contract |
| `handle_sites` | `handle` typed contract | HIR preflight、MIR lowering facts | 归入 `hir_facts` source-site contract |
| `call_site_kinds` | call site category | HIR dump/tests | 与 call-site contract 合并为 `hir_facts` query |
| `call_site_contracts` | direct/member/virtual/interface/ctor/closure/funptr/intrinsic/effect call contracts | HIR preflight、MIR lowering facts | 归入唯一 source-site `hir_facts` |
| `with_update_contracts` | copy/update contract clone | HIR preflight、MIR lowering facts | 停止从 `LoweredHir` 复制，归入唯一 `hir_facts` |
| `assign_place_contracts` | assignment place contract clone | HIR preflight、MIR lowering facts | 停止从 `LoweredHir` 复制，归入唯一 `hir_facts` |
| `top_level_init_roots` | top-level init root contracts | HIR preflight、MIR lowering facts | 归入 global init/root `hir_facts` |
| `extern_global_contracts` | extern global contracts | HIR preflight、MIR lowering facts | 归入 global/extern `hir_facts` |

主要使用点：`crates/scoopc/src/pipeline/hir_stage.rs` 的 stable dump 与单测、`crates/scoopc/src/pipeline/hir_preflight.rs`、`crates/scoopc/src/pipeline/mir_stage.rs:218-223`、`crates/scoopc/src/mir/lower/mir_lowering_facts.rs:39-71,272-360`、`crates/scoopc/src/pipeline/effect_facts_stage.rs` 的测试 scaffolding。当前 `MirStageOutput` 仍把 `TypedHirEffectContracts` 作为字段继续暴露，这是 P2/P3 边界需要清理的直接证据。

### `ProgramFacts` 当前字段与使用点

`ProgramFacts` 定义在 `crates/scoopc/src/program_facts.rs:18-31`，由 `ProgramFacts::from_lowered(...)` 从 `LoweredHir` side tables 复制事实。

| 字段 | 当前职责 | 主要读取点 | P2 处理方向 |
| --- | --- | --- | --- |
| `ctor_call_targets` | ctor call target index | `ExprFactResolver` / LLVM / effect analysis | 归入 `hir_facts` source-site ctor contract |
| `continuation_resume_call_sites` | resume sites | effect analysis | 归入 `hir_facts` continuation contract |
| `non_pure_continuation_resume_call_sites` | outward resume sites | effect analysis | 归入 `hir_facts` continuation contract |
| `top_level_value_tys` | top-level var/val type lookup | `ExprFactResolver` | 归入 declaration/global root `hir_facts` |
| `fun_return_tys` | callable return type lookup | `ExprFactResolver` | 归入 callable declaration `hir_facts` |
| `object_property_tys` | object property type lookup | `ExprFactResolver` | 归入 object declaration `hir_facts` |
| `struct_field_tys` | struct field type lookup | `ExprFactResolver` | 归入 nominal/entity `hir_facts` |
| `class_field_tys` | class field type lookup | `ExprFactResolver` | 归入 nominal/entity `hir_facts` |
| `class_super_keys` | class super lookup | `ExprFactResolver` | 归入 nominal/entity `hir_facts` |
| `object_value_fqns` | object value FQN set | effect analysis | 归入 global object `hir_facts` |
| `object_property_fqns` | object property FQN set | effect analysis | 归入 object declaration `hir_facts` |
| `top_level_immutable_value_fqns` | top-level immutable roots | effect analysis | 归入 top-level init/root `hir_facts` |

主要使用点：`crates/scoopc/src/llvm/emit.rs:486` 构造并注入 LLVM codegen；`crates/scoopc/src/llvm/codegen/mod.rs:508,824` 持有；`crates/scoopc/src/expr_facts.rs` 通过 `ExprFactResolver` 查询；`crates/scoopc/src/effect/analysis.rs` 和 `crates/scoopc/src/effect/state_machine/analysis/*` 消费；`crates/scoopc/src/llvm/codegen/main/call.rs` 也通过 `ExprFactResolver` 读取。P2 结束时不得继续让 `ProgramFacts` 与 `hir_facts` 并行回答同一问题。

### declaration legality 与 HIR barrier 现状

- `@CallingConvention` generic 函数当前在 `crates/scoopc/src/typecheck/annotations.rs:2628-2649` 拒绝，属于 HIR barrier 内已有检查，P2 需把测试与文档归入正式 barrier gate。
- top-level `var` 必须显式 `@ThreadLocal` 或 `@Global` 的错误定义在 `crates/scoopc/src/typecheck/annotations.rs:432-438`，检查入口在 `crates/scoopc/src/typecheck/annotations.rs:791-805`。
- `object` 语法当前没有 type params；top-level `val` / `var` AST 也没有 type params，但 P2 仍需在 global-root facts 与 fixtures 中冻结“global roots monomorphic”规则，防止后续语法或 lowering 路径绕过该约束。
- `crates/scoopc/src/pipeline/hir_preflight.rs` 已有 typed HIR completeness 与 MIR smoke 方向，但仍依赖 `TypedHirEffectContracts`，且不能证明所有后续用户可见错误都已前移。

## [DONE] TODO-3-INIT：初始化并细化本任务包

- 目标：
  - 分析 `PLAN.md` §4/P2、`PIPELINE_REFACTOR.md` 和当前 HIR/typecheck/effect contract 相关代码的真实职责分布；
  - 生成本任务包的详细任务列表，覆盖 HIR semantic barrier、`hir_facts` 拆分、declaration legality gate 和后续阶段错误边界；
  - 更新 `TODO.md` 的具体任务索引，用新生成的任务替换或扩展 `TODO-3-INIT` 所在索引行。
- 必须实现的内容：
  1. 列出 `LoweredHir` 当前字段、side tables、构造点和下游读取点。
  2. 列出 `TypedHirEffectContracts`、`ProgramFacts` 与 HIR facts 候选项的当前使用点。
  3. 把 P2 拆成数量适中的实现小阶段，每个阶段必须有明确目标、修改范围、验证命令和完成条件。
  4. 在每个实现小阶段后插入独立 review 任务，review 任务必须复审前一阶段是否满足 HIR barrier 约束。
  5. 同步更新 `TODO.md` 中的具体任务索引，确保任务 ID、状态和顺序与本文件一致。
- 完成条件：
  - `TODO-3.md` 不再只是范围说明，而是包含可直接执行的详细任务列表；
  - `TODO.md` 的具体任务索引已经同步反映 `TODO-3.md` 的新任务和 `[TODO]` 状态；
  - 本任务完成记录说明 HIR facts 拆分依据和仍需实现阶段验证的风险。
- 依赖：P1-T06R
- 完成记录：
  - 拆分依据：P2 的执行顺序由当前耦合方向决定，先建立独立 `hir_facts` crate/data product，再固定 `HirStageOutput = { hir, hir_facts }`，随后移除 HIR 反向携带 MIR 的路径，最后分别迁移 declaration/entity facts、source-site typed contracts、declaration legality gate 和全包清场。
  - 触碰面记录：已在本文件“触碰面基线”中记录 `LoweredHir` 字段/side tables/构造点/下游读取点，`TypedHirEffectContracts` 字段和使用点，`ProgramFacts` 字段和使用点，以及 declaration legality 的现有检查位置。
  - 任务结构：新增 7 个实现阶段和 7 个 review 阶段：`hir_facts` crate 与数据模型、HIR stage output 形状、移除 HIR->MIR reverse materialization、declaration/entity facts 迁移、source-site typed contracts 迁移、semantic barrier legality gate、P2 全包清场。
  - 核心决策：`TypedHirEffectContracts` 与 `ProgramFacts` 不作为长期并列 fact source；二者当前承载的信息必须迁入 `hir_facts` 或被重新归属到后续阶段 facts。`LoweredHir::materialized_mir` 是 P2 必须移出的多阶段 bundle 证据，不能继续作为 HIR 输出的合法组成部分。
  - 未展开风险：production frontend 当前仍一次处理 build closure；P2 的目标是固定 cone-level HIR barrier 与 facts handoff，但逐 cone 编译 orchestration 的彻底物理拆分若超出 HIR/facts 边界，应作为后续阶段或新增前置任务明确记录。LLVM backend 仍大量回看 HIR scaffold，P2 只负责让 facts owner 唯一，不提前承诺完成 P7 backend cleanup。
  - 验证命令：文档/计划任务仅需检查 markdown/TODO 一致性；本次执行使用 `git diff --check`。

## [DONE] P2-T01：建立 `hir_facts` crate 与事实数据模型

- 参考：
  - `PLAN.md` §1.2、§1.3、§4/P2
  - `PIPELINE_REFACTOR.md` “fact crate 必须自包含”“HIR stage”
  - 本文件“触碰面基线”
- 目标：
  - 在 workspace 中加入独立 `scoopc_hir_facts` 数据产品；
  - 定义 `HirFacts` 的顶层结构、dump/verifier 入口和初始模块边界；
  - 固定 fact crate 只能依赖基础 crate 的 DAG 门禁。
- 必须检查和修改的主要位置：
  - `Cargo.toml`
  - `crates/scoopc/Cargo.toml`
  - `crates/scoopc_hir_facts/`
  - `tools/scoop_tools` 的 dependency gate
  - `README.md` 的 crate 概览
- 必须实现的内容：
  1. 新建 `scoopc_hir_facts` crate，至少包含 crate-level 职责文档、`#![forbid(unsafe_code)]`、`HirFacts` 顶层类型、空 verifier/dump skeleton 和单元测试。
  2. 将 `HirFacts` 先划分为 declaration/entity facts、source-site typed contracts、global root/init facts、native/extern facts、type context reference 这几组模块或子结构。
  3. 只使用 `scoopc_span`、`scoopc_source`、`scoopc_types`、`scoopc_ids`、`scoopc_project_model` 中的基础类型表达 identity/type/span/cone 信息；不得引用 `crate::hir`、`crate::ast`、`crate::mir` 或 `scoopc` facade 类型。
  4. 更新依赖门禁，使 `scoopc_hir_facts` 作为 fact crate 被检查，拒绝依赖 stage/facade/其它 fact crate。
  5. 在 `scoopc` facade 中只添加必要依赖或 re-export anchor，不迁移业务事实内容。
- 禁止事项：
  - 禁止把 HIR node、AST node、MIR node 或 backend ABI 类型放入 `scoopc_hir_facts`。
  - 禁止为了快速迁移而让 `scoopc_hir_facts` 依赖 `scoopc` facade。
  - 禁止复制一套 `TypeStore`、`Span`、`SourceId` 或 cone identity。
- 验证：
  1. `cargo fmt`
  2. `cargo check -p scoopc_hir_facts`
  3. `cargo test -p scoopc_hir_facts`
  4. `cargo run -p scoop_tools -- dependency-gate`
  5. `cargo clippy --all-targets -- -D warnings`
- 完成条件：
  - `scoopc_hir_facts` 可独立编译和测试；
  - dependency gate 能证明该 fact crate 未依赖 `scoopc`、stage crate、backend crate 或其它 fact crate；
  - `HirFacts` 模型已能承接后续迁移任务的事实分类。
- 依赖：TODO-3-INIT
- 完成记录：
  - 改动范围：新增 workspace crate `crates/scoopc_hir_facts`，包含 crate-level 职责文档、`#![forbid(unsafe_code)]`、`HirFacts` 顶层数据产品、fact group 模块、dump/verifier 入口和单元测试；同步接入 workspace、`scoopc` facade re-export anchor、README crate 概览和 `scoop_tools dependency-gate`。
  - 事实模型：`HirFacts` 初始划分为 declaration/entity facts、source-site typed contracts、global root/init facts、native/extern facts、type context reference/source cone ownership 五组；数据结构只引用 `scoopc_span`、`scoopc_source`、`scoopc_types`、`scoopc_ids`、`scoopc_project_model` 的基础类型，不承载 HIR/AST/MIR/backend ABI 节点。
  - 依赖门禁：dependency gate 现在同时检查 5 个 base crate 与 1 个 fact crate；`scoopc_hir_facts` 被归类为 fact crate，允许依赖基础 crate，拒绝依赖 `scoopc` facade、stage/backend/tool/runtime crate 或其它 fact crate。
  - 核心决策：type context 在 `scoopc_hir_facts` 中只以 `TypeContextReference` 引用 HIR-owned `TypeStore`，不复制 `TypeStore`；`scoopc` 只新增依赖与 `hir_facts` re-export anchor，不迁移业务事实内容。
  - 验证命令：`cargo fmt`；`cargo check -p scoopc_hir_facts`；`cargo test -p scoopc_hir_facts`；`cargo run -p scoop_tools -- dependency-gate`；`cargo test -p scoop_tools dependency_gate`；`cargo clippy --all-targets -- -D warnings`。
  - 残余风险：当前为 facts skeleton，尚未接入 HIR stage output，也尚未迁移 `TypedHirEffectContracts` / `ProgramFacts` 的实际内容；这些分别由后续 P2-T02、P2-T04、P2-T05 处理。

## [DONE] P2-T01R：Review `hir_facts` crate 与事实模型

- 参考：P2-T01。
- 重点：
  - `scoopc_hir_facts` 是否满足 fact crate 自包含约束；
  - `HirFacts` 分类是否覆盖本文件触碰面基线中的候选事实；
  - dependency gate 是否能阻止 fact crate 依赖 `scoopc` 或其它 stage/fact crate。
- 必须复查的范围：
  - `Cargo.toml`
  - `crates/scoopc_hir_facts/`
  - `crates/scoopc/Cargo.toml`
  - `tools/scoop_tools`
  - `README.md`
- 验证：
  - 重新运行 P2-T01 的所有验证；
  - 额外运行 `cargo tree -p scoopc_hir_facts`，确认只出现允许的基础依赖。
- 完成条件：
  - review 结论明确写出：`hir_facts` crate 壳层满足 P2/Pipeline fact DAG 约束，或列出阻塞项并在本 review 内修复。
- 依赖：P2-T01
- 完成记录：
  - 复查范围：已复查 workspace 成员、`crates/scoopc_hir_facts/` 模型和测试、`crates/scoopc/Cargo.toml` 与 `scoopc::hir_facts` facade anchor、`tools/scoop_tools` dependency gate、`README.md` crate 概览。
  - 事实模型结论：`HirFacts` 顶层结构按 declaration/entity、source-site typed contracts、global root/init、native/extern、type context reference/source cone ownership 五组划分，覆盖本文件触碰面基线中后续迁移需要承接的事实分类；当前仍是 skeleton，不迁移业务事实内容，符合 P2-T01 范围。
  - 依赖结论：`scoopc_hir_facts` 直接依赖仅包含 `scoopc_ids`、`scoopc_project_model`、`scoopc_source`、`scoopc_span`、`scoopc_types`；未依赖 `scoopc` facade、stage/backend crate 或其它 fact crate。`cargo tree -p scoopc_hir_facts` 显示 workspace 依赖只出现在允许的基础 crate 集合中，外部依赖均来自基础 crate 的传递依赖。
  - dependency gate 结论：`scoop_tools dependency-gate` 已把 `scoopc_hir_facts` 归为 fact crate 并拒绝 facade/其它 fact crate 依赖；对应单元测试覆盖允许基础依赖、拒绝 `scoopc` facade 和拒绝其它 fact crate。
  - 修复情况：review 未发现需要在本任务内修复的阻塞项；未修改实现代码。
  - 验证命令：`cargo fmt`；`cargo check -p scoopc_hir_facts`；`cargo test -p scoopc_hir_facts`；`cargo run -p scoop_tools -- dependency-gate`；`cargo test -p scoop_tools dependency_gate`；`cargo tree -p scoopc_hir_facts`；`cargo clippy --all-targets -- -D warnings`。
  - 残余风险：`hir_facts` 仍未接入正式 HIR stage output，也未迁移 `TypedHirEffectContracts` / `ProgramFacts` 的实际内容；这些继续由 P2-T02、P2-T04、P2-T05 跟进。

## [DONE] P2-T02：固定 `HirStageOutput = { hir, hir_facts }` 输出形状

- 参考：
  - `PIPELINE_REFACTOR.md` “stage output wrapper 规则”“HIR stage”
  - `crates/scoopc/src/pipeline/hir_stage.rs`
  - `crates/scoopc/src/pipeline/hir_preflight.rs`
- 目标：
  - 将现有 `TypedHirStageOutput` 收口为正式 HIR stage handoff，输出 HIR 本体、`HirFacts` 和必要 type/context；
  - 让 dump/preflight/pipeline helper 优先消费正式 HIR stage output；
  - 为后续迁移 facts 内容建立单一入口。
- 必须检查和修改的主要位置：
  - `crates/scoopc/src/pipeline/hir_stage.rs`
  - `crates/scoopc/src/pipeline/hir_preflight.rs`
  - `crates/scoopc/src/pipeline/mod.rs`
  - `crates/scoopc/src/driver_cli.rs`
  - `tests/fixtures/hir/`、`tests/fixtures/typecheck/` 中依赖 `dump-hir` 输出的 fixture
- 必须实现的内容：
  1. 引入正式 `HirStageOutput` 或等价命名，使公开语义为 `{ hir, hir_facts }`，旧 `TypedHirStageOutput` 只能作为迁移期 type alias/adapter 或被移除。
  2. 将当前 `TypedHirEffectContracts` 的生成入口接到 `HirFacts` skeleton 上，允许内容仍为空或桥接，但下游只能通过 `hir_facts()` 访问正式 facts 入口。
  3. 更新 HIR stable dump，使 HIR 本体和 facts dump 边界清晰，避免继续把 typed side tables 描述成临时附录。
  4. 更新 `hir_preflight`，让它检查 `HirFacts` 必备合同，而不是直接检查 `TypedHirEffectContracts`。
  5. 保持现有 HIR/MIR dump fixture 行为稳定，除非输出名称变化是本任务明确要求并同步更新 fixture。
- 禁止事项：
  - 禁止在 `HirStageOutput` 中嵌套 MIR/effect/LIR/codegen stage output。
  - 禁止让下游绕过 `hir_facts()` 继续直接依赖 `TypedHirEffectContracts` 作为正式 API。
  - 禁止把 build closure flatten 描述为一个 HIR compilation unit。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features hir_stage`
  3. `cargo test -p scoopc --no-default-features hir_preflight`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/hir`
  5. `cargo clippy --all-targets -- -D warnings`
- 完成条件：
  - HIR stage 对外输出形状已经明确为 HIR + `hir_facts`；
  - preflight 和 dump 入口均从正式 HIR stage output 读取；
  - 旧 typed contracts 名称不再是新的任务入口。
- 依赖：P2-T01R
- 完成记录：
  - 改动范围：将公开 HIR stage handoff 收口为 `HirStageOutput`，新增 `hir_facts()` 正式入口；`load_hir_stage_output_for_dump`、`dump-hir`、fixture runner、MIR/LLVM stage helper 和相关测试均改为消费新的 HIR stage output 名称。
  - facts 入口：`scoopc_hir_facts::HirFacts` 新增 `contract_bridge` 覆盖计数，HIR stage 在生成现有 typed contract bridge 后同步构建并验证 `HirFacts`，同时发布 `type_context` 引用，避免 dump/preflight 继续把 typed side tables 当作唯一事实入口。
  - dump/preflight：HIR stable dump 现在明确分为 HIR 本体、`hir_facts { ... }` 和迁移期 `typed_contract_bridge { ... }`；`hir_preflight` 的必备合同检查改为读取 `output.hir_facts().contract_bridge`。
  - 核心决策：`TypedHirEffectContracts` 仍作为 P2 迁移 bridge 保留给 MIR lowering 和内部测试，但不再作为公开 HIR stage output 类型或 dump 主标题；实际 declaration/entity/source-site facts 的逐项迁移继续由 P2-T04/P2-T05 完成。
  - 验证命令：`cargo fmt`；`cargo test -p scoopc_hir_facts`；`cargo test -p scoopc --no-default-features hir_stage`；`cargo test -p scoopc --no-default-features hir_preflight`；`cargo run -p scoop -- test --fixtures tests/fixtures/hir`；`cargo clippy --all-targets -- -D warnings`。
  - 残余风险：`contract_bridge` 目前只提供迁移期覆盖计数，不承载完整 source-site/declaration fact payload；后续任务必须继续把 `TypedHirEffectContracts` / `ProgramFacts` 的实际内容迁入 `HirFacts`，并最终移除 bridge API。

## [DONE] P2-T02R：Review HIR stage output 形状

- 参考：P2-T02。
- 重点：
  - stage output 是否只暴露 HIR 本体、HIR facts 和必要 type/context；
  - dump/preflight 是否已经以 `hir_facts` 为检查入口；
  - 是否留下会误导后续任务继续消费 `TypedHirEffectContracts` 的公开 API。
- 必须复查的范围：
  - `crates/scoopc/src/pipeline/hir_stage.rs`
  - `crates/scoopc/src/pipeline/hir_preflight.rs`
  - `crates/scoopc/src/pipeline/mod.rs`
  - HIR fixture 输出
- 验证：
  - 重新运行 P2-T02 的所有验证；
  - 额外搜索 `TypedHirStageOutput|TypedHirEffectContracts|hir_facts\(`，确认旧名称只作为迁移期 adapter 或测试说明存在。
- 完成条件：
  - review 结论明确写出：`HirStageOutput = { hir, hir_facts }` 外形成立，或列出阻塞项并在本 review 内修复。
- 依赖：P2-T02
- 完成记录：
  - 复查范围：已复查 `crates/scoopc/src/pipeline/hir_stage.rs`、`hir_preflight.rs`、`pipeline/mod.rs`、`dump-hir` 调用点和 HIR fixture 输出。
  - review 结论：`HirStageOutput = { hir, hir_facts }` 的公开外形成立；外部入口暴露 `hir_file()`、`types()`、`hir_facts()`、`source_path()` 与 `stable_dump()`，旧 `TypedHirStageOutput` 在 Rust 代码中已无引用。
  - dump/preflight 结论：HIR fixtures 均包含明确的 `hir_facts { ... }` 边界和迁移期 `typed_contract_bridge { ... }` 边界；`hir_preflight` 的 typed contract 必备项检查读取 `output.hir_facts().contract_bridge`，不再直接以 `TypedHirEffectContracts` 为检查入口。
  - 修复情况：将 `TypedHirEffectContracts` 收紧为 `pub(crate)` 迁移 bridge，并修正文档注释，明确它只服务 crate 内部 MIR lowering adapter，直到 P2-T05 将完整 source-site contract payload 迁入 `scoopc_hir_facts`。
  - 搜索结论：`TypedHirEffectContracts` 的 Rust 代码引用仅剩 HIR stage 内部 bridge 构造/dump、MIR lowering adapter、MIR/effect stage 测试 scaffolding 和 crate 内 re-export；`hir_facts()` 引用集中在正式 stage output、preflight、dump/tests 中。
  - 验证命令：`cargo fmt`；`cargo test -p scoopc --no-default-features hir_stage`；`cargo test -p scoopc --no-default-features hir_preflight`；`cargo run -p scoop -- test --fixtures tests/fixtures/hir`；`cargo test -p scoopc_hir_facts`；`cargo clippy --all-targets -- -D warnings`；搜索 `TypedHirStageOutput|TypedHirEffectContracts|hir_facts\(` 和 HIR fixtures 中的 `hir_facts {|typed_contract_bridge {`。
  - 残余风险：`typed_contract_bridge` 仍是 P2 迁移期 payload，不承载最终 source-site/declaration fact 结构；其内容迁移和最终 bridge 清理由 P2-T04/P2-T05 继续处理。

## [DONE] P2-T03：移除 HIR 反向携带 MIR materialization

- 参考：
  - `PLAN.md` §1.1、§4/P2
  - `PIPELINE-CLEANUP.md` P1/P2/P6/P7
  - 本文件触碰面中 `LoweredHir::materialized_mir`
- 目标：
  - 让 HIR lowering 不再先运行 MIR materialization；
  - 从 `LoweredHir` 移除 `materialized_mir`、`materialized_*` accessors 和 clone-without-materialized workaround；
  - 将 request-root instance collection / canonical materialized MIR handoff 归还给 MIR stage 或 pipeline orchestration。
- 必须检查和修改的主要位置：
  - `crates/scoopc/src/hir/lower/types.rs`
  - `crates/scoopc/src/hir/lower/main/compilation_unit.rs`
  - `crates/scoopc/src/frontend.rs`
  - `crates/scoopc/src/pipeline/mir_stage.rs`
  - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`
  - `crates/scoopc/src/pipeline/effect_facts_stage.rs`
  - `crates/scoopc/src/pipeline/effect_lowering_stage.rs`
  - `crates/scoopc/src/llvm/emit.rs`
- 必须实现的内容：
  1. 删除 `LoweredHir` 中的 `materialized_mir` 字段和所有从 HIR 输出读取 MIR/pass view 的 accessor。
  2. 将 `lower_for_compilation_unit_multi_files_via_mir_instance_collection*` 的职责拆开：HIR lowering 只产出 HIR；MIR stage/pipeline 负责根据 request roots materialize canonical MIR。
  3. 更新 production build/run pipeline，使 codegen 继续获得必要 MIR/effect/LIR handoff，但不再经由 `LoweredHir` 传递。
  4. 删除或重写 `clone_hir_compat_scaffold_without_materialized_mir` 这类承认混合对象的 workaround。
  5. 更新 tests/fixtures，证明合法 HIR output 之后 MIR stage 才生成 canonical materialized snapshot。
- 禁止事项：
  - 禁止引入新的 `HirStageOutput { mir: ... }`、`HirFacts { materialized_mir: ... }` 或等价嵌套。
  - 禁止通过重新 materialize 一次来掩盖 stage ordering 错误；若需要 materialization，必须由 MIR stage 明确拥有。
  - 禁止缩窄 fixture 或绕开 production codegen 路径。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features hir_stage`
  3. `cargo test -p scoopc --no-default-features mir_stage`
  4. `cargo test --all --all-targets --no-default-features`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  6. `cargo clippy --all-targets -- -D warnings`
  7. 搜索 `materialized_mir|materialized_pass_view|clone_hir_compat_scaffold_without_materialized_mir`，确认不再出现在 HIR output API 中。
- 完成条件：
  - HIR lowering 不再依赖 MIR materialization；
  - `LoweredHir` 不再携带 MIR/pass artifacts；
  - production codegen 的 MIR/effect/LIR 输入来自 MIR 之后的 stage handoff，而不是 HIR bundle。
- 依赖：P2-T02R
- 完成记录：
  - 改动范围：从 `LoweredHir` 删除 `materialized_mir` 字段、`materialized_mir()` / `materialized_callable_view()` / `materialized_pass_view()` / `materialized_mir_mut()` / `into_materialized_mir()` accessor，以及 `clone_hir_compat_scaffold_without_materialized_mir()` workaround；`LoweredHir` 现在只作为 HIR 本体、type context 与迁移期 HIR side table 输出。
  - 责任拆分：删除 HIR lowering 内部的 `lower_for_compilation_unit_multi_files_via_mir_instance_collection*` materialization 入口，改为先由 frontend/pipeline 显式调用 MIR materializer 得到 canonical `MaterializedMir`，再通过 `lower_for_compilation_unit_multi_files_with_explicit_mir_instances(...)` 构造 HIR compatibility scaffold。该 HIR helper 只消费已收集的 `InstanceKey` / `TypeStore`，不运行 MIR materialization，也不保存 MIR/pass artifacts。
  - production handoff：新增 `frontend::CodegenLoweringOutput = { lowered_hir, materialized_mir }`，`LlvmCodegenStageInput` 直接接收该 handoff；LLVM stage 先把 `lowered_hir` 推进 `HirStageOutput -> MirStageOutput`，再把独立 `materialized_mir` 显式挂到 MIR stage output，之后 effect facts / effect lowering / LLVM emit 只从 post-MIR stage handoff 读取 canonical pass view。
  - fixture 覆盖：新增 `tests/fixtures/run-pass/stage_handoff_generic_materialization.scoop`，覆盖 top-level generic 与 member generic 在 HIR compatibility scaffold 与 MIR-owned materialized snapshot 分离后仍能经 production build/run 输出 `42`。
  - 搜索结论：`materialized_mir|materialized_pass_view|clone_hir_compat_scaffold_without_materialized_mir` 在 `crates/scoopc/src/hir/**/*.rs` 中已无命中；`lower_for_compilation_unit_multi_files_via_mir_instance_collection|MirInstanceCollectionOptions|clone_hir_compat_scaffold_without_materialized_mir|into_materialized_mir` 在 `crates/scoopc/src/**/*.rs` 中已无命中。
  - 验证命令：`cargo fmt`；`cargo test -p scoopc --no-default-features hir_stage`；`cargo test -p scoopc --no-default-features mir_stage`；`cargo test -p scoopc hir_stage`；`cargo test -p scoopc mir_stage`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/stage_handoff_generic_materialization.scoop`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`；`cargo test --all --all-targets --no-default-features --quiet`；`cargo clippy --all-targets -- -D warnings`；上述搜索命令。
  - 残余风险：LLVM backend 仍保留 HIR compatibility scaffold 读取 declaration/layout 等非 effect side table；这不再携带 MIR/pass artifacts，但 declaration/entity facts 迁移仍由 P2-T04 继续处理，source-site contract bridge 清理由 P2-T05 继续处理。

## [DONE] P2-T03R：Review HIR/MIR 单向边界

- 参考：P2-T03。
- 重点：
  - 是否还存在 HIR lowering 调用 MIR materializer 的路径；
  - 是否还存在从 HIR output accessor 取 MIR/pass view 的路径；
  - production build/run 是否仍覆盖 canonical MIR handoff。
- 必须复查的范围：
  - `crates/scoopc/src/hir/lower/`
  - `crates/scoopc/src/frontend.rs`
  - `crates/scoopc/src/pipeline/`
  - `crates/scoopc/src/llvm/`
- 验证：
  - 重新运行 P2-T03 的所有验证；
  - 额外运行至少一个 build fixture 和一个 run-pass fixture，确认 production codegen 没有退化。
- 完成条件：
  - review 结论明确写出：`AST/HIR -> MIR` 单向边界在代码路径上成立，或列出阻塞项并在本 review 内修复。
- 依赖：P2-T03
- 完成记录：
  - 复查范围：已复查 `crates/scoopc/src/hir/lower/`、`crates/scoopc/src/hir/mod.rs`、`crates/scoopc/src/pipeline/hir_stage.rs`、`frontend.rs`、`pipeline/{mir_stage,effect_facts_stage,effect_lowering_stage,llvm_codegen_stage}.rs`、`llvm/{emit,frontend}.rs` 与 build/run production 入口。
  - review 结论：`AST/HIR -> MIR` 单向边界在代码路径上成立。`LoweredHir` 和 `HirStageOutput` 不再携带 `MaterializedMir`、pass view 或 materialized accessor；HIR lowering 中未发现调用 MIR materializer 的路径。
  - handoff 结论：production codegen 先由 frontend/project pipeline 生成分离的 `CodegenLoweringOutput = { lowered_hir, materialized_mir }`，LLVM stage 再把 `materialized_mir` 显式挂到 `MirStageOutput`，effect facts / late lowering / LLVM emit 均从 post-MIR stage handoff 读取 canonical pass view，而不是从 HIR bundle 回取。
  - 搜索结论：`materialized_mir|materialized_pass_view|clone_hir_compat_scaffold_without_materialized_mir` 在 `crates/scoopc/src/hir` 中无命中；`materialized_mir|materialized_pass_view|MaterializedMir|MaterializedMirPassView` 在 `pipeline/hir_stage.rs` 中无命中；`lower_for_compilation_unit_multi_files_via_mir_instance_collection|MirInstanceCollectionOptions|into_materialized_mir` 在 `crates/scoopc/src` 中无命中；`materialize_compilation_unit_from_typechecked_inputs|materialize_for_dump|MaterializedMir|MaterializedMirPassView|pass_view\(` 在 `crates/scoopc/src/hir/lower` 中无命中。
  - 注意项：`hir/lower/main/tests.rs` 仍有仅测试辅助函数名 `lower_typed_single_source_file_via_mir_instance_collection`，它通过 frontend 获取分离 handoff 后只返回 `lowered_hir`；该路径不是 HIR lowering API，也不把 MIR/pass artifact 挂回 HIR 输出。
  - 验证命令：`cargo fmt`；`cargo test -p scoopc --no-default-features hir_stage`；`cargo test -p scoopc --no-default-features mir_stage`；`cargo test --all --all-targets --no-default-features`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`；`cargo run -p scoop -- test --fixtures tests/fixtures/build/emit_llvm_basic.scoop`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/stage_handoff_generic_materialization.scoop`；`cargo clippy --all-targets -- -D warnings`；上述边界搜索。
  - 残余风险：P2-T04/P2-T05 仍需迁移 declaration/entity facts 与 source-site typed contracts；当前 review 未发现与 P2-T03R 相关的阻塞项。

## [DONE] P2-T04：迁移 declaration/entity facts 并收口 `ProgramFacts`

- 参考：
  - 本文件 `LoweredHir` 和 `ProgramFacts` 字段清单
  - `PIPELINE-CLEANUP.md` P16/P18
  - `PIPELINE_REFACTOR.md` “HIR stage / 应发布的 facts”
- 目标：
  - 将 declaration/entity/global/native facts 从 `LoweredHir` side tables 和 `ProgramFacts` 迁入 `HirFacts`；
  - 让 `ProgramFacts` 不再与 `hir_facts` 并行回答同一问题；
  - 下游需要声明/实体事实时显式消费 `HirFacts`。
- 必须检查和修改的主要位置：
  - `crates/scoopc/src/program_facts.rs`
  - `crates/scoopc/src/expr_facts.rs`
  - `crates/scoopc/src/effect/analysis.rs`
  - `crates/scoopc/src/effect/state_machine/analysis/`
  - `crates/scoopc/src/llvm/emit.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - `crates/scoopc/src/mir/lower/mir_lowering_facts.rs`
  - `crates/scoopc/src/rtti/type_desc.rs`
- 必须实现的内容：
  1. 在 `HirFacts` 中发布 nominal kind/variance/direct supertypes、struct/enum/class/object/interface metadata、native/extern metadata、top-level roots、storage policy、source cone ownership 等 declaration/entity facts。
  2. 将 `ProgramFacts::from_lowered(...)` 当前复制的 type/field/object/root 信息迁移到 `HirFacts` query；删除 `ProgramFacts` 或把它降级为仅无重叠的临时 adapter，并在完成记录中说明清除条件。
  3. 更新 `ExprFactResolver`、effect analysis 和 LLVM codegen，使它们从 `HirFacts` 读取 declaration/entity facts，不再同时持有 `LoweredHir` side table 与 `ProgramFacts` 的重复答案。
  4. 将仍必须留在 `LoweredHir` 的内容限定为 HIR IR 本体或 HIR 内部 helper，并写明原因。
  5. 增加/更新 tests，覆盖 object/top-level/class/struct field type 查询、direct supertypes、native/extern metadata、source cone ownership。
- 禁止事项：
  - 禁止保留 `ProgramFacts` 与 `HirFacts` 两套可替代查询面。
  - 禁止把 HIR node 或 backend ABI 物理类型放入 `scoopc_hir_facts`。
  - 禁止为了减少改动让 LLVM codegen 继续从 `LoweredHir` 现场重建同一 declaration fact。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc_hir_facts`
  3. `cargo test -p scoopc --no-default-features expr_facts`
  4. `cargo test -p scoopc --no-default-features effect`
  5. `cargo test --all --all-targets --no-default-features`
  6. `cargo clippy --all-targets -- -D warnings`
  7. 搜索 `ProgramFacts::from_lowered|program_facts\.|LoweredHir.*struct_layouts|LoweredHir.*object_inits`，分类确认没有重复 authoritative 查询面。
- 完成条件：
  - declaration/entity/global/native facts 的 owner 是 `HirFacts`；
  - `ProgramFacts` 已删除或不再与 `HirFacts` 重叠；
  - downstream declaration/entity 查询不再直接依赖 `LoweredHir` side tables 作为事实来源。
- 依赖：P2-T03R
- 完成记录：
  - 改动范围：扩展 `scoopc_hir_facts` declaration/global/native/type-context 模型，HIR stage 现在发布 nominal/direct-supertype、callable、field/object property、enum variant、top-level root/storage、object/class initializer、extern/native、extern library、stable type-param 与 source-cone ownership facts。
  - `ProgramFacts` 收口：删除 `crates/scoopc/src/program_facts.rs`，新增仅承载 P2-T05 尚未迁移 call-site facts 的 `SourceSiteMigrationFacts` 临时 bridge；`ExprFactResolver`、effect/state-machine analysis 和 LLVM codegen 的 declaration/entity 查询改为消费 `HirFacts`。
  - 下游查询：`HirFactResolver` 现在从 `HirFacts` 回答 top-level value/object property/function return/struct field/class field/direct-supertype 递归等查询；LLVM stage output 和 emit handoff 显式携带 `HirFacts`，不再现场构造 `ProgramFacts`。
  - HIR 内部 helper 边界：`LoweredHir` 仍保留 HIR IR 本体、type store、body inventory，以及尚待 P2-T05/P3+ 迁移的 source-site/MIR lowering/codegen compatibility side tables；declaration/entity authoritative query surface 已迁到 `HirFacts`。
  - 测试与 fixtures：新增 HIR stage 单测覆盖 struct/class/object field facts、direct supertypes、top-level roots/storage、extern global、extern function/native callable metadata 和 source-cone facts；同步更新 `tests/fixtures/hir/*.hir` 中的 `hir_facts` dump 计数。
  - 验证命令：`cargo fmt`；`cargo test -p scoopc_hir_facts`；`cargo test -p scoopc --no-default-features expr_facts`；`cargo test -p scoopc --no-default-features effect`；`cargo test -p scoopc --no-default-features hir_stage`；`cargo run -p scoop -- test --fixtures tests/fixtures/hir`；`cargo test --all --all-targets --no-default-features`；`cargo clippy --all-targets -- -D warnings`；搜索 `ProgramFacts::from_lowered|program_facts\.|LoweredHir.*struct_layouts|LoweredHir.*object_inits`。
  - 搜索结论：`ProgramFacts::from_lowered`、`program_facts.` 以及 `LoweredHir.*struct_layouts|LoweredHir.*object_inits` 在 Rust 源码中无命中；剩余 `TODO-3.md` 命中是历史任务描述与本验证项。
  - 残余风险：`SourceSiteMigrationFacts` 仍从 `LoweredHir` 复制 ctor/resume call-site facts，这是 P2-T05 的明确迁移范围；LLVM codegen 仍持有部分 layout/codegen compatibility side table 用于实际 lowering，不再作为 `ProgramFacts` 式 declaration/entity 查询 owner。

## [DONE] P2-T04R：Review declaration/entity facts 迁移结果

- 参考：P2-T04。
- 重点：
  - `HirFacts` 是否成为 declaration/entity facts 的唯一 owner；
  - `ProgramFacts` 是否已被删除或不再重叠；
  - LLVM/effect/expr facts 是否不再从 `LoweredHir` 复制同一事实。
- 必须复查的范围：
  - `crates/scoopc_hir_facts/`
  - `crates/scoopc/src/program_facts.rs`
  - `crates/scoopc/src/expr_facts.rs`
  - `crates/scoopc/src/effect/`
  - `crates/scoopc/src/llvm/`
  - `crates/scoopc/src/mir/lower/mir_lowering_facts.rs`
- 验证：
  - 重新运行 P2-T04 的所有验证；
  - 额外检查 `cargo tree -p scoopc_hir_facts` 和 fact crate dependency gate。
- 完成条件：
  - review 结论明确写出：declaration/entity facts 已收口到 `HirFacts`，或列出阻塞项并在本 review 内修复。
- 依赖：P2-T04
- 完成记录：
  - 复查范围：已复查 `crates/scoopc_hir_facts/`、已删除的 `crates/scoopc/src/program_facts.rs` 路径、`expr_facts.rs`、effect analysis、LLVM emit/codegen handoff、`mir/lower/mir_lowering_facts.rs`、HIR fixture dump 与 dependency gate。
  - review 结论：declaration/entity/global/native facts 的 authoritative query surface 已收口到 `HirFacts`；`ProgramFacts` / `program_facts` 在 Rust 源码中无命中；`ExprFactResolver`、effect shared analysis、MIR lowering 的 member/nominal/enum facts、LLVM effect-instance classification 与 top-level/extern type 查询均显式消费 `HirFacts`。
  - 修复情况：补齐 `HirFacts` dispatch/interface table 发布；将 `MirLoweringFacts` 的 member value type、nominal kind、enum payload kind 构造改为从 `HirFacts` 派生；将 LLVM codegen 的 effect nominal 收集和 top-level/extern value type/contract 查询改为走 `HirFactResolver`；修正 callable fact identity，使重载函数不会产生重复 fact key；同步 HIR fixture 中的 dispatch fact 计数；更新 internal-bug sentinel 审计期望。
  - 搜索结论：`ProgramFacts::from_lowered|program_facts\.`、`LoweredHir.*struct_layouts|LoweredHir.*object_inits`、`with_member_value_types|with_nominal_kinds|with_enum_payload_kinds` 均无命中；`DispatchTableFact|dispatch\.vtables|interface_tables` 只命中 fact 模型、dump 与 HIR stage 发布点。剩余 `lowered.(...)` 命中已分类为 HIR facts 构建、hidden-init effect body scan、LLVM/RTTI codegen compatibility side table 或测试 scaffolding，不再是 `ProgramFacts` 式 declaration/entity authoritative 查询面。
  - 依赖结论：`cargo tree -p scoopc_hir_facts` 显示 `scoopc_hir_facts` 仅依赖基础 crate 及其外部依赖；`scoop_tools dependency-gate` 通过，确认 fact crate 未依赖 `scoopc` facade、stage/backend crate 或其它 fact crate。
  - 验证命令：`cargo fmt`；`cargo test -p scoopc_hir_facts`；`cargo test -p scoopc --no-default-features expr_facts`；`cargo test -p scoopc --no-default-features effect`；`cargo test -p scoopc --no-default-features hir_stage`；`cargo test -p scoopc --no-default-features mir_stage`；`cargo run -p scoop -- test --fixtures tests/fixtures/hir`；`cargo test --all --all-targets --no-default-features`；`cargo run -p scoop_tools -- dependency-gate`；`cargo tree -p scoopc_hir_facts`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`；上述 review 搜索。
  - 残余风险：`SourceSiteMigrationFacts` 与 `TypedHirEffectContracts` 仍是 P2-T05 的 source-site contract 迁移范围；LLVM/RTTI 仍保留若干 HIR side table 作为实际 body/layout/codegen compatibility 输入，但 declaration/entity 查询 owner 不再是 `ProgramFacts` 或现场重建的 duplicate fact surface。

## [TODO] P2-T05：迁移 source-site typed contracts 并删除 fallback 双轨

- 参考：
  - 本文件 `TypedHirEffectContracts` 字段清单
  - `PIPELINE-CLEANUP.md` P16/P17
  - `crates/scoopc/src/mir/lower/mir_lowering_facts.rs`
- 目标：
  - 将 call-site、perform/resume/handle、arg binding、assignment/update、dispatch/ctor/when pattern 等 source-site typed contracts 迁入 `HirFacts`；
  - 删除 `MirLoweringFacts` 的 typed/fallback 双轨输入；
  - 让 MIR lowering 只接受一套完整 authoritative HIR facts。
- 必须检查和修改的主要位置：
  - `crates/scoopc/src/pipeline/hir_stage.rs`
  - `crates/scoopc/src/pipeline/hir_preflight.rs`
  - `crates/scoopc/src/mir/lower/mir_lowering_facts.rs`
  - `crates/scoopc/src/mir/lower/fn_lowering_call.rs`
  - `crates/scoopc/src/mir/lower/fn_lowering_effect.rs`
  - `crates/scoopc/src/mir/lower/fn_lowering_basic.rs`
  - `crates/scoopc/src/pipeline/mir_stage.rs`
  - HIR/MIR/typecheck fixtures covering call/effect/assign/update contracts
- 必须实现的内容：
  1. 将 `TypedHirEffectContracts` 内容迁入 `HirFacts` source-site typed contract 模块，并移除或降级旧类型为无重叠 adapter。
  2. 将 `MirLoweringFacts::from_typed_handoff(...)` 改为从 `HirFacts` 构造，删除 `from_hir_side_tables_and_resume_spans(...)` 及 fallback perform/resume/handle 分支。
  3. 删除 `MirSiteContractSource::{FallbackSideTables, Typed}` 或等价双轨状态；MIR lowering 不得再根据 facts 来源走不同语义路径。
  4. 更新 HIR preflight，要求 `HirFacts` 覆盖 call-site、continuation resume、perform、handle、assign place、with-update、top-level init root、extern global contracts。
  5. 增加/更新 fixtures，覆盖 typed contracts 完整性和 MIR lowering 不再使用 HIR-origin fallback reason。
- 禁止事项：
  - 禁止保留 typed/fallback 两套可替代输入。
  - 禁止让 MIR lowering 在缺少 `HirFacts` 时回头扫描 `LoweredHir` side tables。
  - 禁止把 source-site contracts 拆成“临时只给 fixture 用”的窄表。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features hir_preflight`
  3. `cargo test -p scoopc --no-default-features mir_lowering_facts`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/hir`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
  6. `cargo run -p scoop -- test --fixtures tests/fixtures/mir`
  7. `cargo clippy --all-targets -- -D warnings`
  8. 搜索 `FallbackSideTables|fallback_perform|fallback_resume|TypedHirEffectContracts`，确认旧双轨已删除或仅剩归档/迁移说明。
- 完成条件：
  - MIR lowering 只有一套 authoritative HIR fact input；
  - source-site typed contracts 的 owner 是 `HirFacts`；
  - HIR-origin fallback reason 不再作为合法 MIR smoke 兜底存在。
- 依赖：P2-T04R
- 完成记录：
  - 待填写。

## [TODO] P2-T05R：Review source-site contract 迁移结果

- 参考：P2-T05。
- 重点：
  - MIR lowering 是否已经删除 typed/fallback 双轨；
  - `HirFacts` 是否覆盖全部当前 `TypedHirEffectContracts` 内容；
  - preflight 是否能阻止合法 typed HIR 之后再发生 HIR-origin fallback。
- 必须复查的范围：
  - `crates/scoopc_hir_facts/`
  - `crates/scoopc/src/pipeline/hir_stage.rs`
  - `crates/scoopc/src/pipeline/hir_preflight.rs`
  - `crates/scoopc/src/mir/lower/`
  - 相关 fixtures
- 验证：
  - 重新运行 P2-T05 的所有验证；
  - 额外运行 `cargo test --all --all-targets --no-default-features`。
- 完成条件：
  - review 结论明确写出：source-site typed contract facts 单一化成立，或列出阻塞项并在本 review 内修复。
- 依赖：P2-T05
- 完成记录：
  - 待填写。

## [TODO] P2-T06：收口 HIR semantic barrier legality gate 与错误边界

- 参考：
  - `PLAN.md` §1.5、§4/P2、§6
  - `PIPELINE_REFACTOR.md` “错误收口边界”“global object/var/val 不能是 generic”
  - `crates/scoopc/src/typecheck/annotations.rs`
  - `crates/scoopc/src/pipeline_user_visible_failure_policy.rs`
- 目标：
  - 将 `@CallingConvention` non-generic、global roots monomorphic、top-level `var` storage policy 固定为 HIR barrier 约束；
  - 补齐 fixtures 和 diagnostics，防止后续阶段继续报告普通源码语义错误；
  - 明确后续 stage 的 user-visible failure policy。
- 必须检查和修改的主要位置：
  - `crates/scoopc/src/typecheck/annotations.rs`
  - `crates/scoopc/src/typecheck/`
  - `crates/scoopc/src/resolve/`
  - `crates/scoopc/src/pipeline/hir_completeness.rs`
  - `crates/scoopc/src/pipeline/hir_preflight.rs`
  - `crates/scoopc/src/pipeline_user_visible_failure_policy.rs`
  - `tests/fixtures/typecheck/`
  - `tests/fixtures/hir/`
  - `tests/fixtures/typecheck/` 中的错误 fixture 或等价错误 fixture 目录
- 必须实现的内容：
  1. 审计并补测试：`@CallingConvention` generic 函数必须在 typecheck/HIR barrier 拒绝，且错误不来自 MIR/codegen/link。
  2. 审计并补测试：top-level `var` 缺少 `@Global` / `@ThreadLocal` 必须在 HIR barrier 拒绝，并在 `HirFacts` global root 输出中携带已解析 storage policy。
  3. 冻结 global roots monomorphic 规则：当前语法若不允许 object/top-level val/var type params，也要用 parser/typecheck/HIR facts tests 证明这些 root 不会发布 generic identity；若发现可构造绕过路径，直接修复。
  4. 更新 `pipeline_user_visible_failure_policy` 或等价 tests，分类后续 stage 允许的 failure：compiler bug/impossible state、output drift、environment/toolchain/link/runtime path。
  5. 对仍在后续阶段暴露的普通源码语义错误做清单化处理：能修就前移到 HIR barrier；不能在本任务内修完则在本任务前插入前置任务并停止。
- 禁止事项：
  - 禁止把 HIR barrier 约束留给 MIR/materialization/codegen/link 报错。
  - 禁止只靠后续 panic 文案变化来“证明”错误前移。
  - 禁止为当前 fixture 形状添加特殊拒绝逻辑。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features typecheck`
  3. `cargo test -p scoopc --no-default-features hir_preflight`
  4. `cargo test -p scoopc --no-default-features pipeline_user_visible_failure_policy`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
  6. `cargo run -p scoop -- test --fixtures tests/fixtures/hir`
  7. `cargo clippy --all-targets -- -D warnings`
- 完成条件：
  - declaration legality gates 都在 HIR barrier 内稳定拒绝；
  - `HirFacts` 只发布已经通过 legality gate 的 global/native/root facts；
  - 后续阶段不再承担普通源码语义错误检查职责。
- 依赖：P2-T05R
- 完成记录：
  - 待填写。

## [TODO] P2-T06R：Review HIR semantic barrier 与错误边界

- 参考：P2-T06。
- 重点：
  - legality gates 是否确实在 HIR/typecheck 前端拒绝；
  - 后续阶段是否只保留 impossible-state / environment / link / runtime 类失败；
  - fixtures 是否覆盖成功和失败两侧。
- 必须复查的范围：
  - `crates/scoopc/src/typecheck/`
  - `crates/scoopc/src/pipeline/`
  - `crates/scoopc/src/pipeline_user_visible_failure_policy.rs`
  - `tests/fixtures/typecheck/`
  - `tests/fixtures/hir/`
- 验证：
  - 重新运行 P2-T06 的所有验证；
  - 额外抽查至少一个 `@CallingConvention` generic reject、一个 top-level `var` storage reject 和一个合法 global root facts fixture。
- 完成条件：
  - review 结论明确写出：HIR semantic barrier 与 declaration legality gate 满足 P2 约束，或列出阻塞项并在本 review 内修复。
- 依赖：P2-T06
- 完成记录：
  - 待填写。

## [TODO] P2-T07：P2 全包清场、文档同步与依赖审计

- 参考：
  - P2-T01 到 P2-T06R 的完成记录
  - `PLAN.md` §4/P2、§6
  - `PIPELINE_REFACTOR.md` / `PIPELINE-CLEANUP.md` 中 HIR facts 和 semantic barrier 描述
- 目标：
  - 全仓清理 P2 迁移后的旧名称、重复 facts、fallback side tables 和过期文档；
  - 确认 `HirStageOutput = { hir, hir_facts }` 与 HIR barrier 语义真实成立；
  - 为 `TODO-4.md` / P3 MIR boundary + MIR pass pipeline 留出干净起点。
- 必须实现的内容：
  1. 全仓搜索并分类处理 `TypedHirEffectContracts`、`ProgramFacts`、`FallbackSideTables`、`materialized_mir`、`materialized_pass_view`、`LoweredHir` side table 旧 owner 文案。
  2. 确认 `scoopc_hir_facts` 依赖方向、dump/verifier 和 fixture coverage 完整。
  3. 更新 `README.md`、`PIPELINE_REFACTOR.md`、`PIPELINE-CLEANUP.md`、`TODO.md` 和本文件完成记录；只有阶段级计划改变时才更新 `PLAN.md`。
  4. 确认 P3 入口处 MIR stage 只需要 HIR output 与 `hir_facts`，不再依赖 `LoweredHir` 临时 bundle 或旧 typed/fallback 合同。
  5. 记录 P2 结束时仍保留的 HIR scaffold，如果只是为 P7 backend cleanup 暂留，必须说明它不再是事实 owner。
- 禁止事项：
  - 禁止把 P3/P4/P5 的 MIR/effect/LIR 输出收口提前塞进本清场任务。
  - 禁止留下未解释的旧名称或重复 authoritative 查询面。
  - 禁止把 `ProgramFacts` 或 `TypedHirEffectContracts` 当作“兼容层”无限期保留而不记录清除条件。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc_hir_facts`
  3. `cargo test --all --all-targets --no-default-features`
  4. `cargo run -p scoop -- test`
  5. `cargo run -p scoop_tools -- spec-fixtures check`
  6. `cargo run -p scoop_tools -- dependency-gate`
  7. `cargo clippy --all-targets -- -D warnings`
  8. 全仓搜索 P2 清场关键词，并在完成记录中附允许命中分类摘要。
- 完成条件：
  - `HirStageOutput = { hir, hir_facts }` 在代码、tests 和文档中一致成立；
  - `LoweredHir` 不再是跨阶段 fact bundle 或 MIR artifact carrier；
  - P3 可以直接在 `HIR + hir_facts` 输入上收口 MIR boundary。
- 依赖：P2-T06R
- 完成记录：
  - 待填写。

## [TODO] P2-T07R：Review P2 全包完成度

- 参考：P2-T07。
- 重点：
  - P2 全部完成记录是否真实覆盖 HIR barrier、`hir_facts`、legality gate 和 error boundary；
  - 代码中是否还存在阻塞 P3 的 HIR facts owner 重叠或 HIR->MIR reverse dependency；
  - 文档和 TODO 索引是否与实现一致。
- 必须复查的范围：
  - `crates/scoopc_hir_facts/`
  - `crates/scoopc/src/hir/`
  - `crates/scoopc/src/pipeline/`
  - `crates/scoopc/src/mir/lower/`
  - `crates/scoopc/src/effect/`
  - `crates/scoopc/src/llvm/`
  - `README.md`
  - `PIPELINE_REFACTOR.md`
  - `PIPELINE-CLEANUP.md`
  - `TODO.md` / `TODO-3.md`
- 验证：
  - 重新运行 P2-T07 的所有验证；
  - 额外运行 `cargo tree -p scoopc_hir_facts`；
  - 抽查一个 HIR facts dump fixture、一个 MIR lowering fixture、一个 declaration legality reject fixture。
- 完成条件：
  - review 结论明确写出：P2 全包满足 `AST -> HIR` semantic frontend barrier 和独立 `hir_facts` 目标，或列出阻塞项并在本 review 内修复。
- 依赖：P2-T07
- 完成记录：
  - 待填写。
