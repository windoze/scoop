# TODO-4：MIR boundary + MIR pass pipeline

> 生成时间：2026-05-21
> 计划基线：[`PLAN.md`](./PLAN.md) §4/P3
> 设计基线：[`PIPELINE_REFACTOR.md`](./PIPELINE_REFACTOR.md)
> 审计基线：[`PIPELINE-CLEANUP.md`](./PIPELINE-CLEANUP.md)
> 索引：[`TODO.md`](./TODO.md)
> 顺序约束：严格按本文件任务顺序推进；每个实现任务后必须执行紧随其后的 review 任务。
> 本包目标：把 MIR 从 materialization 附带物收口成正式阶段输出，发布独立 `mir_facts` / pass artifacts 查询面，并把现有 MIR 优化重排成显式 pass pipeline。

## 全局约束

- 本包只处理 `PLAN.md` 的 P3：`MirStageOutput`、`mir_facts`、MIR-owned root inventories / snapshot binding / pass artifacts，以及普通调用图/实例级 MIR pass pipeline；不得提前把 P4 effect facts purity、P5 LIR output、P6 global init model 或 P7 backend cleanup 塞进本包。
- `MirStageOutput = { mir, mir_facts }` 语义必须成立；`mir` 是 MIR stage 自己的 authoritative IR/handoff，`mir_facts` 是 MIR stage 发布的独立事实产品。后续阶段需要 MIR root inventory、snapshot binding、pass artifacts 或 MIR-derived global facts 时，应显式消费 `mir_facts` 或 MIR pass query surface，不能从 `MaterializedMir`、P4/P5 wrapper 或 HIR scaffold 重新推导同一事实。
- `mir_facts` 必须是独立 fact crate 或等价独立数据产品；fact crate 只能依赖基础 crate，不得依赖 `scoopc` facade、HIR/MIR/effect/LIR/codegen stage crate 或其它 fact crate。当前 `TemplateKey` / `InstanceKey` 是 MIR materialization 内部键，不能被 fact crate 当作跨阶段基础 ID 直接外露。
- MIR pass 的 owner 必须唯一。普通 dispatch 去虚化、summary-driven inlining、escape analysis、closure simplification、cleanup / summary refresh 都属于 MIR 或 MIR facts；HIR 不承载 optimization pass，codegen 不得被 P3 新增为普通语义优化 fallback。
- HIR 仍可以发布 dispatch source-site typed contract，但不得把 exact-receiver 去虚化结果写入 HIR 输出；P3 完成后，HIR 层 `devirtualize_dispatch_calls` 必须被删除或退化为不执行优化的兼容参数，并由 review 证明不会改变 HIR 语义。
- P3 可以保留 P4/P5/P7 后续任务仍会清理的 nested wrapper / HIR scaffold 过渡输入，但不得新增新的上游整包回看，也不得把这些过渡输入描述为最终合法边界。
- 每个任务完成后，在该任务的“完成记录”下写明改动范围、核心决策、验证命令和残余风险。

## 触碰面基线

本节是 `TODO-4-INIT` 的仓库搜索记录，后续 P3 任务应优先从这些位置开始，不再重复做开放式仓库搜索。

### `MirStageOutput` 当前字段、构造点与读取点

`MirStageOutput` 定义在 `crates/scoopc/src/pipeline/mir_stage.rs:28-35`，构造入口是 `MirStageOutput::new(...)`。生产 direct-style MIR 构造点是 `crates/scoopc/src/pipeline/mir_stage.rs:204-228`，该路径先从 `HirFacts` 构造 `MirLoweringFacts`，再调用 `lower_hir_file_for_dump_with_facts(...)` 生成 `LoweredMir`，最后用 `MaterializedMir = None` 构造 stage output。

| 字段 / 查询面 | 当前职责 | 构造 / 写入点 | 主要读取点 | P3 处理方向 |
| --- | --- | --- | --- | --- |
| `lowered_mir: LoweredMir` | direct-style MIR file + 对应 `TypeStore` | `MirStageOutput::new(...)`；`lower_mir_stage_unvalidated(...)` | `file()`、`types()`、`stable_dump()`、`into_lowered_mir()`；MIR stage tests；effect facts stage `file()` 间接读取 | 收口为 MIR output 的 direct-style 组成部分，不再让 root inventory 旁路存放在 `MirStageOutput` 字段中 |
| `callable_body_indices: BTreeMap<String, usize>` | callable body root inventory | `collect_callable_body_indices(...)` 从 `LoweredMir.file.items` 扫描 | `callable_body_fqns()`、`callable_body()`；MIR tests / preflight | 迁入 `mir_facts` root inventory，`MirStageOutput` 查询方法改为委托 facts |
| `initializer_root_indices: BTreeMap<String, usize>` | top-level initializer/value root inventory | `collect_initializer_root_indices(...)` | `initializer_root_fqns()`、`initializer_root()`；MIR tests /后续 global init 输入 | 迁入 `mir_facts` root inventory，后续 P6 再闭合 init model |
| `global_root_indices: BTreeMap<String, usize>` | `@Extern` global root inventory | `collect_global_root_indices(...)` | `global_root_fqns()`、`extern_global_root()` | 迁入 `mir_facts` extern/global root inventory |
| `metadata_root_indices: BTreeMap<String, usize>` | type/object/typealias metadata root inventory | `collect_metadata_root_indices(...)` | `metadata_root_fqns()`、`metadata_root()` | 迁入 `mir_facts` metadata root inventory |
| `materialized_mir: Option<MaterializedMir>` | optional canonical materialized MIR snapshot | `with_materialized_mir(...)`；`pipeline/mod.rs:128-131`；`llvm_codegen_stage.rs:139-141`；多处 tests | `materialized_mir()`、`materialized_mir_mut()`；P4 `effect_facts_stage.rs:47-55,91-143`；P5 wrapper；LLVM/codegen 经 P5 wrapper 回看 pass view | 消除 P4 输入的 optional gap；改成 MIR-owned canonical snapshot / pass artifacts handoff，并发布 snapshot binding |

主要 direct downstream：`crates/scoopc/src/pipeline/mod.rs:80-165`、`crates/scoopc/src/pipeline/effect_facts_stage.rs:22-68,85-145`、`crates/scoopc/src/pipeline/effect_lowering_stage.rs:44-68,82-137`、`crates/scoopc/src/pipeline/llvm_codegen_stage.rs:126-160`、`crates/scoopc/src/pipeline/hir_preflight.rs:345-*`、`crates/scoopc/src/llvm/emit.rs:134-157`、`crates/scoopc/src/llvm/codegen/mod.rs:497-506,1038-1048`。

主要 indirect downstream（通过 `EffectFactsStageOutput` / `EffectLoweredStageOutput` / `MaterializedMirPassView`）：`crates/scoopc/src/effect_lowered/{ir.rs,segment.rs,frame.rs,materialize/tests.rs}`、`crates/scoopc/src/effect_facts/solver.rs`、`crates/scoopc/src/effect_facts/builder.rs`、`crates/scoopc/src/effect/state_machine/analysis/suspend_call.rs`、`crates/scoopc/src/llvm/codegen/{ordinary_callee.rs,main/context.rs,call/lowering.rs,main/identity.rs,main/declare.rs,mir_body/callable_lookup.rs,ty.rs}`、`crates/scoopc/src/llvm/reachability.rs`。

### `LoweredMir` 当前字段、构造点与读取点

`LoweredMir` 定义在 `crates/scoopc/src/mir/lower/entry.rs:24-31`。

| 字段 | 当前职责 | 构造点 | 主要读取点 | P3 处理方向 |
| --- | --- | --- | --- | --- |
| `file: File` | generic direct-style MIR compilation unit | `mir/lower/entry.rs:66-82` 的 `lower_for_dump(...)`；`pipeline/mir_stage.rs:204-218`；MIR/effect facts/layout tests scaffolding | `MirStageOutput::file()`、root inventory collection、stable dump、materialization input、MIR validation/tests | 继续作为 MIR IR 本体；不把 downstream facts 重复塞在 `File` 外的并列 map 中 |
| `types: TypeStore` | MIR 节点 `TypeId` 解码/展示使用的 type context | 同上 | `MirStageOutput::types()`、stable dump、effect facts/LIR/codegen 的 type lookup 过渡路径 | 保持单一 type universe；P3 不引入第二套 `TypeStore`，后续 LIR/codegen type handoff 由 P5/P7 收口 |

`LoweredMir` 的长期问题不是字段过多，而是 `MirStageOutput` 目前把它与 root indices 和 optional `MaterializedMir` 并列暴露。P3 应明确 direct-style MIR、canonical materialized snapshot 和 MIR facts 的关系，而不是让下游自行选择 source-of-truth。

### `MaterializedMir` / pass artifacts 当前字段、构造点与读取点

`MaterializedMir` 定义在 `crates/scoopc/src/mir/materialize/mod.rs:142-159`，主构造点是 `MirInstanceMaterializer::run(...)` 中的 `MaterializedMir { ... }`（`crates/scoopc/src/mir/materialize/run.rs:242-256`）。入口包括 `materialize_for_dump_with_opt_level(...)`（`crates/scoopc/src/mir/materialize/entry.rs:14-39`）、`materialize_compilation_unit_from_typechecked_inputs(...)`（`entry.rs:48-172`）和 `crate::mir::materialize_compilation_unit_from_typechecked_inputs_with_opt_level(...)` wrappers。

| 字段 / 查询面 | 当前职责 | 构造 / 写入点 | 主要读取点 | P3 处理方向 |
| --- | --- | --- | --- | --- |
| `file: File` | raw materialized MIR bodies | `MirInstanceMaterializer::run(...)` 的 `items` | raw dump/tests、`callable_view()`、部分 LIR/codegen fallback | 保留为 raw materialized snapshot；production canonical 读取应走 pass view / MIR facts |
| `types: TypeStore` | materialized snapshot type context | materializer 持有并移动进输出 | effect facts builder 当前可变写入；LIR/codegen type lookup | P3 记录 snapshot type context 绑定；P4 再处理 effect facts mutating type context |
| `instance_keys: Vec<InstanceKey>` | materialized + declaration-only instance inventory | materializer worklist + decl-only 合并 | `callable_view()`、dump/tests、stable instance key lookup | 作为 MIR-owned instance family inventory 发布，fact crate 不直接外露内部 `InstanceKey` |
| `summaries: MaterializedMirSummaries` | raw per-instance summary | `build_materialized_summary_table(...)` | raw callable view/tests；pass artifacts initial publication | 作为 raw materialization summary；canonical post-pass summary 走 pass artifacts |
| `top_level_value_tys` | top-level value type lookup | `collect_top_level_value_tys()` | effect/codegen helper | 若下游长期需要，归入 MIR facts 或 HIR facts owner；P3 不新增并列 owner |
| `stable_cone_key`、`stable_instance_keys`、`stable_template_keys`、`nongeneric_callable_signature_keys` | stable identity / symbol bridge | materialization inputs and stable-id helpers | codegen symbols、ABI identity、tests | 通过 `scoopc_ids` 稳定身份表达跨阶段 facts；不要把内部 `TemplateKey` / `InstanceKey` 直接放入 fact crate |
| `opt_level` | 当前 snapshot 的 MIR opt level | materialization options | effect facts solver、P5 stable dump | 进入 snapshot binding / pass pipeline metadata |
| `callable_families` | raw instance family -> callable FQN 映射 | `MaterializedCallableFamilies::from_inputs(...)` | raw callable view、pass artifacts initial publication | canonical family mapping 由 pass artifacts 发布 |
| `pass_artifacts: MaterializedMirPassArtifacts` | canonical post-pass callable body / summary / family / escape facts side table | `MaterializedMirPassArtifacts::from_initial_publication(...)`；`inline.rs`、`escape.rs`、`closure_simplify.rs` 写入 | `MaterializedMir::pass_view()`；effect facts builder；LIR builder；LLVM codegen/reachability | 抽成正式 MIR pass artifacts 查询面，并由 `mir_facts` 发布 binding / dump / verifier |
| `caller_side_pass_candidates` | request-root reachable non-generic caller body candidates | materializer 收集 | inline/escape/closure simplification | 纳入 pass pipeline context，而不是长期裸露为 ad-hoc candidate list |

`MaterializedMirPassArtifacts` 定义在 `crates/scoopc/src/mir/pass_view.rs:20-36`，字段包括 `callable_bodies_by_fqn`、`callable_families`、`instance_keys`、`summaries`、`escape_facts`、`overridden_body_fqns`、`overridden_summary_instances`。它通过 `replace_callable_body(...)`、`set_instance_summary(...)`、`replace_callable_family(...)`、`set_escape_facts(...)` 被当前 pass 修改，并通过 `MaterializedMirPassView` 暴露 `callable(...)`、`owner_of_callable(...)`、`instance(...)`、`root_body(...)`、`root_summary(...)`、`escape_facts()`。

### 当前 MIR pass 入口与执行顺序

现有 MIR pass 并没有独立 pipeline driver，而是内嵌在 `crates/scoopc/src/mir/materialize/run.rs:257-268` 的 materializer 尾部：

1. materialization worklist 完成后构造 raw `MaterializedMir`、raw summaries、initial pass artifacts。
2. 若 `opt_level.enables_summary_driven_mir_inlining()`，运行 `mir/inline.rs::run_summary_driven_inlining(...)`。该 pass 迭代最多 4 轮，写入 `replace_callable_body(...)` 与 `set_instance_summary(...)`。
3. 若 `opt_level.enables_mir_escape_analysis()`，运行 `mir/escape.rs::run_escape_analysis(...)`。该 analysis 写入 `MaterializedMirPassArtifacts::set_escape_facts(...)`。
4. 随后运行 `mir/closure_simplify.rs::run_non_escaping_closure_simplification(...)`。若有 rewrite，写入 callable body / summary，并再次运行 escape analysis 刷新 escape facts。
5. 最后调用 `materialized.validate_materialized()?`。

当前缺口：

- dispatch 去虚化不是显式 pass，而是内嵌在 `mir/materialize/rewrite.rs:1052-1110` 的 instance rewrite 过程中。
- HIR 层仍有 `devirtualize_dispatch_calls` 开关和 exact-receiver 去虚化路径，主要在 `hir/lower/expr/main_lower.rs:964-1000,1051-1079`、`hir/lower/main/compilation_unit.rs`、`hir/lower/util/*`、`hir/lower/main/impl_lowering.rs`。
- codegen / reachability 仍有去虚化残留，主要在 `llvm/codegen/call/lowering.rs:2623-2643` 与 `llvm/reachability.rs:831-*`；这些属于 P7 cleanup，但 P3 不得新增或依赖它们作为 MIR pass fallback。
- cleanup / summary refresh 目前散在 pass 内部：`inline.rs` 和 `closure_simplify.rs` 对 rewritten body 调用 `summarize_pass_rewritten_fun(...)`，没有显式全局 cleanup / summary refresh pass。
- escape analysis 当前按 opt level gate 运行；目标上应作为 MIR analysis / facts always-on，closure simplification 可继续 opt-level gate。

## [DONE] TODO-4-INIT：初始化并细化本任务包

- 目标：
  - 分析 `PLAN.md` §4/P3、`PIPELINE_REFACTOR.md` 和当前 MIR materialization/pass/output 的真实边界；
  - 生成本任务包的详细任务列表，覆盖 `MirStageOutput` 收口、`mir_facts`、pass artifacts 查询面和显式 MIR pass pipeline；
  - 更新 `TODO.md` 的具体任务索引，用新生成的任务替换或扩展 `TODO-4-INIT` 所在索引行。
- 必须实现的内容：
  1. 列出 `MirStageOutput`、`LoweredMir`、`MaterializedMir`、当前 MIR-owned root inventories / pass artifacts 的字段、构造点和下游读取点。
  2. 列出现有 escape analysis、devirtualization、inlining、closure simplification 和 cleanup/summary refresh 的入口与执行顺序。
  3. 把 P3 拆成数量适中的实现小阶段，每个阶段必须有明确目标、修改范围、验证命令和完成条件。
  4. 在每个实现小阶段后插入独立 review 任务，review 任务必须复审前一阶段是否满足 MIR owner 和输出边界约束。
  5. 同步更新 `TODO.md` 中的具体任务索引，确保任务 ID、状态和顺序与本文件一致。
- 完成条件：
  - `TODO-4.md` 不再只是范围说明，而是包含可直接执行的详细任务列表；
  - `TODO.md` 的具体任务索引已经同步反映 `TODO-4.md` 的新任务和 `[TODO]` 状态；
  - 本任务完成记录说明 MIR pipeline 拆分依据和仍需确认的下游兼容风险。
- 依赖：P2-T07R
- 完成记录：
  - 拆分依据：P3 的依赖顺序由当前输出混合方式决定，先建立独立 `mir_facts` 数据产品，再把 direct-style root inventories 迁入 facts，随后固定 canonical materialized snapshot / pass artifacts 查询面，最后把现有 pass 从 materializer 尾部和 HIR exact-receiver 路径迁入显式 MIR pass pipeline。
  - 触碰面记录：已在本文件“触碰面基线”中记录 `MirStageOutput`、`LoweredMir`、`MaterializedMir`、root inventories、pass artifacts 的字段/构造点/读取点，以及 inline、escape、closure simplification、devirtualization、summary refresh 的当前入口和顺序。
  - 任务结构：新增 7 个实现阶段和 7 个 review 阶段：`mir_facts` crate 与事实模型、root inventories 迁移、snapshot binding / pass artifacts 查询面、downstream MIR query 切换、显式 MIR pass pipeline、dispatch 去虚化 owner 迁移、P3 全包清场。
  - 核心决策：`MaterializedMirPassArtifacts` 已经是事实上的 canonical pass side table，但它还挂在 `MaterializedMir` 内部；P3 任务将把它提升为 MIR-owned 查询面并绑定到 `mir_facts`。`TemplateKey` / `InstanceKey` 继续视为 MIR 内部键，跨阶段 facts 必须使用基础 stable identity 或新建 stage-independent key。
  - 未展开风险：P4/P5/P7 仍会继续清理 `EffectFactsStageOutput` / `EffectLoweredStageOutput` 的 nested upstream bundle 和 LLVM HIR scaffold；P3 只负责确保它们读取的是 MIR stage 发布的 authoritative MIR handoff，不在本包内承诺完成 LIR/codegen 自足输入。codegen/reachability 中的去虚化残留按 `PLAN.md` P7 处理，但 P3 必须删除 HIR 层去虚化并阻止新的 HIR optimization owner。
  - 验证命令：文档/计划任务仅需检查 markdown/TODO 一致性；本次执行使用 `git diff --check`。

## [DONE] P3-T01：建立 `mir_facts` crate 与 MIR facts 数据模型

- 参考：
  - `PLAN.md` §1.2、§1.3、§4/P3
  - `PIPELINE_REFACTOR.md` “fact crate 必须自包含”“MIR stage”“优化框架”
  - 本文件“触碰面基线”
- 目标：
  - 在 workspace 中加入独立 `scoopc_mir_facts` 数据产品；
  - 定义 `MirFacts` 顶层结构、root inventory、snapshot binding、pass artifacts metadata、dump/verifier skeleton；
  - 固定 fact crate 只能依赖基础 crate 的 DAG 门禁。
- 必须检查和修改的主要位置：
  - `Cargo.toml`
  - `crates/scoopc/Cargo.toml`
  - 新增 `crates/scoopc_mir_facts/`
  - `tools/scoop_tools` dependency gate
  - `README.md` workspace/crate 概览
- 必须实现的内容：
  1. 新建 `scoopc_mir_facts` crate，至少包含 crate-level 职责文档、`#![forbid(unsafe_code)]`、`MirFacts` 顶层类型、空 verifier/dump skeleton 和单元测试。
  2. 将 `MirFacts` 初始划分为 root inventories、materialized snapshot binding、instance/callable family inventory、pass artifact metadata、MIR pass pipeline metadata 这几组模块或子结构。
  3. 只使用 `scoopc_span`、`scoopc_source`、`scoopc_types`、`scoopc_ids`、`scoopc_project_model` 中的基础类型表达 identity/type/span/cone 信息；不得引用 `crate::mir`、`crate::hir`、`MaterializedMir`、`TemplateKey`、`InstanceKey` 或 backend ABI 类型。
  4. 如现有基础 ID 不足以表达 snapshot/callable/body identity，先在 `scoopc_ids` 中新增 stage-independent key；不得把 MIR 内部 key 泄漏进 fact crate。
  5. 更新依赖门禁，使 `scoopc_mir_facts` 作为 fact crate 被检查，拒绝依赖 `scoopc` facade、stage/backend crate 或其它 fact crate。
  6. 在 `scoopc` facade 中只添加必要依赖或 re-export anchor，不迁移业务事实内容。
- 禁止事项：
  - 禁止让 `scoopc_mir_facts` 依赖 `scoopc`、`scoopc_hir_facts`、MIR stage 类型或 backend 类型。
  - 禁止把 `MaterializedMir`、`MaterializedMirPassView`、`FunDecl`、`File` 或 `Body` 放入 fact crate。
  - 禁止复制 `TypeStore` 或引入第二套 type universe。
- 验证：
  1. `cargo fmt`
  2. `cargo check -p scoopc_mir_facts`
  3. `cargo test -p scoopc_mir_facts`
  4. `cargo run -p scoop_tools -- dependency-gate`
  5. `cargo clippy --all-targets -- -D warnings`
- 完成条件：
  - `scoopc_mir_facts` 可独立编译和测试；
  - dependency gate 能证明该 fact crate 未依赖 facade、stage crate、backend crate 或其它 fact crate；
  - `MirFacts` 模型已能承接后续 root inventory、snapshot binding 和 pass artifacts 迁移任务。
- 依赖：TODO-4-INIT
- 完成记录：
  - 改动范围：新增 workspace crate `crates/scoopc_mir_facts/`，包含 `MirFacts` 顶层结构、root inventories、materialized snapshot bindings、instance/callable family inventory、pass artifact metadata、MIR pass pipeline metadata、dump/verifier skeleton 和单元测试；新增 `scoopc::mir_facts` facade anchor，并更新 workspace、README 与 dependency gate。
  - 核心决策：`scoopc_mir_facts` 只依赖 `scoopc_span`、`scoopc_source`、`scoopc_types`、`scoopc_ids`、`scoopc_project_model`，不依赖 `scoopc` facade、HIR/MIR stage、其它 fact crate 或 backend 类型。为避免泄漏 `TemplateKey` / `InstanceKey`，在 `scoopc_ids` 中新增通用 `StageArtifactKey` 表达 snapshot、instance/family 和 pass artifact revision identity。
  - 验证命令：`cargo fmt`；`cargo check -p scoopc_mir_facts`；`cargo test -p scoopc_mir_facts`；`cargo run -p scoop_tools -- dependency-gate`；`cargo clippy --all-targets -- -D warnings`；额外运行 `cargo test -p scoopc_ids` 与 `cargo test -p scoop_tools dependency_gate` 覆盖新增基础 key 和门禁测试。
  - 残余风险：当前 facts crate 仍是数据模型壳层；P3-T02 负责把现有 root inventory 构造迁入 `MirFacts`，P3-T03 之后负责把 canonical snapshot / pass artifacts 与真实 MIR stage handoff 接起来。当前 verifier/dump 只做结构性 skeleton 检查，不替代后续迁移验证。

## [DONE] P3-T01R：Review `mir_facts` crate 与事实模型

- 参考：P3-T01。
- 重点：
  - `scoopc_mir_facts` 是否满足 fact crate 自包含约束；
  - `MirFacts` 分类是否覆盖本文件触碰面基线中的 root inventory、snapshot binding、instance family、pass artifacts 和 pass pipeline metadata；
  - dependency gate 是否能阻止 fact crate 依赖 `scoopc`、HIR/MIR stage、其它 fact crate 或 backend crate。
- 必须复查的范围：
  - `Cargo.toml`
  - `crates/scoopc_mir_facts/`
  - `crates/scoopc_ids/` 中本任务新增的 stable identity
  - `crates/scoopc/Cargo.toml`
  - `tools/scoop_tools`
  - `README.md`
- 验证：
  - 重新运行 P3-T01 的所有验证；
  - 额外运行 `cargo tree -p scoopc_mir_facts`，确认只出现允许的基础依赖。
- 完成条件：
  - review 结论明确写出：`mir_facts` crate 壳层满足 P3/Pipeline fact DAG 约束，或列出阻塞项并在本 review 内修复。
- 依赖：P3-T01
- 完成记录：
  - 改动范围：复查 `P3-T01` 建立的 `scoopc_mir_facts` crate、`scoopc_ids::StageArtifactKey`、`scoopc` facade anchor、workspace/README 条目和 `tools/scoop_tools` dependency gate；review 未发现需要修复的代码问题，本次只更新任务状态与完成记录。
  - review 结论：`scoopc_mir_facts` 当前仅直接依赖 `scoopc_span`、`scoopc_source`、`scoopc_types`、`scoopc_ids`、`scoopc_project_model`，源码未引用 `scoopc` facade、HIR/MIR stage 类型、其它 fact crate、backend/LLVM 类型、`TemplateKey` 或 `InstanceKey`。`MirFacts` 已按 root inventories、materialized snapshot binding、instance/callable family inventory、pass artifact metadata、MIR pass pipeline metadata 分组，覆盖 P3-T01 要求的事实模型壳层。
  - dependency gate 结论：`FACT_CRATES` 已包含 `scoopc_mir_facts`，门禁会拒绝 fact crate 依赖 facade、driver/runtime/tool、stage/backend crate 或其它 fact crate；`cargo tree -p scoopc_mir_facts` 的 workspace 依赖只包含允许的基础 crate。
  - 验证命令：`cargo fmt`；`cargo check -p scoopc_mir_facts`；`cargo test -p scoopc_mir_facts`；`cargo run -p scoop_tools -- dependency-gate`；`cargo clippy --all-targets -- -D warnings`；`cargo tree -p scoopc_mir_facts`；额外复跑 `cargo test -p scoopc_ids` 与 `cargo test -p scoop_tools dependency_gate`。
  - 残余风险：当前 crate 仍是数据模型与 verifier/dump skeleton；实际 root inventory 构造、canonical snapshot/pass artifacts 绑定和下游查询切换仍由后续 `P3-T02` 到 `P3-T04` 完成。

## [DONE] P3-T02：迁移 MIR-owned root inventories 到 `mir_facts`

- 参考：
  - 本文件“`MirStageOutput` 当前字段、构造点与读取点”
  - `crates/scoopc/src/pipeline/mir_stage.rs`
- 目标：
  - 将 `callable_body_indices`、`initializer_root_indices`、`global_root_indices`、`metadata_root_indices` 从 `MirStageOutput` 私有字段迁入 `MirFacts`；
  - 让 `MirStageOutput` 发布 `mir_facts()`，并把现有 root query 方法改成委托 facts + MIR file；
  - 保持 direct-style MIR dump 和现有 fixtures 稳定，除非 facts dump 边界变化需要同步更新。
- 必须检查和修改的主要位置：
  - `crates/scoopc/src/pipeline/mir_stage.rs`
  - `crates/scoopc_mir_facts/`
  - `crates/scoopc/src/mir/dump.rs`
  - `crates/scoopc/src/pipeline/hir_preflight.rs`
  - `tests/fixtures/mir_lowered/`、`tests/fixtures/mir/` 中依赖 root inventory 输出的 fixture
- 必须实现的内容：
  1. 在 `MirFacts` 中实现 root inventory 数据结构，覆盖 callable body、initializer root、extern/global root、metadata root 的 stable identity、FQN、source/span/type 参考和必要 kind。
  2. 在 MIR stage 构造时从 `LoweredMir.file` 构建 `MirFacts`，并由 `MirStageOutput` 保存为唯一 root inventory owner。
  3. 删除 `MirStageOutput` 中四个 root index 字段，或将其降为 `MirFacts` 内部实现细节；不得保留两套并列 authoritative map。
  4. 更新 `callable_body_fqns()`、`callable_body()`、`initializer_root_fqns()`、`initializer_root()`、`global_root_fqns()`、`extern_global_root()`、`metadata_root_fqns()`、`metadata_root()` 等查询方法，使它们通过 `MirFacts` 定位 MIR item。
  5. 更新 stable dump / tests，使 MIR 本体与 MIR facts 的边界清晰可见。
- 禁止事项：
  - 禁止让下游直接重新扫描 `MirFile.items` 来替代 `MirFacts` root inventory。
  - 禁止把 HIR facts 或 HIR side table 重新打包进 `MirFacts`。
  - 禁止为了兼容旧测试保留重复 root map。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc_mir_facts`
  3. `cargo test -p scoopc --no-default-features mir_stage`
  4. `cargo test -p scoopc --no-default-features hir_preflight`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_lowered`
  6. `cargo clippy --all-targets -- -D warnings`
- 完成条件：
  - MIR root inventories 的唯一 owner 是 `MirFacts`；
  - `MirStageOutput` 的 root query surface 仍可用但不再持有独立 root maps；
  - direct-style MIR dump / tests 能证明 facts 与 MIR item 对齐。
- 依赖：P3-T01R
- 完成记录：
  - 改动范围：扩展 `scoopc_mir_facts::roots`，为 callable body、initializer、extern/global、metadata roots 发布稳定 identity、FQN、MIR item reference、span/source path、type/body reference 和 root-kind-specific detail；`MirStageOutput` 删除并列 root map 字段，新增 `mir_facts()`，现有 root query surface 全部委托 `MirFacts.roots` 后再定位 direct-style MIR item；`dump-mir` / `mir_lowered` golden 追加 `mir_facts { ... }` 边界。
  - 核心决策：`MirFacts` 只保存 stage-independent fact 数据和 direct-style MIR item index reference，不暴露 `FunDecl`、`InitializerRoot`、`ExternGlobalRoot`、`MetadataRoot` 或其它 MIR node 类型。root fact identity 使用 root kind + FQN，避免 object initializer 与 metadata root 共享 FQN 时发生 owner 冲突。source path 在 MIR stage 构造时归一化为稳定 dump path，`TypeId` 只作为同一 MIR type universe 内的引用保留。
  - 验证命令：`cargo fmt`；`cargo test -p scoopc_mir_facts`；`cargo test -p scoopc --no-default-features mir_stage`；`cargo test -p scoopc --no-default-features hir_preflight`；`cargo run -p scoop -- test --fixtures tests/fixtures/mir_lowered`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
  - 残余风险：P3-T02 只迁移 direct-style root inventories；canonical materialized snapshot binding、pass artifacts metadata、P4-ready mandatory snapshot handoff 和 downstream raw pass-view 切换仍按 P3-T03/P3-T04 处理。`EffectFactsStageOutput` / `EffectLoweredStageOutput` 的 nested upstream wrapper 仍属于后续 P4/P5/P7 收口范围。

## [DONE] P3-T02R：Review MIR root inventory 迁移结果

- 参考：P3-T02。
- 重点：
  - `MirStageOutput` 是否不再并列持有 root inventory map；
  - `MirFacts` root inventory 是否覆盖 callable/initializer/global/metadata roots；
  - 下游查询是否通过 `mir_facts()` 或委托方法，而不是重新扫描 `MirFile`。
- 必须复查的范围：
  - `crates/scoopc/src/pipeline/mir_stage.rs`
  - `crates/scoopc_mir_facts/`
  - `crates/scoopc/src/pipeline/hir_preflight.rs`
  - MIR dump / fixture 更新
- 验证：
  - 重新运行 P3-T02 的所有验证；
  - 额外搜索 `callable_body_indices|initializer_root_indices|global_root_indices|metadata_root_indices`，确认活跃源码中不再有 `MirStageOutput` 并列字段。
- 完成条件：
  - review 结论明确写出：root inventories 已由 `MirFacts` 唯一发布，或列出阻塞项并在本 review 内修复。
- 依赖：P3-T02
- 完成记录：
  - 改动范围：复查 `P3-T02` 迁移结果，覆盖 `crates/scoopc/src/pipeline/mir_stage.rs`、`crates/scoopc_mir_facts/`、`crates/scoopc/src/pipeline/hir_preflight.rs`、`tests/fixtures/mir_lowered/` 中的 MIR dump/facts 边界；本 review 未发现需要修复的代码问题，本次只更新任务状态与完成记录。
  - review 结论：`MirStageOutput` 已删除并列 root inventory map 字段，只保留 `mir_facts: MirFacts` 作为 root inventory owner；`callable_body_fqns()`、`callable_body()`、`initializer_root_fqns()`、`initializer_root()`、`global_root_fqns()`、`extern_global_root()`、`metadata_root_fqns()`、`metadata_root()` 均通过 `MirFacts.roots` 中的 item reference 委托定位 direct-style MIR item。`MirFacts.roots` 覆盖 callable body、initializer、extern/global、metadata 四类 root，并在 stable dump 中显式展示 `mir_facts { ... }` 边界。
  - 搜索结论：额外搜索 `callable_body_indices|initializer_root_indices|global_root_indices|metadata_root_indices`，活跃 Rust 源码中无命中；未发现下游为 root inventory 重新扫描 `MirFile` 来绕过 `MirFacts` 的路径。
  - 验证命令：`cargo fmt`；`cargo test -p scoopc_mir_facts`；`cargo test -p scoopc --no-default-features mir_stage`；`cargo test -p scoopc --no-default-features hir_preflight`；`cargo run -p scoop -- test --fixtures tests/fixtures/mir_lowered`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
  - 残余风险：canonical materialized snapshot binding、pass artifacts metadata、P4-ready mandatory snapshot handoff 和 downstream raw pass-view 切换仍按 `P3-T03` / `P3-T04` 处理；本 review 不提前收口这些后续边界。

## [DONE] P3-T03：固定 canonical materialized snapshot binding 与 pass artifacts 查询面

- 参考：
  - 本文件“`MaterializedMir` / pass artifacts 当前字段、构造点与读取点”
  - `crates/scoopc/src/mir/materialize/run.rs`
  - `crates/scoopc/src/mir/pass_view.rs`
  - `crates/scoopc/src/pipeline/effect_facts_stage.rs`
- 目标：
  - 让交给 P4 的 `MirStageOutput` 必然携带完整 canonical materialized MIR handoff，不再以 `Option<MaterializedMir>` 作为 P4 输入边界；
  - 将 snapshot binding、opt level、instance family inventory 和 pass artifacts metadata 作为 MIR-owned facts/query surface 发布；
  - 保留 direct-style MIR dump helper，但避免把“无 materialized snapshot 的 output”传入 effect facts/LIR 生产路径。
- 必须检查和修改的主要位置：
  - `crates/scoopc/src/pipeline/mir_stage.rs`
  - `crates/scoopc/src/pipeline/mod.rs`
  - `crates/scoopc/src/pipeline/effect_facts_stage.rs`
  - `crates/scoopc/src/mir/materialize/{entry.rs,run.rs,mod.rs}`
  - `crates/scoopc/src/mir/pass_view.rs`
  - `crates/scoopc_mir_facts/`
- 必须实现的内容：
  1. 引入明确的 MIR output/handoff 结构，区分 direct-style MIR、canonical materialized snapshot、pass artifacts query surface 和 `MirFacts`，并让 `MirStageOutput` 对 P4 发布完整 handoff。
  2. 消除 `EffectFactsStageOutput` 构造路径上的 `MissingMaterializedMirSnapshot` 正常错误分支；若无 snapshot，应在进入 P4 前被视为 MIR stage 内部 invariant violation。
  3. 在 `MirFacts` 中记录 snapshot binding：opt level、snapshot identity、query surface、canonical body FQN 集合、instance/family inventory、pass artifact revision 或等价稳定描述。
  4. 将 `MaterializedMirPassArtifacts` 的 metadata / dump / verifier 接到 MIR facts 或 MIR pass query surface，避免 P4/P5 把 `MaterializedMir` 当作唯一事实入口。
  5. 更新 dump/test helper 命名，使 direct-style-only helper 与 P4-ready MIR stage output helper 不再混淆。
- 禁止事项：
  - 禁止让 P4/effect facts stage 在边界内重新 materialize MIR 或补挂 snapshot。
  - 禁止保留“传入 `None`，P4 自己决定怎么办”的生产路径。
  - 禁止把 pass artifacts metadata 放进 effect facts 或 LIR output 中冒充 MIR owner。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features mir_stage`
  3. `cargo test -p scoopc --no-default-features effect_facts_stage`
  4. `cargo test -p scoopc --no-default-features effect_lowering_stage`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/effect_facts`
  6. `cargo clippy --all-targets -- -D warnings`
- 完成条件：
  - P4-ready `MirStageOutput` 必然包含 canonical materialized snapshot / pass artifacts handoff；
  - snapshot binding 与 pass artifacts metadata 由 MIR stage/facts 发布；
  - effect facts stage 不再把 missing snapshot 当作可恢复输入形态。
- 依赖：P3-T02R
- 完成记录：
  - 改动范围：将 direct-style-only MIR 输出拆为 `DirectStyleMirStageOutput`，把 P4-ready `MirStageOutput` 固定为携带 mandatory canonical `MaterializedMir` 的 handoff；新增 `load_p4_ready_mir_stage_output_for_dump(...)`，`dump-mir` 继续使用 direct-style-only helper。`EffectFactsStageOutput` / effect facts stage 现在只接受 P4-ready handoff，不再处理缺失 snapshot 的正常错误分支。
  - MIR facts 发布：`MirStageOutput` 构造时把 canonical snapshot binding、opt level、canonical pass-visible body FQN 集合、materialized instance/family inventory、pass artifact revision、summary artifact 和 escape-facts artifact metadata 写入 `MirFacts`；`scoopc_mir_facts` dump 现在会展示非空 snapshot / family / pass artifact metadata。
  - 核心决策：direct-style MIR dump 与 P4-ready stage output 使用不同类型表达，避免以 `Option<MaterializedMir>` 同时承载两种边界；snapshot 与 pass artifacts 的 stable identity 使用 `StageArtifactKey` / `BodyVersionKey`，不把 MIR 内部 `TemplateKey` / `InstanceKey` 暴露进 fact crate。
  - 验证命令：`cargo fmt`；`cargo test -p scoopc_mir_facts`；`cargo test -p scoopc --no-default-features mir_stage`；`cargo test -p scoopc --no-default-features effect_facts_stage`；`cargo test -p scoopc --no-default-features effect_lowering_stage`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_facts`；`cargo clippy --all-targets -- -D warnings`。
  - 残余风险：P3-T03 只固定 P4 输入 handoff 和 MIR-owned metadata 发布；downstream 仍有 raw `MaterializedMirPassView` / nested wrapper 过渡读取，按 P3-T04/P4/P5/P7 继续收口。effect facts builder 仍会在 P4 内部为 compiler runtime error schema 扩展 snapshot type context，这不是缺失 snapshot fallback，后续 effect facts purity 任务继续处理。

## [DONE] P3-T03R：Review MIR snapshot binding 与 pass artifacts 查询面

- 参考：P3-T03。
- 重点：
  - P4 输入是否不再是 optional materialized snapshot；
  - snapshot binding / pass artifacts metadata 是否归属 MIR stage/facts；
  - direct-style dump helper 是否没有被误当成 P4-ready stage output。
- 必须复查的范围：
  - `crates/scoopc/src/pipeline/mir_stage.rs`
  - `crates/scoopc/src/pipeline/effect_facts_stage.rs`
  - `crates/scoopc/src/mir/materialize/`
  - `crates/scoopc/src/mir/pass_view.rs`
  - `crates/scoopc_mir_facts/`
- 验证：
  - 重新运行 P3-T03 的所有验证；
  - 额外搜索 `MissingMaterializedMirSnapshot|materialized_mir: Option|materialized_mir_mut`，确认命中只剩不破坏 P3 边界的测试/过渡说明或被明确记录为 P4 前置清理项。
- 完成条件：
  - review 结论明确写出：canonical snapshot/pass artifacts 已成为 MIR-owned handoff，或列出阻塞项并在本 review 内修复。
- 依赖：P3-T03
- 完成记录：
  - 改动范围：复查 `P3-T03` 的 MIR stage handoff、effect facts stage 输入、materialized MIR/pass view、`scoopc_mir_facts` snapshot/pass artifact metadata 与 pipeline helper 命名；review 未发现需要修复的代码问题，本次只更新任务状态、完成记录和执行计划记录。
  - review 结论：P4 输入不再是 optional materialized snapshot；`MirStageOutput` 持有 mandatory canonical `MaterializedMir`，`EffectFactsStageOutput` / effect facts stage 只接受 P3 的 P4-ready handoff，并通过 `materialized_mir()` / `materialized_pass_view()` 读取 canonical snapshot。`MissingMaterializedMirSnapshot`、`materialized_mir: Option`、`materialized_mir_mut` 在活跃 Rust 源码中无命中。
  - MIR facts 结论：`MirStageOutput::from_direct_style(...)` 在构造 P4-ready handoff 时发布 snapshot binding、canonical body FQN 集合、instance/family inventory、pass artifact revision、summary artifact 和 escape-facts artifact metadata；`scoopc_mir_facts` verifier 会校验 canonical snapshot binding 与 artifact key 唯一性。
  - helper 边界结论：`DirectStyleMirStageOutput` 明确是 direct-style dump/validation helper，`load_direct_style_mir_stage_output_for_dump(...)` 与 `load_p4_ready_mir_stage_output_for_dump(...)` 命名和返回类型已分离；direct-style helper 没有被 P4/effect facts 生产路径当成 P4-ready output 使用。
  - 验证命令：`cargo fmt`；`cargo test -p scoopc_mir_facts`；`cargo test -p scoopc --no-default-features mir_stage`；`cargo test -p scoopc --no-default-features effect_facts_stage`；`cargo test -p scoopc --no-default-features effect_lowering_stage`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_facts`；`cargo clippy --all-targets -- -D warnings`；额外使用仓库搜索确认 `MissingMaterializedMirSnapshot|materialized_mir: Option|materialized_mir_mut` 在活跃 Rust 源码中无命中，并运行 `git diff --check`。
  - 残余风险：effect facts builder 仍会在 P4 内部扩展 materialized snapshot type context，且 downstream raw pass-view / nested wrapper 过渡读取仍按 `P3-T04`、P4/P5/P7 后续任务收口；这些不构成 P3-T03R 阻塞项。

## [DONE] P3-T04：切换下游 MIR 查询到 `mir_facts` / pass artifacts surface

- 参考：
  - 本文件“主要 indirect downstream”
  - `crates/scoopc/src/pipeline/effect_facts_stage.rs`
  - `crates/scoopc/src/pipeline/effect_lowering_stage.rs`
  - `crates/scoopc/src/effect_lowered/builder.rs`
- 目标：
  - 让 effect facts stage、LIR builder 和现有 LLVM bridge 读取 MIR root/pass 信息时走 MIR stage 发布的 query surface；
  - 将当前 LIR stage 现场重算的 MIR-derived global facts（例如 nominal direct supertypes）迁回 `mir_facts`；
  - 不在本任务内拆除 P4/P5/P7 的 nested wrapper，但要避免继续扩散 raw `MaterializedMir` / `MaterializedMirPassView` 作为万能查询入口。
- 必须检查和修改的主要位置：
  - `crates/scoopc/src/pipeline/effect_facts_stage.rs`
  - `crates/scoopc/src/pipeline/effect_lowering_stage.rs:85-97,119-131`
  - `crates/scoopc/src/effect_lowered/builder.rs`
  - `crates/scoopc/src/effect_lowered/{ir.rs,segment.rs,frame.rs}`
  - `crates/scoopc/src/effect_facts/{builder.rs,solver.rs}`
  - `crates/scoopc/src/llvm/{emit.rs,codegen/**,reachability.rs}` 中经 P5 wrapper 读取 MIR pass view 的位置
- 必须实现的内容：
  1. 为 downstream 提供窄 MIR query surface：root inventory、canonical callable body、summary、family、escape facts、snapshot binding、MIR-derived nominal metadata。
  2. 将 `effect_lowering_stage` 中 `collect_nominal_direct_supertypes_from_mir_file(...)` 这类 LIR 现场重算迁到 MIR facts，LIR 只读取已发布事实。
  3. 更新 effect facts builder / LIR builder，使其输入签名尽量表达为 `MirFacts` + canonical pass query，而不是裸 `MaterializedMir`。
  4. 对暂时不能切掉的 raw pass-view 读取，新增明确 TODO/完成记录说明其归属 P4/P5/P7，且不得作为新任务的正常扩展点。
  5. 更新 tests，覆盖 downstream 不再重扫 MIR file 生成已迁移 facts。
- 禁止事项：
  - 禁止在 LIR/effect facts/codegen 中重新构造与 `MirFacts` 同职责的 fact table。
  - 禁止把 `MirFacts` 复制进 `EffectFactsStageOutput` 或 `EffectLoweredStageOutput` 当作 nested upstream bundle 的新变体。
  - 禁止为了快速通过测试而让 `mir_facts` 和旧 raw view 同时回答同一问题。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features effect_facts_stage`
  3. `cargo test -p scoopc --no-default-features effect_lowering_stage`
  4. `cargo test -p scoopc --no-default-features effect_lowered`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`
  6. `cargo clippy --all-targets -- -D warnings`
- 完成条件：
  - 已迁移的 MIR-derived facts 只有 MIR stage/facts 一个 owner；
  - downstream 读取 root/pass 信息时优先走 MIR query surface；
  - 剩余 raw pass-view 读取被明确限制为 P4/P5/P7 过渡风险，不阻塞 P3 继续推进。
- 依赖：P3-T03R
- 完成记录：
  - 改动范围：新增 `scoopc_mir_facts::metadata`，由 MIR stage 从 direct-style metadata roots 发布 nominal/object direct-supertype facts；`MirFacts` verifier 纳入该 metadata fact identity。`EffectFactsStageOutput` / `EffectLoweredStageOutput` 新增 `mir_facts()` 查询入口，`LateLoweredProgramBuilder::from_canonical_inputs(...)` 改为显式接收 `MirFacts` + canonical pass view + P4 effect facts。
  - 下游切换：删除 LIR/effect-lowered 侧 `collect_nominal_direct_supertypes_from_mir_file(...)` 和 override builder 路径；`effect_lowering_stage`、raw late-lowered builder tests、LLVM layout ABI visibility helper 均从 MIR stage 发布的 `MirFacts` 读取 nominal direct supertypes。`EffectFactsStageOutput::materialized_pass_view()` 现在委托 `MirStageOutput` 的 canonical pass query surface，而不是自行从 raw snapshot 重建入口。
  - 测试覆盖：新增/更新 MIR facts 和 effect-lowered tests，断言 nominal upcast 所需的 `a.Derived -> a.Base` direct-supertype 信息来自 `MirFacts`；仓库搜索确认旧的 MIR-file 重扫 helper 与 override builder 路径已删除。
  - 验证命令：`cargo fmt`；`cargo test -p scoopc_mir_facts`；`cargo test -p scoopc --no-default-features mir_stage`；`cargo test -p scoopc --no-default-features effect_facts_stage`；`cargo test -p scoopc --no-default-features effect_lowering_stage`；`cargo test -p scoopc --no-default-features effect_lowered`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`；`cargo clippy --all-targets -- -D warnings`；额外搜索 `collect_nominal_direct_supertypes_from_mir_file|with_nominal_direct_supertypes` 无命中，并运行 `git diff --check`。
  - 残余风险：P4 effect facts builder 仍因 compiler-generated runtime error schema 需要可变 materialized snapshot/type context；P5 wrapper 与 LLVM bridge 中的 `materialized_pass_view` 读取仍作为 P5/P7 过渡风险保留。本任务未复制 `MirFacts` 到 effect facts/LIR outputs，也未把 raw pass view 扩展为新的事实 owner。

## [DONE] P3-T04R：Review downstream MIR query 切换结果

- 参考：P3-T04。
- 重点：
  - LIR/effect facts 是否停止现场重算已归属 MIR 的 facts；
  - downstream 是否通过 MIR query surface 读取 root/pass 信息；
  - 是否新增了 `MirFacts` 与 raw pass view 并列回答同一问题的双轨。
- 必须复查的范围：
  - P3-T04 修改过的 effect facts / LIR / LLVM bridge 文件
  - `crates/scoopc_mir_facts/`
  - `crates/scoopc/src/pipeline/effect_lowering_stage.rs`
- 验证：
  - 重新运行 P3-T04 的所有验证；
  - 额外搜索 `collect_nominal_direct_supertypes_from_mir_file|materialized_pass_view\(`，确认剩余命中均有合理 owner 或后续阶段归属说明。
- 完成条件：
  - review 结论明确写出：downstream 已切到 MIR-owned query surface，或列出阻塞项并在本 review 内修复。
- 依赖：P3-T04
- 完成记录：
  - 改动范围：复查 `P3-T04` 修改过的 MIR facts、effect facts stage、effect lowering stage、late-lowered builder/tests 与 LLVM bridge 过渡读取点；review 未发现需要修复的代码问题，本次只更新任务状态、完成记录和执行计划记录。
  - review 结论：downstream 已把 nominal direct-supertype 这类已迁移 MIR-derived facts 切到 `MirFacts.metadata`，`LateLoweredProgramBuilder::from_canonical_inputs(...)` 显式消费 `MirFacts` + canonical pass query + P4 effect facts；`EffectFactsStageOutput` / `EffectLoweredStageOutput` 只提供 `mir_facts()` 查询入口，没有复制 `MirFacts` 成新的 nested upstream bundle。
  - 搜索结论：`collect_nominal_direct_supertypes_from_mir_file|with_nominal_direct_supertypes` 在活跃 Rust 源码中无命中；`materialized_pass_view(` 的剩余命中属于 `MirStageOutput` canonical query surface、P4/P5 handoff accessor、定向测试或已按 P5/P7 记录的 LLVM 过渡桥接，没有发现 `MirFacts` 与 raw pass view 并列回答同一已迁移 fact 的新双轨。
  - 验证命令：`cargo fmt`；`cargo test -p scoopc --no-default-features effect_facts_stage`；`cargo test -p scoopc --no-default-features effect_lowering_stage`；`cargo test -p scoopc --no-default-features effect_lowered`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
  - 残余风险：P5 wrapper 和 LLVM backend 仍通过 `materialized_pass_view()` 读取 canonical MIR pass surface，并且 LLVM 仍保留 HIR compatibility scaffold；这些已由 P4/P5/P7 后续阶段收口，不是 P3-T04R 阻塞项。本 review 未推进 P3-T05。

## [DONE] P3-T05：建立显式 MIR pass pipeline 与 refresh 顺序

- 参考：
  - 本文件“当前 MIR pass 入口与执行顺序”
  - `crates/scoopc/src/mir/materialize/run.rs`
  - `crates/scoopc/src/mir/{inline.rs,escape.rs,closure_simplify.rs,summary.rs,pass_view.rs}`
- 目标：
  - 将 materializer 尾部隐式 pass 调度抽成显式 MIR pass pipeline；
  - 固定 pass 顺序、opt-level gate、analysis refresh 和 summary refresh 规则；
  - 让 pass artifacts 的 mutation 都通过统一 pipeline context 发生。
- 必须检查和修改的主要位置：
  - 新增或重构 `crates/scoopc/src/mir/pass_pipeline.rs`
  - `crates/scoopc/src/mir/materialize/run.rs`
  - `crates/scoopc/src/mir/inline.rs`
  - `crates/scoopc/src/mir/escape.rs`
  - `crates/scoopc/src/mir/closure_simplify.rs`
  - `crates/scoopc/src/mir/summary.rs`
  - `crates/scoopc/src/mir/pass_view.rs`
- 必须实现的内容：
  1. 引入 `MirPassPipeline` 或等价 driver，显式列出 pass schedule、输入、输出、opt-level gate 和 refresh 条件。
  2. 将现有 summary-driven inlining、escape analysis、closure simplification 从 `MirInstanceMaterializer::run(...)` 尾部移入 pipeline driver。
  3. 将 escape analysis 改为 MIR analysis / facts owner：目标上 always-on；如果实现中必须保留 opt-level gate，必须在完成记录中说明 blocker 并新增前置任务，不能悄悄改变目标。
  4. 将 cleanup / summary refresh 明确为 pipeline step 或 helper，不再让每个 pass 私下决定是否刷新 summary。
  5. 在 `mir_facts` 或 stable dump 中记录 pass pipeline metadata，至少能看出哪些 pass 运行、哪些 pass 改写了 body/summary、escape facts 对应哪个 pass revision。
  6. 保持 raw materialized MIR 与 pass artifacts 分层，不把 pass rewrite 写回 raw `MaterializedMir.file`。
- 禁止事项：
  - 禁止继续在 `MirInstanceMaterializer::run(...)` 尾部直接硬编码 pass 顺序。
  - 禁止 pass 直接扫描 HIR 或 codegen state。
  - 禁止为修测试把 pass rewrite 写回 raw materialized MIR。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features mir::pass_view`
  3. `cargo test -p scoopc --no-default-features mir::inline`
  4. `cargo test -p scoopc --no-default-features mir::escape`
  5. `cargo test -p scoopc --no-default-features mir::closure_simplify`
  6. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_materialized`
  7. `cargo clippy --all-targets -- -D warnings`
- 完成条件：
  - MIR pass 调度有单一显式 owner；
  - inline / escape / closure simplification / cleanup / summary refresh 的顺序可由代码和 dump 验证；
  - materializer 只负责构造初始 materialized snapshot 和调用 pipeline，不再隐式持有 pass policy。
- 依赖：P3-T04R
- 完成记录：
  - 改动范围：新增 `crates/scoopc/src/mir/pass_pipeline.rs` 作为 MIR pass 调度的唯一 owner；`MirInstanceMaterializer::run(...)` 现在只构造 raw materialized snapshot、初始化 pass artifacts，然后调用显式 pipeline。`inline`、`escape`、`closure_simplify` 改为通过 `MirPassPipelineContext` 发布 rewritten body、summary refresh 和 escape facts，不再直接在 materializer 尾部硬编码调度策略。
  - pass 顺序与 gate：pipeline 顺序固定为 summary-driven inlining（非 `O0` gate）-> escape analysis（always-on）-> closure simplification（非 `O0` gate）-> closure rewrite 后的 escape refresh（仅 closure pass 改写 body 时运行）。escape analysis 已从可选优化附属品改为 MIR analysis/facts owner；`O0` 现在也发布 pass-view escape facts，但 closure simplification 仍不会在 `O0` 改写 body。
  - refresh / cleanup 决策：summary refresh 集中在 `MirPassPipelineContext::publish_instance_rewrite(...)`，由 pipeline context 读取上一版 pass summary 并统一写入 pass artifacts revision；pass-local dead-artifact cleanup 收口到 `cleanup_pass_rewritten_body(...)` helper，按 inline / closure rewrite 形态选择窄 cleanup mode。pass rewrite 仍只写入 `MaterializedMirPassArtifacts`，不回写 raw `MaterializedMir.file`。
  - MIR facts 发布：`MaterializedMirPassArtifacts` 现在记录 pipeline run、revision、body override revision、summary override revision 和 escape-facts revision；`MirFacts.pass_pipeline` 与 `MirFacts.pass_artifacts` stable dump 可显示哪些 pass 运行、输入/输出 revision、body/summary 改写数和 escape facts 对应 revision。
  - 验证命令：`cargo fmt`；`cargo test -p scoopc --no-default-features mir::pass_view`；`cargo test -p scoopc --no-default-features mir::inline`；`cargo test -p scoopc --no-default-features mir::escape`；`cargo test -p scoopc --no-default-features mir::closure_simplify`；`cargo test -p scoopc_mir_facts`；`cargo test -p scoopc --no-default-features mir_stage`；`cargo test -p scoopc --no-default-features mir::materialize`；`cargo clippy --all-targets -- -D warnings`。
  - fixture 验证说明：任务原验证命令 `cargo run -p scoop -- test --fixtures tests/fixtures/mir_materialized` 已执行但当时仓库中不存在 `tests/fixtures/mir_materialized` 目录，命令在 fixture 定位阶段失败。为覆盖现有 materialized MIR 路径，额外运行 `cargo test -p scoopc --no-default-features mir::materialize` 全部通过；`P3-T05R` 已补齐真实 `mir_materialized` fixture phase 与 pass-pipeline metadata golden，使该验证命令成为后续可执行门禁。
  - 残余风险：dispatch 去虚化仍在 P3-T06 迁移；`MirPassKind::Devirtualization` 还未进入实际 schedule。本任务未新增 HIR/codegen fallback，也未把 pass rewrite 写回 raw materialized MIR。

## [DONE] P3-T05R：Review 显式 MIR pass pipeline

- 参考：P3-T05。
- 重点：
  - pass schedule 是否有单一 owner；
  - escape analysis 是否成为 MIR analysis / facts，而不是可选优化附属品；
  - summary refresh / cleanup 是否显式，不再散落在 materializer 尾部或 pass 私有逻辑里。
- 必须复查的范围：
  - `crates/scoopc/src/mir/pass_pipeline.rs` 或等价新模块
  - `crates/scoopc/src/mir/materialize/run.rs`
  - `crates/scoopc/src/mir/{inline.rs,escape.rs,closure_simplify.rs,summary.rs,pass_view.rs}`
  - `crates/scoopc_mir_facts/` pass metadata
- 验证：
  - 重新运行 P3-T05 的所有验证；
  - 额外搜索 `run_summary_driven_inlining\(|run_escape_analysis\(|run_non_escaping_closure_simplification\(`，确认调度入口只由 MIR pass pipeline 控制。
- 完成条件：
  - review 结论明确写出：MIR pass pipeline 已显式化且符合 P3 owner 约束，或列出阻塞项并在本 review 内修复。
- 依赖：P3-T05
- 完成记录：
  - 改动范围：复查 `P3-T05` 的 `pass_pipeline.rs`、materializer 尾部、inline/escape/closure simplification、summary refresh、pass artifacts 与 MIR facts metadata；修复 review 中发现的两个直接问题：`OptLevel::enables_mir_escape_analysis()` 仍表达旧 O0 gate 语义，以及 `tests/fixtures/mir_materialized` 验证路径缺少真实 fixture phase。新增 `mir_materialized` fixture runner 路由和 `pass_pipeline_metadata` golden，覆盖 P4-ready MIR facts 中的 pass schedule、body/summary override revision 与 closure simplification 后 escape refresh。
  - review 结论：MIR pass 调度已由 `run_mir_pass_pipeline(...)` / `MirPassPipeline` 单一 owner 控制；`MirInstanceMaterializer::run(...)` 只构造 raw snapshot、初始化 pass artifacts、调用 pipeline 并验证。搜索 `run_summary_driven_inlining|run_escape_analysis|run_non_escaping_closure_simplification` 确认除定义外，活跃调度调用只在 `pass_pipeline.rs` 中出现。escape analysis 在 pipeline 中 always-on，`OptLevel::enables_mir_escape_analysis()` 已同步为所有 opt level 都发布 MIR escape-analysis facts。
  - refresh / cleanup 结论：summary refresh 集中在 `MirPassPipelineContext::publish_instance_rewrite(...)`，cleanup 集中在 `cleanup_pass_rewritten_body(...)`，pass 只通过 pipeline context 发布 body/summary/escape facts mutation。`pass_pipeline_metadata.mir` golden 显示 summary-driven inlining -> escape analysis -> closure simplification -> escape-analysis refresh 的 revision 顺序，且 raw materialized MIR 与 pass artifacts 继续分层。
  - 验证命令：`cargo fmt`；`cargo test -p scoopc --no-default-features mir::pass_view`；`cargo test -p scoopc --no-default-features mir::inline`；`cargo test -p scoopc --no-default-features mir::escape`；`cargo test -p scoopc --no-default-features mir::closure_simplify`；`cargo test -p scoopc_project_model`；`cargo test -p scoop --no-default-features fixtures`；`cargo run -p scoop -- test --fixtures tests/fixtures/mir_materialized`；`cargo test -p scoopc_mir_facts`；`cargo test -p scoopc --no-default-features mir_stage`；`cargo test -p scoopc --no-default-features mir::materialize`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
  - 残余风险：dispatch 去虚化仍按 `P3-T06` 迁移；`MirPassKind::Devirtualization` 尚未进入实际 schedule。P5/P7 wrapper/backend 过渡读取不属于本 review 的新阻塞项。

## [DONE] P3-T06：迁移 dispatch 去虚化到 MIR pass 并删除 HIR owner

- 参考：
  - `PIPELINE_REFACTOR.md` “HIR 中的 dispatch 去虚化”“MIR devirtualization”
  - 本文件“当前 MIR pass 入口与执行顺序”
  - `crates/scoopc/src/mir/materialize/rewrite.rs:1052-1110`
  - `crates/scoopc/src/hir/lower/expr/main_lower.rs:964-1000,1051-1079`
- 目标：
  - 将 virtual/interface dispatch exact-receiver 去虚化从 materialization rewrite 与 HIR lowering 开关中迁入显式 MIR pass；
  - 删除 HIR 层 `devirtualize_dispatch_calls` owner，使 HIR 一律保留 dynamic dispatch 语义与 source-site dispatch contract；
  - 保证 MIR devirtualization 是普通语义去虚化的唯一 authoritative owner。
- 必须检查和修改的主要位置：
  - `crates/scoopc/src/mir/materialize/rewrite.rs`
  - `crates/scoopc/src/mir/pass_pipeline.rs` 或等价新模块
  - `crates/scoopc/src/devirtualize.rs`
  - `crates/scoopc/src/hir/lower/{mod.rs,expr/main_lower.rs,main/compilation_unit.rs,main/impl_lowering.rs,util/**}`
  - `crates/scoopc/src/frontend.rs`
  - `crates/scoopc/src/cone/pre_specialize.rs`
  - `crates/scoopc/src/mir/materialize/tests.rs`
  - `crates/scoopc/src/monomorph/lower.rs`
- 必须实现的内容：
  1. 新增 MIR devirtualization pass，输入为 canonical pass-visible MIR + MIR/HIR facts 中已经发布的 dispatch metadata，输出为 pass artifacts 中 rewritten callable body / refreshed summary。
  2. 从 `mir/materialize/rewrite.rs` 中移除 virtual/interface call 物化期间直接改写为 `CallKind::Direct` 的逻辑；materialization 只应做 substitution / instance discovery，不承担优化 owner。
  3. 删除或无害化 HIR lowering 中的 `devirtualize_dispatch_calls` 参数和调用点，确保 HIR output 保留 `CallKind::Virtual` / `CallKind::Interface` 对应语义。
  4. 更新 tests/fixtures：原本断言 HIR/monomorph lowering 已去虚化的测试必须改为断言 MIR pass 后去虚化；不得用 wrapper 或 narrower fixture 避开 broken path。
  5. 明确记录 codegen/reachability 中仍存在的去虚化残留归属 P7，不允许 P3 新增依赖这些残留的行为。
- 禁止事项：
  - 禁止保留 HIR `devirtualize_dispatch_calls: true` 路径作为优化 owner。
  - 禁止在 codegen 中新增新的 devirtualization fallback 来弥补 MIR pass。
  - 禁止只迁移 virtual 或只迁移 interface；两类 dispatch 必须作为同一 root cause 处理。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features hir`
  3. `cargo test -p scoopc --no-default-features mir::materialize`
  4. `cargo test -p scoopc --no-default-features monomorph`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_materialized`
  6. 搜索 `devirtualize_dispatch_calls`，活跃源码中不得再有执行优化的路径。
  7. `cargo clippy --all-targets -- -D warnings`
- 完成条件：
  - virtual/interface devirtualization 的 ordinary owner 是显式 MIR pass；
  - HIR lowering 不再执行 dispatch 去虚化；
  - materialization rewrite 不再把 devirtualization 藏在 substitution 流程里。
- 依赖：P3-T05R
- 完成记录：
  - 改动范围：新增 `crates/scoopc/src/mir/dispatch_devirtualize.rs`，将 exact-receiver virtual/interface dispatch 去虚化接入显式 MIR pass pipeline；`MirInstanceMaterializer` 现在只在 substitution / reachability 阶段发现 dispatch candidate 实例并记录 canonical target facts，不再把 dispatch call 直接改写为 `CallKind::Direct`。删除 HIR lowering 的 `devirtualize_dispatch_calls` 参数、上下文和执行路径，`frontend` / `cone` / generic lowering 调用点同步改为只保留 dynamic dispatch contract。
  - 核心决策：devirtualization pass always-on，调度在 summary-driven inlining 之前，因此后续 inlining/escape/closure pass 只消费 pass-visible canonical body。materialization 为 effect-generic dispatch candidate 继续做实例发现，避免把 owner/effect args 丢给 pass 临时推断；pass-published rewritten body 会规整重复 `SiteId`，保证 request-root ordinary callable 进入 pass artifacts 后仍满足 materialized validation。
  - 测试与 fixture：`mir::materialize` 新增 exact-receiver devirtualization pass 回归；monomorph 旧的“已在 monomorph 去虚化”断言改为验证 monomorph 保留 dynamic dispatch 交给 MIR pass；`tests/fixtures/mir_materialized/pass_pipeline_metadata.mir` 更新为显示 devirtualization pass revision/run。
  - 验证命令：`cargo fmt`；`cargo test -p scoopc --no-default-features hir --lib`；`cargo test -p scoopc --no-default-features mir::materialize --lib`；`cargo test -p scoopc --no-default-features monomorph --lib`；`cargo run -p scoop -- test --fixtures tests/fixtures/mir_materialized`；搜索 `devirtualize_dispatch_calls` 无命中；搜索 `try_devirtualize_dispatch_target\(` 确认非测试活跃调用点只剩 MIR pass 与 P7 归属的 LLVM codegen/reachability 残留；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
  - 残余风险：LLVM codegen / reachability 中的去虚化残留仍按 P7 cleanup 处理，本任务未新增也未依赖这些 backend fallback。P3-T06 只完成普通语义 dispatch 去虚化 owner 迁移；P7 仍需移除 backend residual。

## [DONE] P3-T06R：Review dispatch 去虚化 owner 迁移结果

- 参考：P3-T06。
- 重点：
  - HIR 是否保留 dynamic dispatch 语义且不再有 devirtualization 开关；
  - MIR devirtualization 是否由显式 pass pipeline 调度；
  - materialization rewrite 是否只做 substitution / instance discovery，不再做优化改写。
- 必须复查的范围：
  - `crates/scoopc/src/mir/pass_pipeline.rs` 或等价新模块
  - `crates/scoopc/src/mir/materialize/rewrite.rs`
  - `crates/scoopc/src/hir/lower/**`
  - `crates/scoopc/src/frontend.rs`
  - `crates/scoopc/src/cone/pre_specialize.rs`
  - `crates/scoopc/src/mir/materialize/tests.rs`
  - `crates/scoopc/src/monomorph/lower.rs`
- 验证：
  - 重新运行 P3-T06 的所有验证；
  - 额外搜索 `try_devirtualize_dispatch_target\(`，确认非测试活跃调用点只位于 MIR pass 或明确归属 P7 的 backend residual。
- 完成条件：
  - review 结论明确写出：普通 dispatch 去虚化已由 MIR pass 唯一拥有，或列出阻塞项并在本 review 内修复。
- 依赖：P3-T06
- 完成记录：
  - 改动范围：复查 `P3-T06` 的 MIR pass pipeline、`dispatch_devirtualize` pass、materialization rewrite / reachable discovery、HIR lowering 调用点、`frontend` / `cone` 调用面、`monomorph` tests 与 `mir_materialized` fixture；review 未发现需要修复的代码问题，本次只更新任务状态、完成记录和执行计划记录。
  - review 结论：普通 virtual/interface dispatch 去虚化已由显式 MIR pass pipeline 中的 `MirPassKind::Devirtualization` 调度并拥有；HIR lowering 不再存在 `devirtualize_dispatch_calls` 开关，仍记录 dispatch call-site contract 并保留 dynamic dispatch 语义；materialization rewrite 对 dispatch call 只做 receiver/type substitution、candidate instance discovery 和 canonical target fact 记录，不再直接把 call 改写为 `CallKind::Direct`。
  - 搜索结论：`devirtualize_dispatch_calls` 在活跃 Rust 源码中无命中；`try_devirtualize_dispatch_target(` 的非测试命中仅为 MIR pass、共享 helper，以及已按 `P3-T06` 完成记录归属 P7 的 LLVM reachability/codegen residual。
  - 验证命令：`cargo fmt`；`cargo test -p scoopc --no-default-features hir`；`cargo test -p scoopc --no-default-features mir::materialize`；`cargo test -p scoopc --no-default-features monomorph`；`cargo run -p scoop -- test --fixtures tests/fixtures/mir_materialized`；搜索 `devirtualize_dispatch_calls` 无命中；搜索 `try_devirtualize_dispatch_target\(` 确认非测试活跃调用点只位于 MIR pass / helper / P7 residual；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
  - 残余风险：LLVM reachability / codegen 中的去虚化 residual 仍按 P7 backend cleanup 收口；本 review 未新增或依赖这些 backend fallback，也未推进 `P3-T07`。

## [DONE] P3-T07：P3 全包清场、文档同步与依赖审计

- 参考：
  - `PLAN.md` §4/P3、§5
  - `PIPELINE_REFACTOR.md` “MIR stage”“优化框架”“stage output wrapper 规则”
  - `PIPELINE-CLEANUP.md` P3/P4/P19 当前有效结论
- 目标：
  - 对 P3 全部改动做收口审计；
  - 确认 `MirStageOutput = { mir, mir_facts }` 语义成立；
  - 同步文档、fixtures、dependency gate 与 TODO 完成记录，明确剩余 P4/P5/P7 风险不属于 P3 未完成项。
- 必须检查和修改的主要位置：
  - `TODO-4.md`
  - `TODO.md`
  - `PLAN.md`（仅当阶段级计划实际变化）
  - `PIPELINE_REFACTOR.md` / `PIPELINE-CLEANUP.md` 中 P3 状态说明
  - `README.md` crate 概览
  - `tools/scoop_tools` dependency gate
  - `crates/scoopc/src/pipeline/{mir_stage.rs,effect_facts_stage.rs,effect_lowering_stage.rs,llvm_codegen_stage.rs}`
  - `crates/scoopc/src/mir/`
- 必须实现的内容：
  1. 全仓搜索并记录 P3 关键边界：`MirStageOutput` 不嵌套 HIR output，不把 root inventory / pass artifacts 留在并列 ad-hoc 字段中；`mir_facts` 是 MIR facts owner。
  2. 搜索 `materialized_pass_view()`、`MaterializedMirPassView`、`MaterializedMirPassArtifacts`、`try_devirtualize_dispatch_target(...)`、`devirtualize_dispatch_calls`，区分已清理、P4/P5/P7 过渡残留和测试命中。
  3. 更新 `PIPELINE-CLEANUP.md` P3 状态，把已解决项标记为历史，保留 P4/P5/P7 后续问题。
  4. 更新 `README.md` / dependency gate 文档，使 `scoopc_mir_facts` 和 MIR pass pipeline 角色可见。
  5. 运行 P3 包验收验证，并在完成记录中列出命令、结果和残余风险。
- 禁止事项：
  - 禁止把 P4/P5/P7 未做的 nested output/backend cleanup 伪装成 P3 已完成；必须准确记录后续风险。
  - 禁止更新 `PLAN.md` 作为普通执行日志；只有阶段级计划改变才允许修改。
  - 禁止保留 HIR devirtualization 或 MIR pass 双 owner 作为“后续再说”的 P3 残留。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc_mir_facts`
  3. `cargo test -p scoopc --no-default-features mir_stage`
  4. `cargo test -p scoopc --no-default-features effect_facts_stage`
  5. `cargo test -p scoopc --no-default-features effect_lowering_stage`
  6. `cargo run -p scoop_tools -- dependency-gate`
  7. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_lowered`
  8. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_materialized`
  9. `cargo clippy --all-targets -- -D warnings`
- 完成条件：
  - `MirStageOutput = { mir, mir_facts }` 语义成立；
  - MIR root inventories、snapshot binding、pass artifacts 和 MIR pass metadata 均有 MIR owner；
  - ordinary devirtualization / inlining / escape / closure simplification / refresh 由显式 MIR pass pipeline 调度；
  - P4/P5/P7 的剩余问题在文档中被准确保留，且不阻塞进入 `TODO-5.md`。
- 依赖：P3-T06R
- 完成记录：
  - 改动范围：复查 P3-T01 到 P3-T06R 的 MIR stage / MIR facts / pass pipeline 边界，更新 `README.md`、`PLAN.md` 当前状态、`PIPELINE_REFACTOR.md`、`PIPELINE-CLEANUP.md`、`tools/scoop_tools` dependency-gate help/docs、`TODO.md` / 本文件；同步修正 `effect_facts_stage` 与 `pipeline` 单测中仍按旧 3-step pass pipeline 断言的 stale expectation。
  - P3 边界审计：`MirStageOutput` 现在以 direct-style MIR + mandatory canonical materialized snapshot + `MirFacts` 形成 P4-ready handoff；root inventories、snapshot binding、instance/callable family inventory、pass artifact metadata 和 MIR pass pipeline metadata 均由 `scoopc_mir_facts` 发布。搜索 `callable_body_indices|initializer_root_indices|global_root_indices|metadata_root_indices` 在活跃 `crates/scoopc/src` Rust 源码中无命中。
  - pass / 去虚化审计：`devirtualize_dispatch_calls` 在活跃 Rust 源码中无命中；`try_devirtualize_dispatch_target(` 的非测试命中只剩 MIR pass owner / shared helper，以及已归属 P7 的 LLVM reachability/codegen residual。活跃 HIR 源码中无 `materialized_pass_view`、`MaterializedMir`、`MaterializedMirPassView` 或去虚化调用残留。
  - 文档与门禁：`PIPELINE-CLEANUP.md` / `PIPELINE_REFACTOR.md` 已把 P3 root/snapshot/pass owner 和显式 MIR pass pipeline 标记为 P3 收口结果，并保留 P4/P5/P7 的 nested output、effect facts mutability、LIR/backend handoff 和 backend devirtualization residual；`README.md` 与 dependency gate 文档已明确 `scoopc_mir_facts` 和 MIR pass pipeline metadata 角色。
  - 验证命令：`cargo fmt`；`cargo test -p scoopc_mir_facts`；`cargo test -p scoopc --no-default-features mir_stage`；`cargo test -p scoopc --no-default-features effect_facts_stage`；`cargo test -p scoopc --no-default-features effect_lowering_stage`；`cargo run -p scoop_tools -- dependency-gate`；`cargo run -p scoop -- test --fixtures tests/fixtures/mir_lowered`；`cargo run -p scoop -- test --fixtures tests/fixtures/mir_materialized`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
  - 残余风险：P4/P5/P7 仍需收口 `EffectFactsStageOutput` / `EffectLoweredStageOutput` 的 nested upstream bundle、effect facts stage 对 MIR snapshot type context 的可变扩展、LIR 输出自足性、LLVM HIR compatibility scaffold 和 backend 层去虚化 residual；这些已在文档中保留，不阻塞进入 `P3-T07R` review。

## [DONE] P3-T07R：Review P3 全包完成度

- 参考：P3-T07。
- 重点：
  - P3 是否真正满足 `PLAN.md` 的 MIR boundary + MIR pass pipeline 完成标准；
  - 是否还有 HIR 层 dispatch 去虚化、MIR pass 隐式 materializer 尾部调度、root inventory / pass artifacts 双 owner；
  - `TODO.md` / `TODO-4.md` / cleanup 文档是否准确反映 P3 完成和 P4/P5/P7 剩余边界。
- 必须复查的范围：
  - P3-T01 到 P3-T07 的全部改动
  - `crates/scoopc_mir_facts/`
  - `crates/scoopc/src/mir/`
  - `crates/scoopc/src/pipeline/`
  - `PIPELINE_REFACTOR.md`
  - `PIPELINE-CLEANUP.md`
  - `TODO.md` / `TODO-4.md`
  - `README.md`
- 验证：
  - 重新运行 P3-T07 的所有验证；
  - 如时间允许，运行 `cargo test --all --all-targets`；
  - 搜索 `devirtualize_dispatch_calls|callable_body_indices|initializer_root_indices|global_root_indices|metadata_root_indices|MissingMaterializedMirSnapshot`，确认无 P3 违规残留。
- 完成条件：
  - review 结论明确写出：P3 全包满足 MIR owner、output boundary 和 pass pipeline 约束，或列出阻塞项并在本 review 内修复。
- 依赖：P3-T07
- 完成记录：
  - 改动范围：复查 `P3-T01` 到 `P3-T07` 的 MIR stage / `scoopc_mir_facts` / pass pipeline / downstream handoff / 文档状态；review 过程中修复 `mir::escape` flow-insensitive alias propagation 的非收敛问题，并新增 `conflicting_alias_origins_converge_to_unknown` 回归测试，避免同一 local 从多个 closure/continuation origin 赋值时在 escape analysis fixpoint 中反复覆盖 alias。
  - review 结论：P3 全包满足 MIR owner、output boundary 和 pass pipeline 约束。`MirStageOutput` 不嵌套 HIR output，root inventories、snapshot binding、instance/callable family inventory、pass artifact metadata 和 pass pipeline metadata 均由 MIR stage / `MirFacts` 发布；ordinary devirtualization、inlining、escape analysis、closure simplification 和 summary/escape refresh 由显式 MIR pass pipeline 调度。
  - 搜索结论：`devirtualize_dispatch_calls|callable_body_indices|initializer_root_indices|global_root_indices|metadata_root_indices|MissingMaterializedMirSnapshot` 在活跃 Rust 源码中无命中；HIR 源码中无 `materialized_pass_view`、`MaterializedMirPassView` 或 `MaterializedMirPassArtifacts` 命中；`try_devirtualize_dispatch_target(` 的非测试活跃命中只剩 MIR pass owner / shared helper，以及已归属 P7 的 LLVM reachability/codegen residual。
  - 验证命令：`cargo fmt`；`cargo test -p scoopc_mir_facts`；`cargo test -p scoopc --no-default-features mir_stage`；`cargo test -p scoopc --no-default-features effect_facts_stage`；`cargo test -p scoopc --no-default-features effect_lowering_stage`；`cargo run -p scoop_tools -- dependency-gate`；`cargo run -p scoop -- test --fixtures tests/fixtures/mir_lowered`；`cargo run -p scoop -- test --fixtures tests/fixtures/mir_materialized`；`cargo clippy --all-targets -- -D warnings`；额外运行 `cargo test -p scoopc --no-default-features conflicting_alias_origins_converge_to_unknown`、`cargo test -p scoopc --no-default-features mir::escape`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_indirect_perform_nonresuming_function_value_higher_order_when_direct.scoop`、`cargo test --all --all-targets`。
  - 残余风险：P4/P5/P7 仍需收口 `EffectFactsStageOutput` / `EffectLoweredStageOutput` 的 nested upstream bundle、effect facts 对 materialized snapshot type context 的可变扩展、LIR 输出自足性、LLVM HIR compatibility scaffold 和 backend 层去虚化 residual；这些风险已在 P3 文档中保留，不阻塞进入 `TODO-5.md`。
