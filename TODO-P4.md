# TODO（P4：effect facts 与 `resolved_outward_cases` 分析落地）

> 生成时间：2026-05-02  
> 设计基线：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 前置条件：`TODO-P3.md` 已完整完成；refactor direct-style MIR stage 已存在；`Call / Perform / Resume / Handle` 的 typed contract、`SiteId`、CFG/cleanup/`finally` 已在 P3 中显式下沉到 MIR 层。  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 本阶段目标：在 refactor 新路径上建立完整的 `MaterializedEffectFacts`，让 `StepSchema` / `ContinuationSchema` / callable-block-site facts / `resolved_outward_cases` / `needs_reentry` / `impl_plan` 都在 MIR 之后显式化，并成为 P5 唯一允许消费的 effect 合同输入。

## 全局约束

- [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 是本阶段唯一设计基线；若实现过程中需要改动主张，必须先回写该文档，再继续实现。
- [`PLAN.md`](./PLAN.md) 与 [`TODO-P0.md`](./TODO-P0.md)、[`TODO-P1.md`](./TODO-P1.md)、[`TODO-P2.md`](./TODO-P2.md)、[`TODO-P3.md`](./TODO-P3.md) 是本阶段执行前提；P4 不得重新开启 P0-P3 已经收敛的 CLI / dispatcher / typed HIR / direct-style MIR contract 讨论。
- 本阶段只处理 refactor 新路径上的 effect facts 与 `resolved_outward_cases` 分析。
  - 明确禁止：在 P4 中实现 late-lowered `Step_F`、continuation object 物化、state-machine transformation、LLVM lowering；这些属于 P5/P6。
  - 明确禁止：在 P4 中回退到 HIR-driven `handle` 状态机规划，或直接调用当前 legacy `crates/scoopc/src/effect/state_machine/**`、`crates/scoopc/src/llvm/codegen/effect/**` 作为“已经有一套分析”的替代路线。
- P4 的 canonical 输入必须固定为 P3 的 refactor MIR stage 输出。
  - 允许消费：
    - 当前 canonical materialized MIR 快照；
    - 与其绑定的 `TypeStore`；
    - P3 已显式下沉到 MIR 的 typed contract / metadata；
    - 明确的外部输入：`Session`/opt level/feature flags/target ABI/预算参数。
  - 明确禁止：
    - 回 AST / HIR / typecheck 内部缓存补语义；
    - 重新读取 `hir::LoweredHir` 的私有 side table 再做二次解释；
    - 回 LLVM codegen 或 runtime bridge 查询 effect 语义。
- `MaterializedEffectFacts` 必须是**独立 side-table 子系统**。
  - 明确禁止：继续把新的 effect ABI/schema/type/site 字段塞进 `crates/scoopc/src/mir/summary.rs::InstanceSummary`；
  - 明确禁止：把 `MaterializedEffectFacts` 混入 `crates/scoopc/src/program_facts.rs::ProgramFacts`；
  - 明确禁止：把 refactor facts 直接附会到当前 `crates/scoopc/src/effect/analysis.rs::EffectAnalysisCtx` / `ContinuationEscapeFacts` 上当最终容器。
- `bool may_outward_effect` 在本阶段只能继续作为 legacy/通用优化摘要存在，不得再作为 refactor 新路径的 authoritative effect contract。
  - refactor 分析不得以 `InstanceSummary.may_outward_effect` 为主输入；
  - 若 legacy 路径仍保留它，可接受；但 refactor P4 结果必须完全由 `MaterializedEffectFacts` 表达。
- 本阶段必须继续遵守 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 顶部“闭包原则”。
  - P5 对 effect 语义的全部判断，只允许依赖：
    - P4 stage 输出的 `MaterializedEffectFacts`；
    - 与之绑定的 canonical MIR snapshot；
    - 显式外部输入（target ABI、opt level、feature flags）。
  - 因此 P4 必须把 `resume` 输入/输出 contract、`perform` payload tuple、`handle` 吸收集/arm/finally outward、dynamic `invoke(args_tuple)` 形状，全部显式落在 facts 或 schema 中。
- 本阶段对 `StepSchema` / `ContinuationSchema` / `CaseSet` / `ImplPlan` 的数据结构设计，必须直接面向最终形态。
  - 明确禁止：先做一版“轻量 bool summary”，以后再补 schema；
  - 明确禁止：把明显会在 P5 被推翻的临时字段形状写成阶段目标。
- `ConcreteOpKey` / case identity 必须至少区分到 generic-specialized concrete op。
  - 不能只用 effect 名、op 名或 bare FQN 字符串；
  - 若当前 P3 MIR 只持有 `op_fqn: String`，P4 必须补齐与 monomorphic concrete op 身份的稳定映射，而不是把字符串继续当最终 case 身份。
- runtime error 仍必须按普通 effect 分支建模。
  - `ContinuationAlreadyResumed` 等内部 runtime error，必须进入普通 `Raise<RuntimeError>` 等价 case；
  - 不能发明“runtime-error pseudo case”“hidden trap channel”或第四种 site fact 变体。
- 所有优化级别必须共用同一条 effect facts 管线。
  - `O0` / debug build 不允许切到单独的“debug effect analysis”通道；
  - 差异只能体现在预算、widening 时机、以及 `SingleCase` 是否允许。
- `MaterializedEffectFacts` 必须绑定到“当前 canonical materialized MIR snapshot”。
  - 若某个 pass 之后 body 发生结构性改写，则对应 `BodyEffectFacts` 必须重算；
  - 不能让后续阶段消费“部分 body 已过期、部分 body 已更新”的混合状态。
- 若当前 refactor 管线已经存在 pass 后 canonical callable view，则 facts 必须基于该 canonical snapshot 构建。
  - 允许复用 `crates/scoopc/src/mir/pass_view.rs` 提供的 canonical body 查询面；
  - 但必须在 P4 明确写死“究竟分析 raw `MaterializedMir.file` 还是 pass-view canonical body”，禁止两边混用。
- 本阶段不做 full regression。
  - 只做 effect facts 单元测试、dedicated dump/snapshot、以及必要的 refactor CLI smoke；
  - 不执行 `cargo test --all`；
  - 不执行 `cargo run -p scoop -- test` 的全量 fixture 扫描。
- 所有需要触发新路径的验证都必须通过 `--effect-pipeline refactor` 进入，或通过与该 CLI 路径共用同一 stage helper 的 Rust 测试入口进入；禁止新增只在测试中存在的语义旁路。

## [DONE] P4-T01：建立 refactor effect-facts stage 与独立 side-table 子系统边界

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P4
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.13.1a, §5.4.8, §8
  - 当前实现参考：`crates/scoopc/src/effect/mod.rs`、`crates/scoopc/src/effect/analysis.rs`、`crates/scoopc/src/program_facts.rs`、`crates/scoopc/src/mir/pass_view.rs`
- 目标：
  - 在 refactor 新路径上建立一个显式的 effect-facts stage；
  - 同时把新的 facts 子系统从 legacy `effect/analysis`、`ProgramFacts`、`InstanceSummary` 中彻底分离出来，避免后续实现继续混线。

- 必须实现的内容：
  1. 在 `scoopc` 中新增一个专门承载 P4 facts 子系统的模块树。
     - 推荐位置：`crates/scoopc/src/effect_facts/`；
     - 推荐最小模块拆分：
       - `mod.rs`
       - `schema.rs`
       - `facts.rs`
       - `builder.rs`
       - `solver.rs`
       - `dump.rs`（若最终需要稳定 formatter）
     - 若采用等价拆分也可，但必须保证：
       - 新 facts 子系统不与 legacy `crates/scoopc/src/effect/state_machine/**` 混在一起；
       - 模块命名足够清晰，让后续 P5 能直接在此基础上继续推进。
  2. 在 `crates/scoopc/src/lib.rs` 中为该子系统建立模块入口；
     - 要求：其 API 服务 refactor pipeline 与后续 stage，而不是只给某个测试使用。
  3. 在 refactor pipeline 下新增 effect-facts stage 模块。
     - 推荐位置：`crates/scoopc/src/effect_refactor_pipeline/effect_facts_stage.rs`；
     - 它必须属于 refactor pipeline 的显式阶段入口，而不是在 legacy `effect/analysis.rs` 或 `mir/pass_view.rs` 内再加 pipeline 分支。
  4. 定义一个 refactor effect-facts stage 输出类型。
     - 名称可自定，例如 `RefactorEffectFactsStageOutput`；
     - 该输出必须至少承载：
       - 当前 canonical MIR snapshot 的查询面；
       - 与之绑定的 `TypeStore`；
       - 最终 `MaterializedEffectFacts`；
       - 若 facts 需要附带 formatter/debug view，则提供稳定 formatter API。
     - 注释中必须明确写出：
       - 该 stage 的输入是 P3 MIR stage 输出；
       - 该 stage 的输出是 P5 唯一允许消费的 effect contract；
       - P5 不得再回 HIR/typecheck 推断缺失语义。
  5. 明确 facts 所绑定的 MIR snapshot 来源。
     - 若 refactor pipeline 已有 `MaterializedMir::pass_view()` 形式的 canonical pass 视图，则 P4 必须统一基于该 canonical body/summaries 查询面；
     - 若当前仍只有 raw `MaterializedMir.file` 可用，则必须在本任务中把“raw vs canonical pass body”的边界写清楚，并确保整个 P4 都只用其中一种；
     - 禁止在同一个 facts 构建过程中同时混用 raw body 和 pass-view body。
  6. 明确 facts 生命周期与失效规则。
     - 至少要写清：
       - facts 与当前 materialized MIR snapshot 一一对应；
       - 结构性 rewrite 后必须重算受影响 body 的 facts；
       - stage 输出不允许对外暴露“已知部分过期”的容器。
  7. 如果当前 refactor stage 仍依赖 `ProgramFacts`、`EffectAnalysisCtx`、`ContinuationEscapeFacts` 作为 effect contract 主输入，必须在本任务中改为：
     - 只把这些旧设施当作可选精度输入或 legacy 保留模块；
     - 不允许让它们继续承载 authoritative facts。

- 必须遵从的约束：
  - 禁止把新的 facts 字段塞进 `mir::summary::InstanceSummary`。
  - 禁止把新的 facts 容器塞进 `ProgramFacts` 或 `EffectAnalysisCtx`。
  - 禁止把 refactor effect facts stage 写成“LLVM codegen 调试 helper”；它必须属于 compiler middle-end 正式阶段。
  - 禁止在 `crates/scoopc/src/effect/state_machine/**` 里直接长出 P4 的新主线逻辑。

- 验证：
  1. 新增/更新单元测试，推荐命名：`refactor_effect_facts_stage_*`，至少覆盖：
     - effect-facts stage 输出类型可构造；
     - 该 stage 显式接受 P3 MIR stage 输出；
     - stage 输出显式携带 `MaterializedEffectFacts`，而不是隐式散落在别处。
  2. 运行：
      - `cargo test -p scoopc --no-default-features refactor_effect_facts_stage`
  3. 若需要 smoke 上游输入可用性，可额外运行：
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/dispatch_and_resume_call.scoop`

- 完成条件：
  - refactor 新路径已拥有独立的 effect-facts stage；
  - `MaterializedEffectFacts` 子系统与 legacy `effect/analysis` / `ProgramFacts` / `InstanceSummary` 已明确分离；
  - P5 之后的阶段已有明确的 facts stage 输出可接。
- 依赖：`TODO-P3.md` 最后一项 review 完成
- 完成记录：
  - 2026-05-02：完成 `P4-T01`，新增独立的 `crates/scoopc/src/effect_facts/` 子系统与 `crates/scoopc/src/effect_refactor_pipeline/effect_facts_stage.rs`，为 refactor 新路径建立显式的 effect-facts stage，并定义 `RefactorEffectFactsStageOutput` 作为 P4 -> P5 的稳定 handoff 结构。
  - `crates/scoopc/src/effect_facts/{mod,builder,dump,facts,schema,solver}.rs` 现已把 P4 子系统边界固定为独立模块树：`facts.rs` 定义 `MaterializedEffectFacts` / `MirSnapshotBinding` / callable/body facts 外壳，`schema.rs` 提供 `StepSchemaId` / `ContinuationSchemaId` 与 schema 外壳，`builder.rs` 固定“从 canonical materialized MIR snapshot 建 facts 容器外壳”的入口，`solver.rs` 固定后续 `resolved_outward_cases` 求解将落入的 solver 边界，`dump.rs` 提供稳定 formatter。
  - `RefactorEffectFactsStageOutput` 已显式承载：P3 的 `RefactorMirStageOutput`、与之绑定的 `TypeStore`、canonical `MaterializedMir::pass_view()` 查询面，以及最终 `MaterializedEffectFacts`。其注释明确写死：P4 输入必须是 P3 MIR stage 输出、`materialized_pass_view()` 是当前 canonical MIR snapshot 的唯一查询面、`effect_facts()` 是 P5 唯一允许消费的 authoritative effect contract、MIR snapshot 结构性改写后必须重跑本 stage。
  - 为保证 dump/refactor 路径在尚未把 materialized snapshot 直接挂进 P3 输出时也能稳定进入 P4，`crates/scoopc/src/effect_refactor_pipeline/mod.rs` 现在会在 effect-facts stage 边界使用同一 `session + source` 路由补挂 canonical `MaterializedMir` snapshot；该补挂只发生在 stage wrapper，`dump-mir` 本身不因此额外依赖 materialization 成功。
  - P4 现已明确只绑定 pass-view canonical MIR 查询面：`MaterializedEffectFactsBuilder::from_materialized_snapshot(...)` 只消费 `MaterializedMir::pass_view()`，并用 `MirSnapshotBinding` 记录 query surface、instance 计数与 canonical body FQN 集；不会在同一个 facts 构建过程中混用 raw `MaterializedMir.file` 与 pass-view body/summaries。
  - 搜索摘要：执行 `MaterializedEffectFacts|StepSchema|ContinuationSchema|resolved_outward_cases|impl_plan` 搜索后，命中仅位于新的 `effect_facts` 子系统、`effect_facts_stage.rs` 与 `mir_stage.rs` 的 handoff 注释；未发现这些新 facts 术语被塞进 `crates/scoopc/src/program_facts.rs`、`crates/scoopc/src/mir/summary.rs` 或 `crates/scoopc/src/effect/analysis.rs` 的业务实现中。
  - 新增/更新测试：`crates/scoopc/src/effect_refactor_pipeline/effect_facts_stage.rs` 中的 `refactor_effect_facts_stage_output_is_constructible`、`refactor_effect_facts_stage_explicitly_consumes_p3_mir_stage_output`、`refactor_effect_facts_stage_requires_materialized_snapshot`，以及 `crates/scoopc/src/effect_refactor_pipeline/mod.rs` 中的 `refactor_effect_facts_stage_dispatcher_loads_stage_output`。
  - 2026-05-02：按详细任务文件的完成判定规则复验后，已补齐本任务标题的 `[DONE]` 标记，并同步更新 `TODO.md` 索引；`PLAN.md` 仍无需改动。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_effect_facts_stage`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/dispatch_and_resume_call.scoop`、`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`。

## [DONE] P4-T01R：Review facts stage 边界，确认没有把新 facts 混进 legacy `effect` / `summary` / `ProgramFacts`

- 参考：
  - [`PLAN.md`](./PLAN.md) §0，§2/P4
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.13.1a, §5.4.8
- 重点：
  - 新的 facts 子系统是否已经成为独立模块；
  - refactor effect-facts stage 是否真的以 P3 MIR stage 输出为输入；
  - 是否仍然避免把新的 effect contract 混写到 legacy `effect/analysis.rs`、`ProgramFacts`、`InstanceSummary`。
- 必须检查的文件/位置：
  - 新增的 `crates/scoopc/src/effect_facts/**`
  - 新增的 `crates/scoopc/src/effect_refactor_pipeline/effect_facts_stage.rs`
  - `crates/scoopc/src/lib.rs`
  - `crates/scoopc/src/program_facts.rs`
  - `crates/scoopc/src/mir/summary.rs`
  - `crates/scoopc/src/effect/analysis.rs`

- 验证：
  - 重新运行 P4-T01 的全部测试与命令；
  - 额外搜索：
    - `rg "MaterializedEffectFacts|StepSchema|ContinuationSchema|resolved_outward_cases|impl_plan" crates/scoopc/src`
  - 要求：
    - 允许命中：新 `effect_facts` 子系统、refactor stage、测试、注释；
    - 不允许命中：把这些字段直接塞进 `mir/summary.rs::InstanceSummary`、`program_facts.rs::ProgramFacts`、`effect/analysis.rs::EffectAnalysisCtx` 的业务实现。

- 完成条件：
  - review 能明确说明：P4 facts stage 与 legacy 设施已经分离，后续任务不会靠临时混线推进；
  - 可进入 P4-T02。
- 依赖：P4-T01
- 完成记录：
  - 2026-05-02：完成 `P4-T01R` review，未发现需要在 `P4-T02` 前补入的新前置缺陷；最近一次提交 `[P4-T01] Add refactor effect-facts stage boundary` 与本 review 直接相关，但未显式留下需要追加跟踪的未完成事项。
  - facts stage / 子系统边界复核结论：`crates/scoopc/src/effect_facts/{mod,builder,dump,facts,schema,solver}.rs` 已形成独立的 P4 facts 模块树，`crates/scoopc/src/lib.rs` 也已把 `effect_facts` 暴露为正式模块入口；`crates/scoopc/src/effect_refactor_pipeline/effect_facts_stage.rs` 中 `RefactorEffectFactsStageOutput` 继续以 `RefactorMirStageOutput` 为显式输入，并把 `MaterializedMir::pass_view()` 固定为当前 canonical MIR 查询面，同时将 `MaterializedEffectFacts` 收口为 P5 唯一允许消费的 authoritative effect contract。
  - legacy 容器隔离复核结论：`crates/scoopc/src/program_facts.rs` 仍只承载旧的 HIR/program facts side tables，`crates/scoopc/src/mir/summary.rs` 仍只维护 `InstanceSummary` / `may_outward_effect` 等 legacy MIR summary，`crates/scoopc/src/effect/analysis.rs` 仍只承载 legacy/shared effect analysis context；未发现把 `MaterializedEffectFacts`、`StepSchema`、`ContinuationSchema`、`resolved_outward_cases`、`impl_plan` 直接塞进这些 legacy 容器或业务实现的情况。
  - 搜索摘要：执行 `rg "MaterializedEffectFacts|StepSchema|ContinuationSchema|resolved_outward_cases|impl_plan" crates/scoopc/src` 后，命中集中在新的 `effect_facts` 子系统、`effect_refactor_pipeline/effect_facts_stage.rs`，以及 `mir_stage.rs` 中说明 P3 尚未提供这些 P4 产物的 handoff 注释；`program_facts.rs`、`mir/summary.rs`、`effect/analysis.rs` 无相关命中，说明新 facts 术语尚未渗入 legacy 容器。
  - 复验通过：`cargo test -p scoopc --no-default-features refactor_effect_facts_stage`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/dispatch_and_resume_call.scoop`、`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`。
  - 2026-05-02：本次按“标题必须显式带 `[DONE]` 才算完成”的规则复核后，重新检查了 `effect_facts` 模块树、`effect_facts_stage` handoff 以及 legacy 容器隔离状态；未发现会阻塞 `P4-T02` 的新问题，现补齐本任务标题的 `[DONE]` 标记并与 `TODO.md` 索引同步。

## [DONE] P4-T02：落地 schema identity、canonical schema pool 与 callable-level facts 壳层

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P4
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.3.3-§4.3.4, §5.4.1-§5.4.4, §5.3.9
  - 当前实现参考：`crates/scoopc/src/mir/materialize.rs::InstanceKey`、P3 direct-style MIR metadata、P2 typed continuation/effect contract
- 目标：
  - 建立 P4 需要的 canonical schema identity 与 schema pool；
  - 让 callable-level facts 拥有最终目标形状，不再依赖 `may_outward_effect` 这类 legacy 摘要。

- 必须实现的内容：
  1. 定义 schema identity / case identity 新类型。
     - 必须至少包含：
       - `StepSchemaId`
       - `ContinuationSchemaId`
       - `CaseTag`
       - `ConcreteOpKey`
       - `CaseSet`
       - `ImplPlan = NoOutward | SingleCase(CaseTag) | CanonicalFull`
     - 推荐位置：`crates/scoopc/src/effect_facts/schema.rs`。
  2. 明确 effect facts 使用的 callable 身份键。
     - 首选：直接复用当前 `crates/scoopc/src/mir/materialize.rs::InstanceKey`；
     - 但在真正复用前，必须先验证它是否已经唯一标识 `(symbol, type_args, allowed_row)` 语义；
     - 若当前 `InstanceKey` 可能让不同 `allowed_row` 的 surface 实例冲突，则必须在本任务中新增 refactor facts 专用 key 包装或扩展键，而不是静默复用错误键。
  3. 定义并构建 `StepCaseFact`、`StepSchema`、`ContinuationSchema`。
     - `StepSchema` 必须至少包含：
       - `invoke_args_tuple_ty`
       - `complete_ty`
       - `continuation_obj_ty`
       - `cases: [StepCaseFact]`
     - `ContinuationSchema` 必须至少包含：
       - `resume_tuple_ty`
       - `answer_ty`
       - `out_step_schema`
       - `surface_ty`
     - `StepCaseFact` 必须至少包含：
       - `case_tag`
       - `concrete_op_key`
       - `payload_tuple_ty`
       - `continuation_schema`
  4. 明确 `concrete_op_key` 的真实来源。
     - 若当前 P3 MIR / typed contract 已能拿到 generic-specialized concrete op identity，则直接使用；
     - 若当前只剩 `op_fqn: String`，必须在本任务中补齐一个稳定的 concrete-op identity 映射，最终 facts 不得继续以 bare FQN 作为 case 身份；
     - 允许底层复用 monomorphic callable identity，但对外 API 必须使用语义 newtype `ConcreteOpKey`，不能直接裸露 `InstanceKey`。
  5. 明确 `continuation_obj_ty` 的 contract。
     - 它在 P4 必须已经是一个稳定、可比较、可 dump 的编译器内部 continuation 对象类型身份；
     - 但它不要求在 P4 具备物理布局；
     - 明确禁止：把它留成 `Any`、`Todo`、字符串占位，或“等 P5 再说”的空壳。
  6. 把 runtime error ordinary effect 纳入 schema。
     - `ContinuationAlreadyResumed` 等路径必须对应普通 concrete `Raise<RuntimeError>` 等价 case；
     - 不能在 schema 中另发明“runtime error special case”。
  7. 明确 case/tag 排序与稳定性。
     - `cases` 必须按稳定顺序存储；
     - `CaseTag` 编号对同一 `StepSchema(F)` 固定；
     - `CaseTag` 不因 `resolved_outward_cases` / `impl_plan` 子集变化而重新编号。
  8. 定义 `CallableEffectFacts` 的最终目标字段形状。
     - 必须至少包含：
       - `declared_row`
       - `invoke_args_tuple_ty`
       - `step_schema`
       - `resolved_outward_cases`
       - `needs_reentry`
       - `impl_plan`
     - 若本任务尚未做全量求解，允许在 builder 内部先以保守值初始化；
     - 但 public facts 结构形状必须已经与设计文档一致，不能新增“以后会删掉的临时 bool/enum”。
  9. 明确 `()` / `Unit` 在 schema 中的表示。
     - `payload_tuple_ty == ()` 与 `resume_tuple_ty == ()` 在 facts 中仍需显式记录；
     - 不允许因为后端最终可能零载荷而在 P4 facts 中省略这些类型字段。

- 必须遵从的约束：
  - 禁止继续使用 bare `String` / FQN 作为最终 case identity。
  - 禁止把 `StepSchema` / `ContinuationSchema` 的关键字段推迟到 P5 再补。
  - 禁止通过 `may_outward_effect`、函数是否 effectful、或单个布尔值替代 `CaseSet` / schema。
  - 禁止跨不同 `allowed_row` 偷偷共享 callable facts 或 schema identity。

- 验证：
  1. 新增/更新单元测试，推荐命名：
     - `refactor_effect_schema_*`
     - `refactor_continuation_schema_*`
     - `refactor_callable_effect_facts_shell_*`
  2. 测试至少覆盖：
     - `CaseTag` 稳定编号；
     - `ConcreteOpKey` 能区分 generic-specialized concrete op；
     - `payload_tuple_ty` / `resume_tuple_ty` 在 `()` 场景下仍显式记录；
     - runtime error ordinary effect 会进入普通 schema case；
     - `invoke_args_tuple_ty` / `surface_ty` / `continuation_obj_ty` 都已可见。
  3. 运行：
      - `cargo test -p scoopc --no-default-features refactor_effect_schema`
      - `cargo test -p scoopc --no-default-features refactor_continuation_schema`

- 完成条件：
  - canonical schema pool 与 callable-level facts 目标形状已经落地；
  - P4 后续任务可以直接在这些 schema / callable facts 之上继续构建 block/site facts 与求解器；
  - 新路径不再依赖 `may_outward_effect` 这类 legacy 摘要表达 effect contract。
- 依赖：P4-T01R
- 完成记录：
  - 2026-05-02：完成 `P4-T02`，在 `crates/scoopc/src/effect_facts/schema.rs` / `facts.rs` / `builder.rs` 中把 `StepSchemaId`、`ContinuationSchemaId`、`CaseTag`、`ConcreteOpKey`、`CaseSet`、`ImplPlan`、`StepCaseFact`、`StepSchema`、`ContinuationSchema` 与 `CallableEffectFacts` 的目标 public 形状全部落地；`CallableEffectFacts` 现已显式承载 `declared_row`、`invoke_args_tuple_ty`、`step_schema`、`resolved_outward_cases`、`needs_reentry`、`impl_plan`，不再依赖 `may_outward_effect` 这类 legacy bool 摘要表达新路径 contract。
  - callable-level identity 复用 `mir/materialize.rs::InstanceKey` 作为事实主键，并新增验证 `refactor_callable_effect_facts_shell_instance_keys_distinguish_allowed_rows`，确认同一 callable 在不同 `allowed_row` 下不会共享 key；effect/case 相关 API 继续只通过语义 newtype `ConcreteOpKey(InstanceKey)` 暴露 concrete op identity，而不直接裸露 `InstanceKey` 作为 case identity。
  - `MaterializedEffectFactsBuilder` 现在会稳定构建 canonical schema pool：按稳定顺序为每个 callable instance 分配 `StepSchemaId`，为每个 case 生成固定 `CaseTag`，并把 `payload_tuple_ty` / `resume_tuple_ty` / `answer_ty` / `surface_ty` / `continuation_obj_ty` 显式写入 schema；其中 `payload_tuple_ty == ()` 与 `resume_tuple_ty == ()` 仍按显式 `Unit` 记录，未因后续可能零载荷而在 P4 省略。
  - `continuation_obj_ty` 已改为绑定完整 `InstanceKey`（模板位置、type args、effect args），不再仅按 `root_fqn` 生成内部 continuation 对象类型身份；新增 `refactor_continuation_schema_identity_distinguishes_callable_instances` 覆盖同名不同实例的内部类型身份区分。
  - builder 现已显式跳过两类不应进入 callable-level facts 壳层的 surface 声明：effect-op 根声明，以及 compiler-owned `scoop.core.Continuation.resume` surface 方法；前者继续只作为 `ConcreteOpKey` / case contract 的来源，后者继续只通过 P2/P3 下沉到 MIR 的 `CallKind::Resume` metadata 与 continuation schema 建模，避免把这些 surface 声明误当成普通 callable shell 分析对象。
  - runtime error ordinary effect 已进入 schema：`resumeZero`/`Continuation.resume` 路径会把 `Raise<RuntimeError>` 作为普通 concrete case 写入 `StepSchema`，未引入额外 pseudo case；`resolved_outward_cases` / `needs_reentry` / `impl_plan` 当前按保守规则由 builder 初始化，供后续 P4-T03/P4-T04 在同一结构形状上继续细化。
  - 新增/更新测试：`refactor_effect_schema_case_tags_are_stable_and_distinguish_generic_specialized_raise_cases`、`refactor_continuation_schema_explicitly_records_unit_payload_resume_and_surface_type`、`refactor_continuation_schema_identity_distinguishes_callable_instances`、`refactor_callable_effect_facts_shell_uses_final_shape_and_runtime_error_case`、`refactor_callable_effect_facts_shell_instance_keys_distinguish_allowed_rows`、`refactor_callable_effect_facts_shell_skips_effect_op_roots`，并把 sample fixture 改为“generic helper + driver 触发 materialized direct-call instances”的形状，确保测试覆盖真实 materialized callable instance 而不是只落在 surface builtin。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_effect_schema`、`cargo test -p scoopc --no-default-features refactor_continuation_schema`、`cargo test -p scoopc --no-default-features refactor_callable_effect_facts_shell`、`cargo test -p scoopc --no-default-features refactor_effect_facts_stage`、`cargo test -p scoopc --no-default-features materialized_effect_facts_builder_uses_canonical_pass_view_snapshot`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/dispatch_and_resume_call.scoop`、`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`。
  - 2026-05-02：本任务未引入新的阶段依赖或顺序变化，因此 `PLAN.md` 无需改动；同时已把 `TODO.md` 中对应索引条目标记同步为 `[DONE]`。

## [DONE] P4-T02R：Review schema pool 与 callable facts，确认 identity 和 case contract 已经固定

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.3.3-§4.3.4, §5.4.1-§5.4.4
  - [`PLAN.md`](./PLAN.md) §2/P4
- 重点：
  - `StepSchemaId` / `ContinuationSchemaId` / `CaseTag` / `ConcreteOpKey` 是否已经落地为稳定 identity；
  - callable key 是否已正确区分 `(symbol, type_args, allowed_row)`；
  - runtime error 是否作为普通 concrete case 进入 schema；
  - `CallableEffectFacts` 是否已经具有最终字段形状，而不是临时 bool summary。
- 必须检查的文件/位置：
  - 新增的 `crates/scoopc/src/effect_facts/schema.rs`
  - 新增的 `crates/scoopc/src/effect_facts/facts.rs`
  - 与 concrete-op 身份映射相关的 builder 位置
  - `crates/scoopc/src/mir/materialize.rs::InstanceKey`

- 验证：
  - 重新运行 P4-T02 的全部测试与命令；
  - 额外搜索：
    - `rg "may_outward_effect|op_fqn: String|Todo\(|Any" crates/scoopc/src/effect_facts crates/scoopc/src/effect_refactor_pipeline`
  - 要求：
    - 允许命中：legacy 模块、测试、注释；
    - 不允许命中：新的 schema/facts 主实现把这些作为最终 contract 使用。

- 完成条件：
  - review 能明确说明：P4 的 schema/case identity 已经固定，后续任务不会再因 identity 不足回去补 HIR/字符串推断；
  - 可进入 P4-T03。
- 依赖：P4-T02
- 完成记录：
  - 2026-05-02：完成 `P4-T02R` review。最近一次提交 `[P4-T02] Materialize effect schema pool and callable facts` 与本次复核直接相关，但提交信息与当前代码状态中均未发现需要在 `P4-T03` 前单独插入的新前置问题。
  - identity 固定性复核结论：`crates/scoopc/src/effect_facts/builder.rs` 继续以 `MaterializedMir::pass_view()` 的 canonical snapshot 为输入，并按 `pass_view.instances()` 的稳定 `instance_keys` 顺序为 callable 分配 `StepSchemaId`；`crates/scoopc/src/mir/pass_view.rs` 已显式注明该遍历顺序稳定，而 `crates/scoopc/src/mir/materialize.rs` 会先按 `instance_fqn` 排序生成 `instance_keys`，且 `instance_fqn` 同时编码 `type_args` 与 `eff_args`，因此 callable identity 能稳定区分 `(symbol, type_args, allowed_row)`。
  - case contract 复核结论：`CaseTag` 继续基于 effect term + concrete op sort key 的稳定排序分配，`ConcreteOpKey` 仍以语义 newtype 包裹 `InstanceKey` 表达 generic-specialized concrete op identity，`ContinuationSchemaKey` 继续把 `resume_tuple_ty`、`answer_ty`、`out_step_schema`、`surface_ty` 一并纳入 identity；后续任务无需回退到 bare FQN 或字符串推断补 identity。
  - schema/facts 形状复核结论：`CallableEffectFacts` 仍显式承载 `declared_row`、`invoke_args_tuple_ty`、`step_schema`、`resolved_outward_cases`、`needs_reentry`、`impl_plan`，未退化回 `may_outward_effect` 之类的临时 bool 摘要；`resumeZero` / `Continuation.resume` 的 runtime error 路径仍通过普通 `Raise<RuntimeError>` concrete case 进入 `StepSchema`，没有额外 pseudo case。
  - 搜索摘要：执行 `rg "may_outward_effect|op_fqn: String|Todo\(|Any" crates/scoopc/src/effect_facts crates/scoopc/src/effect_refactor_pipeline` 后，`effect_facts` 侧仅命中 `crates/scoopc/src/effect_facts/mod.rs` 中 `MalformedEffectOpSignature` 的错误载荷字段 `op_fqn: String`；`effect_refactor_pipeline` 侧命中仅位于 `hir_stage.rs` 的 typed HIR contract 字段与 AST `Todo` 分支；未发现 `schema.rs`、`facts.rs`、`builder.rs` 等 schema/facts 主实现把这些占位形状当作最终 contract 使用。
  - 复验通过：`cargo test -p scoopc --no-default-features refactor_effect_schema`、`cargo test -p scoopc --no-default-features refactor_continuation_schema`、`cargo test -p scoopc --no-default-features refactor_callable_effect_facts_shell`、`cargo test -p scoopc --no-default-features refactor_effect_facts_stage`、`cargo test -p scoopc --no-default-features materialized_effect_facts_builder_uses_canonical_pass_view_snapshot`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/dispatch_and_resume_call.scoop`、`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`。
  - 2026-05-02：本次 review 未改变阶段顺序或依赖，`PLAN.md` 无需改动；现已补齐本任务标题的 `[DONE]` 标记，并同步更新 `TODO.md` 索引。

## [DONE] P4-T02a：修复 canonical materialized MIR pass-view 对普通非泛型 callable body 的发布，确保 P4 能在稳定 `InstanceKey` 键空间上看到 request-root / caller body

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P3，§2/P4
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.4.6-§5.4.8，§8
  - 当前实现参考：`crates/scoopc/src/mir/materialize.rs`、`crates/scoopc/src/mir/callables.rs`、`crates/scoopc/src/mir/pass_view.rs`、`crates/scoopc/src/effect_refactor_pipeline/mir_stage.rs`
- 目标：
  - 修复 P3/P4 handoff 的 canonical materialized MIR 查询面；
  - 让普通非泛型 request-root / caller body 也能通过 `MaterializedMir::pass_view()` 以稳定 `InstanceKey -> root callable / callable family` 形式被 P4 看到，而不是只在 raw MIR / caller-candidate 旁路里存在。

- 必须实现的内容：
  1. 精确定位并修复当前 canonical publication 缺口。
     - 现象：对 `dispatch_and_resume_call`、`handle_perform`、`handle_finally_boundary` 一类普通非泛型样本，refactor `dump-mir` 已能产出 direct-style MIR body，但当前 `MaterializedMir::pass_view().instances()` / `MaterializedEffectFactsBuilder::collect_callable_seeds(...)` 仍可能得到空集合；
     - 必须找出究竟是 `materialize` 阶段没有把这些 body 放进 instance family、还是 `pass_view` / `callables` 查询面对它们做了过滤。
  2. 修复位置必须在 canonical MIR snapshot / pass-view 发布层，而不是在 P4 facts builder 里临时合成 fallback 键。
     - 明确禁止：让 `P4-T03` 通过扫描 raw `MaterializedMir.file` 或 `caller_side_pass_candidate_bodies()` 自己造一套“像 `InstanceKey` 的键空间”来绕过问题；
     - 正确结果应当是：这些 ordinary callable body 本来就属于 P3 交给 P4 的 canonical MIR handoff，因此必须在 `pass_view` / family 映射层被正式发布。
  3. 修复后必须保证以下 canonical 查询成立：
     - `pass_view().instances()` 能枚举普通非泛型 root/caller body；
     - `owner_of_callable(fqn)` 能稳定返回对应 `InstanceKey`；
     - `root_body()` / `callable_bodies()` 对这些 ordinary callable 不再返回空；
     - 若后续 MIR pass override/rehome 这些 body，`pass_view` 仍维持单一 canonical owner。
  4. 新增/更新定向测试，至少覆盖：
     - 一个仅含普通非泛型 direct call 的源文件；
     - 一个含 `dispatch` / `resume` 的普通非泛型源文件；
     - 一个含 `handle` / `perform` / `finally` 的普通非泛型源文件；
     - 要求测试直接断言 `pass_view` / family / owner mapping 非空且可查询，而不是只看 `dump-mir` stdout。

- 必须遵从的约束：
  - 禁止把 ordinary non-generic body 继续留在 raw MIR / caller-candidate 旁路，再让 P4/P5 自己猜“哪些才是 canonical callable”；
  - 禁止把这个问题推迟到 `P4-T03` 用 facts builder 特判修补；
  - 禁止为了让测试过掉而改写成只覆盖 generic 样本或只覆盖已有 instance family 的更窄 shape。

- 验证：
  1. 新增/更新测试，推荐命名：
     - `materialized_pass_view_non_generic_*`
     - `refactor_effect_facts_stage_non_generic_*`
  2. 运行：
      - `cargo test -p scoopc --no-default-features materialized_pass_view_non_generic`
      - `cargo test -p scoopc --no-default-features refactor_effect_facts_stage_non_generic`
      - `cargo test -p scoopc --no-default-features refactor_effect_facts_stage`

- 完成条件：
  - canonical `MaterializedMir::pass_view()` 已正式发布普通非泛型 callable body；
  - P4 之后的阶段可以继续只消费 `InstanceKey` / family / pass-view body，而无需回 raw MIR 旁路补 owner identity；
  - `P4-T03` 可以在不 workaround 的前提下直接构建 `BodyEffectFacts` / `SiteEffectFacts`。
- 依赖：P4-T02R
- 完成记录：
  - 2026-05-02：完成 `P4-T02a`。`crates/scoopc/src/mir/materialize.rs` 现在会把 request-root 可达的 ordinary non-generic callable 作为 canonical pass-view 初始发布的一部分收集到 `pass_published_ordinary_callables`，并在 materialization 收口时为它们构建稳定的 `InstanceKey`、pass-visible family、summary 与 body 映射；这些 callable 不再只停留在 raw MIR / `caller_side_pass_candidate_bodies()` 旁路里。
  - `crates/scoopc/src/mir/pass_view.rs` 现已把 pass-view 的可见实例顺序绑定到 `MaterializedMirPassArtifacts` 自己维护的 `instance_keys`，而不是 raw materialized `instance_keys`；`replace_callable_family(...)` 也会同步确保新 family 的 owner 键可见。因此 `pass_view().instances()`、`owner_of_callable()`、`root_body()`、`callable_bodies()` 现在都能稳定命中 ordinary non-generic root/caller body，并在后续 pass override/rehome 时继续维持单一 canonical owner。
  - `crates/scoopc/src/effect_facts/builder.rs` 已删除对 raw `MirFile` root 声明的 fallback 读取，`collect_callable_seeds(...)` 现在直接要求 `MaterializedMir::pass_view()` 提供 canonical root/body；这把 `P4-T03` 所需的 callable identity 完整收口回 P3/P4 的 authoritative handoff，而不是继续在 facts builder 里自造第二套键空间。
  - 新增/更新验证覆盖：`crates/scoopc/src/mir/pass_view.rs` 中 `materialized_pass_view_non_generic_*` 直接断言普通 non-generic direct-call / dispatch-resume / handle-finally 样本已进入 canonical owner/family/root-body 查询面；`crates/scoopc/src/effect_refactor_pipeline/effect_facts_stage.rs` 中 `refactor_effect_facts_stage_non_generic_*` 继续断言 P4 stage 输出使用这些 canonical `InstanceKey` 发布 ordinary callable facts/body；`crates/scoopc/src/mir/inline.rs` 与 `crates/scoopc/src/llvm/tests.rs` 也已同步更新为匹配“ordinary non-generic body 默认属于 canonical pass-view”的新约束。
  - 为满足本轮“无 lint 告警”要求，`crates/scoopc/src/effect_refactor_pipeline/mod.rs::emit_production_llvm_artifact_to_file` 已按仓库既有风格补充局部 `#[allow(clippy::too_many_arguments)]`，消除与本任务无关但会阻塞 `cargo clippy -D warnings` 的既有告警。
  - 验证通过：`cargo test -p scoopc --no-default-features materialized_pass_view_non_generic`、`cargo test -p scoopc --no-default-features refactor_effect_facts_stage_non_generic`、`cargo test -p scoopc --no-default-features refactor_effect_facts_stage`、`cargo test -p scoopc --no-default-features caller_side_inlining_keeps_non_generic_pass_roots_visible`、`cargo test -p scoopc production_codegen_observes_caller_side_mir_inlining_for_non_generic_body`、`cargo clippy -p scoopc --all-targets --no-default-features -- -D warnings`、`cargo clippy -p scoopc --all-targets -- -D warnings`。

## [DONE] P4-T02aR：Review canonical pass-view 对 ordinary callable body 的发布结果，确认 P4 不再需要 raw/fallback 键空间

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P3，§2/P4
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.4.6-§5.4.8，§8
- 重点：
  - 普通非泛型 request-root / caller body 是否已进入 canonical `pass_view` / family 映射；
  - `owner_of_callable` / `root_body` / `callable_bodies` 是否都能稳定命中这些 ordinary callable；
  - P4 facts builder 是否已经不再需要扫描 raw MIR 或 caller-candidate 旁路来补 callable identity。
- 必须检查的文件/位置：
  - `crates/scoopc/src/mir/materialize.rs`
  - `crates/scoopc/src/mir/callables.rs`
  - `crates/scoopc/src/mir/pass_view.rs`
  - `crates/scoopc/src/effect_refactor_pipeline/mir_stage.rs`
  - `crates/scoopc/src/effect_refactor_pipeline/effect_facts_stage.rs`

- 验证：
  - 重新运行 `P4-T02a` 的全部测试与命令；
  - 额外检查 ordinary non-generic 样本是否能让 `effect_facts().callable_facts()` / `effect_facts().bodies()` 都非空，且 key 来自 canonical `InstanceKey`。

- 完成条件：
  - review 能明确说明：ordinary callable identity 已在 canonical MIR handoff 上收口，`P4-T03` 不再受这个前置问题阻塞；
  - 可进入 `P4-T03`。
- 依赖：P4-T02a
- 完成记录：
  - 2026-05-02：完成 `P4-T02aR` review。最近一次提交 `[P4-T02a] Publish ordinary callables in canonical pass view` 与本次复核直接相关；review 过程中未发现还需要在 `P4-T03` 之前单独新增的前置任务，但确实暴露出一个与 canonical pass-view handoff 直接相关的回归：`effect_facts::builder::collect_callable_seeds(...)` 在 pass-view 保留 family 身份、但当前 canonical snapshot 已移除 root body 时仍报 `MissingCallableRoot`，没有完全按 canonical snapshot 过滤无 body family。
  - 该回归已在本次 review 内直接修复：`crates/scoopc/src/effect_facts/builder.rs` 现在会对“pass-view 中仍保留 instance identity、但当前 canonical snapshot 已无 root body”的 family 直接跳过，不再回 raw `MaterializedMir.file`、也不要求 caller-candidate / fallback 键空间补 owner；P4 callable/body facts 只对当前 canonical snapshot 中仍有 root body 的 family 发布。
  - canonical publication 复核结论：`crates/scoopc/src/mir/materialize.rs` 继续把 ordinary non-generic request-root / caller body 正式发布到 `pass_published_ordinary_callables`，并在 materialization 收口时同步生成稳定的 `pass_instance_keys`、pass-visible `MaterializedCallableFamilies`、`pass_file` 与 `pass_summaries`；`crates/scoopc/src/mir/pass_view.rs` 则以 `MaterializedMirPassArtifacts` 自持的 `instance_keys` / family side table 作为唯一 canonical 查询面，因此 `pass_view().instances()`、`owner_of_callable()`、`root_body()`、`callable_bodies()` 都能稳定命中这些 ordinary callable，且后续 family rehome/override 时仍维持单一 owner。
  - P4 handoff 复核结论：`crates/scoopc/src/effect_refactor_pipeline/effect_facts_stage.rs` 中 `RefactorEffectFactsStageOutput` 继续只暴露 `materialized_pass_view()` 作为 canonical MIR 查询面；`crates/scoopc/src/effect_facts/builder.rs::collect_callable_seeds(...)` 现已完全基于 `pass_view.instances()` 收集 callable seeds，不再扫描 raw MIR root 声明，也不会回退到 `caller_side_pass_candidate_bodies()` 或其它 fallback 键空间。ordinary non-generic sample 的 `effect_facts().callable_facts()` / `effect_facts().bodies()` 也继续通过 canonical `InstanceKey` 命中。
  - 复验通过：`cargo test -p scoopc --no-default-features materialized_effect_facts_builder_uses_canonical_pass_view_snapshot`、`cargo test -p scoopc --no-default-features materialized_pass_view_non_generic`、`cargo test -p scoopc --no-default-features refactor_effect_facts_stage_non_generic`、`cargo test -p scoopc --no-default-features refactor_effect_facts_stage`、`cargo test -p scoopc --no-default-features caller_side_inlining_keeps_non_generic_pass_roots_visible`、`cargo test -p scoopc production_codegen_observes_caller_side_mir_inlining_for_non_generic_body`、`cargo clippy -p scoopc --all-targets --no-default-features -- -D warnings`、`cargo clippy -p scoopc --all-targets -- -D warnings`。
  - 2026-05-02：本次 review 未改变阶段顺序或依赖，`PLAN.md` 无需改动；现已补齐本任务标题的 `[DONE]` 标记，并同步更新 `TODO.md` 索引。

## P4-T03：构建 `BodyEffectFacts` / `SiteEffectFacts` 与 local-case 结构化分析

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P4
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.12, §4.13.2-§4.13.3, §5.4.5-§5.4.7, §5.5.2-§5.5.6, §6
  - 当前实现参考：P3 direct-style MIR、`crates/scoopc/src/mir/mod.rs`、`crates/scoopc/src/mir/pass_view.rs`
- 目标：
  - 从 P3 的 direct-style MIR 中提取 body/block/site 级 facts；
  - 同时把 `local_cases(F)` 与 call-edge 输入显式化，为 T04 的 SCC/dataflow 求解准备完整结构化输入。

- 必须实现的内容：
  1. 定义并落地 `BodyEffectFacts`、`BlockEffectFacts`、`SiteEffectFacts` 及其子结构。
     - `BodyEffectFacts` 必须至少包含：
       - `blocks`
       - `sites`
     - `BlockEffectFacts` 的最终 public 形状必须至少包含：
       - `ambient_cases`
       - `outward_cases`
       - `has_suspend_boundary`
       - `has_handle_boundary`
     - `SiteEffectFacts` 必须至少包含变体：
       - `Call(CallSiteEffectFacts)`
       - `Perform(PerformSiteEffectFacts)`
       - `Resume(ResumeSiteEffectFacts)`
       - `Handle(HandleSiteEffectFacts)`
  2. 构建每个 materialized callable body 的 facts 容器。
     - 键必须是 facts 所采用的 callable 身份键；
     - `sites` 的键必须是 `SiteId`；
     - `blocks` 的键必须是 `BasicBlockId`；
     - 不允许用 `Span` 作为最终主键。
  3. 为 `CallSiteEffectFacts` 明确 target mode 与 target 集合来源。
     - 至少支持三类：
       - `KnownInstance`
       - `CandidateSet`
       - `DynamicFallback`
     - 当前 direct-style MIR 中各类调用的最低要求：
       - `CallKind::Direct`、唯一已知 `Closure` target -> `KnownInstance`
       - 有限候选集合且当前 pass-view/callable family 能枚举 -> `CandidateSet`
       - 无法可靠枚举的函数值 / interface / virtual / 其它动态边界 -> `DynamicFallback`
     - 若当前基础设施不能给出可信 candidate set，必须直接回退 `DynamicFallback`，禁止为追求精度而回 HIR/LLVM 现场重建。
  4. 为 `CallSiteEffectFacts` 明确必须暴露的 contract。
     - 至少要包含：
       - `target_mode`
       - `target`（已知实例或候选集合；若为 dynamic fallback，则显式标记而不是塞空值）
       - `invoke_args_tuple_ty`
       - `callee_schema`
       - `resolved_cases`
       - `precision`
     - 允许 `resolved_cases` 在本任务内先以保守值初始化；
     - 但字段必须已经存在，供 T04 回填。
  5. 为 `PerformSiteEffectFacts` 明确 emitted case 与 captured continuation contract。
     - 至少要包含：
       - `emitted_case`
       - `payload_tuple_ty`
       - `captured_cont_schema`
     - 这些信息必须从 P3 MIR + P2 typed contract 直接得到；
     - 禁止再从 HIR `HandleExpr` 或 parser 形状恢复 payload/resume 关系。
  6. 为 `ResumeSiteEffectFacts` 明确 continuation contract 与 outward contract。
     - 至少要包含：
       - `continuation_schema`
       - `resume_tuple_ty`
       - `answer_ty`
       - `out_step_schema`
       - `resolved_cases`
     - 若当前没有更强 continuation provenance 精度，允许先把 `resolved_cases` 保守初始化为 `cases(out_step_schema)` 或由 P3 metadata 指出的“无 outward”空集；
     - 但必须显式记录 runtime error ordinary effect 的 outward 影响，而不是靠隐藏通道。
  7. 为 `HandleSiteEffectFacts` 与 `HandleArmEffectFacts` 明确 handle 结构化 contract。
     - `HandleSiteEffectFacts` 必须至少包含：
       - `result_ty`
       - `handled_cases`
       - `body_outward_cases`
       - `arm_facts`
       - `finally_outward_cases`
     - `HandleArmEffectFacts` 必须至少包含：
       - `handled_case`
       - `payload_tuple_ty`
       - `continuation_schema`
       - `arm_outward_cases`
     - 对 nested `handle`，必须额外提供一个可直接查询的分类结果：
       - `SelfContained`
       - `MaySuspendOutward`
       - 可以实现为单独字段、枚举、或稳定 query API；
       - 但必须属于 facts 子系统的一部分，而不是交给 P5 重新推断。
  8. 显式提取 `local_cases(F)` 与 call-edge 输入。
     - `local_cases(F)` 至少要覆盖：
       - 本地 `perform` 发出的 cases
       - `handle` arm / `finally` / cleanup 向外再次发出的 cases
       - `resume` / ordinary runtime error 对 outward 的本地贡献
     - call-edge 输入至少要覆盖：
       - direct known callee
       - candidate-set union
       - dynamic fallback
  9. 为 block facts 准备最终回填所需的结构输入。
     - 若 `ambient_cases` / `outward_cases` 需要等 T04 结合全局求解结果后再最终写入，可以在 builder/solver 的**内部中间状态**中保留未决信息；
     - 但这些半结构化中间状态不得作为对外的 `MaterializedEffectFacts` 暴露给 P5 或 dump 命令；
     - 真正对外发布的 `BlockEffectFacts` 必须等 T04 finalization 完成后再写入最终字段。
  10. 若 refactor pipeline 已有 MIR-level escape facts 或 callable provenance facts 可作为精度输入，允许在本任务中接入；
      - 但它们只能提升精度，不能成为 correctness 依赖；
      - 缺失时必须保守 widen，而不是失败或回 HIR 补推断。

- 必须遵从的约束：
  - 禁止以 `Span` 作为 body/site facts 的最终主键。
  - 禁止把 `Resume` 混回普通 `Call` facts 而丢失独立 contract 变体。
  - 禁止让 `HandleSiteEffectFacts` 只留下“这是个 handle”这种布尔信息；必须能回答 P5 需要的 handled/outward/self-contained contract。
  - 禁止继续把 `target` / `resume` / `perform` 事实保留为裸 FQN 或裸 bool。

- 验证：
  1. 新增/更新单元测试，推荐命名：
     - `refactor_body_effect_facts_*`
     - `refactor_site_effect_facts_*`
     - `refactor_nested_handle_classification_*`
  2. 测试至少覆盖：
     - direct call / callable value / dispatch / resume 各类 site facts 结构；
     - `perform` 的 emitted case + continuation schema；
     - `handle` 的 handled cases、arm facts、finally outward；
     - nested handle 的 `SelfContained` vs `MaySuspendOutward` 分类；
     - site facts 全部通过 `SiteId` 查询。
  3. 运行：
      - `cargo test -p scoopc --no-default-features refactor_body_effect_facts`
      - `cargo test -p scoopc --no-default-features refactor_site_effect_facts`
      - `cargo test -p scoopc --no-default-features refactor_nested_handle_classification`

- 完成条件：
  - `BodyEffectFacts` / `SiteEffectFacts` / local-case 输入已经齐备；
  - T04 可以只消费这些结构化输入做全局求解，不再回 MIR/HIR 重新解释 site 语义；
  - nested handle 是否向外传播 suspension 已能通过 facts 直接回答。
- 依赖：P4-T02aR
- 完成记录：
  - 2026-05-02：执行本任务时发现一个会直接阻塞正确实现的新前置缺口：当前 canonical `MaterializedMir::pass_view()` 对 `dispatch_and_resume_call`、`handle_perform`、`handle_finally_boundary` 这类普通非泛型样本仍可能返回空 `instances()`，导致 `MaterializedEffectFactsBuilder` 无法在 authoritative `InstanceKey` / family 键空间上拿到普通 callable body，也就无法按任务要求对 `BodyEffectFacts` / `SiteEffectFacts` 使用稳定的 `(callable identity, BasicBlockId / SiteId)` 组织方式。
  - 按本文件“禁止 workaround / 禁止回 raw MIR 自造键空间”的约束，`P4-T03` 不能通过扫描 raw `MaterializedMir.file` 或 `caller_side_pass_candidate_bodies()` 临时补 owner identity 来继续推进；因此已在本任务前新增 `P4-T02a` / `P4-T02aR`，要求先修复 canonical pass-view 对 ordinary callable body 的发布，再继续 `P4-T03`。
  - 本次仅记录阻塞与新增前置；`P4-T03` 保持未完成状态，`PLAN.md` 暂无需改动。

## P4-T03R：Review body/site facts，确认 contract 已经闭包且不再依赖 HIR/span 推断

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.12, §4.13.2-§4.13.3, §5.4.5-§5.4.7
  - [`PLAN.md`](./PLAN.md) §2/P4
- 重点：
  - body/site facts 是否已通过 `InstanceKey + BasicBlockId/SiteId` 键空间组织；
  - `Call / Perform / Resume / Handle` 是否都拥有独立且可直接消费的 contract；
  - nested handle 分类是否已成为 facts 的显式结果；
  - 新实现是否仍在借助 HIR、span、名字、或 LLVM 现场信息补语义。
- 必须检查的文件/位置：
  - `crates/scoopc/src/effect_facts/builder.rs`
  - `crates/scoopc/src/effect_facts/facts.rs`
  - `crates/scoopc/src/effect_facts/schema.rs`
  - `crates/scoopc/src/mir/mod.rs`
  - `crates/scoopc/src/mir/pass_view.rs`

- 验证：
  - 重新运行 P4-T03 的全部测试与命令；
  - 额外搜索：
    - `rg "LoweredHir|hir::HandleExpr|continuation_resume_call_sites|effect_op_call_sites|Span" crates/scoopc/src/effect_facts crates/scoopc/src/effect_refactor_pipeline`
  - 要求：
    - 允许命中：注释、测试、debug/diagnostic-only 模块；
    - 不允许命中：新的 facts 主分析逻辑仍回 HIR side tables 取 effect contract。

- 完成条件：
  - review 能明确说明：body/site facts 已经形成对 P5 足够的结构化 contract；
  - 可进入 P4-T04。
- 依赖：P4-T03
- 完成记录：
  - （执行时填写）

## P4-T04：实现 `resolved_outward_cases` SCC/dataflow 求解，并完成 `needs_reentry` / `impl_plan` / final block facts 回填

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P4
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §3.3, §3.5, §4.4, §5.4.3-§5.4.7, §6, §7.3, §8
  - 当前实现参考：`crates/scoopc/src/mir/pass_view.rs`、`crates/scoopc/src/mir/summary.rs`（仅作 legacy 对照，不得再作为 authoritative 输入）
- 目标：
  - 用统一的 SCC/dataflow 管线求出 callable-level `resolved_outward_cases`；
  - 同时把 `needs_reentry`、`impl_plan`、site `resolved_cases` / `precision`、block `ambient/outward_cases` 全部回填为最终可消费结果。

- 必须实现的内容：
  1. 定义求解器输入与预算配置。
     - 求解器只能消费：
       - `StepSchema`
       - `ContinuationSchema`
       - `CallableEffectFacts` 壳层
       - `BodyEffectFacts` / `SiteEffectFacts`
       - 外部输入：opt level / feature flags / 预算
     - 预算项必须至少包含：
       - `max_scc_nodes`
       - `max_scc_edges`
       - `max_scc_iterations`
       - `max_candidate_union_size`
     - 这些参数应挂在 refactor stage config / session 派生配置中；禁止从环境变量或测试私货读取。
  2. 实现统一的 callable 图求解。
     - 图节点：当前 facts 所覆盖的 callable 实例；
     - 边来源：`CallSiteEffectFacts` 的 `KnownInstance` / `CandidateSet`；
     - `DynamicFallback` 不要求枚举边，而是直接按 schema 全集处理。
  3. 按设计文档实现 `resolved_outward_cases(F)` 的组合规则。
     - `resolved_outward_cases(F) = local_cases(F) ∪ call_edge_cases(F)`；
     - `local_cases(F)` 至少来自：
       - 本地 `perform`
       - local runtime error ordinary effect
       - handle arm / finally / cleanup outward
       - 本地 handle 吸收结果
     - `call_edge_cases(F)` 至少来自：
       - direct known callee -> 并入 callee `resolved_outward_cases`
       - candidate set -> 并入候选并集
       - dynamic fallback -> 直接取对应 `StepSchema` 全集
  4. 明确 site-level `resolved_cases` 与 `precision` 回填规则。
     - `KnownInstance`：
       - `resolved_cases = callee.resolved_outward_cases`
       - 若未 widen，则 `precision = Precise`
     - `CandidateSet`：
       - `resolved_cases = union(callee.resolved_outward_cases)`
       - 若候选并集或预算导致 widen，则 `precision = Widened`
     - `DynamicFallback`：
       - `resolved_cases = cases(callee_schema)`
       - `precision = SignatureFallback`
     - `Resume`：
       - 若 `out_step_schema` 已知为空集，则保留空集；
       - 否则按 continuation/outward schema 与可用精度回填，最低要求是保守不小于真实 outward；
       - 明确禁止：对 `Resume` 没有最终 `resolved_cases`。
  5. 实现预算耗尽时的 widening 规则。
     - 一旦超过任一预算：
       - 对受影响实例或整个 SCC 直接 widen 到各自 `cases(StepSchema(F))`；
       - 不再继续深挖；
       - 结果仍必须满足 `actual_outward_cases ⊆ resolved_outward_cases ⊆ cases(StepSchema(F))`。
  6. 派生 `needs_reentry`。
     - 规则必须固定为：
       - `needs_reentry(F) = !resolved_outward_cases(F).is_empty()`
     - 不允许在本阶段额外引入第二套“更精确 reentry 条件”的分析。
  7. 派生 `impl_plan`。
     - 只允许三档：
       - `NoOutward`
       - `SingleCase(case_tag)`
       - `CanonicalFull`
     - 规则必须与 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §7.3 一致：
       - 空集 -> `NoOutward`
       - 恰好 1 个 case -> `SingleCase(case_tag)`
       - 其余 -> `CanonicalFull`
     - 优化级别约束：
       - `O0` / debug build：除 `NoOutward` 外，其余非空情况一律使用 `CanonicalFull`
       - 更高优化级别：允许 `SingleCase(case_tag)`
     - 明确禁止：任意子集特化、启发式“小于 N 个 case 就特化”、profile-guided widening。
  8. 回填 final block facts。
     - `BlockEffectFacts.ambient_cases` 必须表示该 block 入口所处的 effect 上下文上界；
     - `BlockEffectFacts.outward_cases` 必须表示该 block 自身可能向外推出的 cases；
     - 若一个 block 有多个前驱，入口 `ambient_cases` 采用保守 union；
     - 对位于 `handle` body/arm/finally 内部的 block，必须显式体现 handled-case 吸收后的上下文；
     - `has_suspend_boundary` / `has_handle_boundary` 必须在 final facts 中可直接查询。
  9. 明确 structural rewrite 后的重算策略。
     - 若 facts 求解前后有新的 MIR rewrite 触发，必须重新构建受影响 body 的 `BodyEffectFacts` 并重新求解；
     - 不允许用 patch-up 方式局部改几处 case 后继续复用旧结果。
  10. 若当前 refactor stage 已有 MIR-level continuation escape/provenance 精度输入，允许在本任务中纳入 solver；
      - 但不能改变“缺失这些精度时仍保守正确”的前提；
      - 更高精度只能缩小 `resolved_outward_cases`，不能扩大到超出 schema 全集。

- 必须遵从的约束：
  - 禁止把 `mir::summary::InstanceSummary.may_outward_effect` 当作 refactor solver 输入。
  - 禁止新增与 opt level 平行的第二套求解管线。
  - 禁止把 budget exhaustion 实现成“保持旧值不变”；必须显式 widen 到 schema 全集。
  - 禁止在本阶段引入超过 `NoOutward | SingleCase | CanonicalFull` 的版本选择。

- 验证：
  1. 新增/更新单元测试，推荐命名：
     - `refactor_effect_solver_*`
     - `refactor_impl_plan_*`
     - `refactor_block_effect_facts_*`
  2. 测试至少覆盖：
     - direct known callee 传播；
     - candidate-set union；
     - dynamic fallback -> schema 全集；
     - budget exhaustion -> widen；
     - `needs_reentry = !resolved_outward_cases.is_empty()`；
     - `O0` 与较高优化级别在 `SingleCase` 选择上的差异；
     - nested handle `SelfContained` vs `MaySuspendOutward` 在 final resolved cases 下的分类结果；
     - block `ambient_cases` / `outward_cases` 的最终回填。
  3. 运行：
      - `cargo test -p scoopc --no-default-features refactor_effect_solver`
      - `cargo test -p scoopc --no-default-features refactor_impl_plan`
      - `cargo test -p scoopc --no-default-features refactor_block_effect_facts`

- 完成条件：
  - `resolved_outward_cases`、`needs_reentry`、`impl_plan`、final block/site facts 已全部可用；
  - P5 可以只消费 P4 facts，而不必再做新的高层 effect 语义求解；
  - 新路径不再依赖 `may_outward_effect` 或 ad-hoc shape 规则做 lowering 决策。
- 依赖：P4-T03R
- 完成记录：
  - （执行时填写）

## P4-T04R：Review solver / widening / `impl_plan`，确认求解结果完全由 facts 驱动

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.4, §5.4.3-§5.4.7, §6, §7.3
  - [`PLAN.md`](./PLAN.md) §2/P4
- 重点：
  - `resolved_outward_cases` 是否只由 facts + 外部输入求出；
  - widening / dynamic fallback / `SingleCase` 规则是否与设计文档一致；
  - `needs_reentry` 是否已退化成对 `resolved_outward_cases` 的非空判定；
  - block/site facts 是否已被 final 结果回填，而不是仍停留在半成品状态。
- 必须检查的文件/位置：
  - `crates/scoopc/src/effect_facts/solver.rs`
  - `crates/scoopc/src/effect_facts/builder.rs`
  - `crates/scoopc/src/effect_facts/facts.rs`
  - refactor effect-facts stage 模块
  - `crates/scoopc/src/mir/summary.rs`（仅对照，不得作为 authoritative 输入）

- 验证：
  - 重新运行 P4-T04 的全部测试与命令；
  - 额外搜索：
    - `rg "may_outward_effect|SingleCase|CanonicalFull|resolved_outward_cases|needs_reentry" crates/scoopc/src`
  - 要求：
    - 必须能明确指出 refactor solver 的 authoritative 结果位于新的 facts 子系统；
    - 若 `may_outward_effect` 仍被 refactor 主线直接消费，必须修复。

- 完成条件：
  - review 能明确说明：P4 求解结果已完全收口到 facts 子系统，不再靠 legacy 摘要或 HIR 旁路；
  - 可进入 P4-T05。
- 依赖：P4-T04
- 完成记录：
  - （执行时填写）

## P4-T05：新增 `dump-effect-facts` / snapshot 基线，并冻结 P4 -> P5 handoff contract

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P4，§2/P5
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.13.1a, §4.15, §5.4.8, §5.5.1, §8
  - 当前 CLI/fixture 入口参考：`crates/scoop/src/cli.rs`、`crates/scoop/src/commands/mod.rs`、`crates/scoop/src/fixtures/mod.rs`
- 目标：
  - 为 refactor effect facts 提供稳定的用户可见 dump 入口；
  - 建立自动化 snapshot/golden 基线；
  - 把 P5 只能消费“P3 MIR snapshot + P4 MaterializedEffectFacts”的合同固化到代码与测试中。

- 必须实现的内容：
  1. 在 `scoop` CLI 上新增 `dump-effect-facts` 子命令。
     - 必须修改：
       - `crates/scoop/src/cli.rs`
       - `crates/scoop/src/commands/mod.rs`
       - 新增 `crates/scoop/src/commands/dump_effect_facts.rs`
     - `refactor` 路径必须显式进入新的 effect-facts stage，并输出稳定 formatter；
     - `legacy` 路径若当前没有等价实现，必须返回明确、稳定、可测试的“legacy pipeline 暂不支持该命令”的诊断，而不是静默回退或打印半成品。
  2. 为 `MaterializedEffectFacts` 提供稳定 formatter。
     - 输出必须至少能稳定展示：
       - `StepSchema` / `ContinuationSchema` pools
       - callable-level facts
       - body/block/site facts
       - `resolved_outward_cases`
       - `needs_reentry`
       - `impl_plan`
       - nested handle `SelfContained` / `MaySuspendOutward` 分类
     - 若 raw `Debug` 输出中 `TypeId(n)`、绝对路径、内部不稳定 id 太多，不足以作为 golden，则必须实现自定义 formatter，优先展示语义可读且稳定的类型/identity 文本。
  3. 为 effect facts 新增 dedicated fixture phase。
     - 推荐目录：`tests/fixtures/effect_facts/**`
     - 推荐 golden 扩展：`.effectfacts`
     - 需要修改 `crates/scoop/src/fixtures/mod.rs`，增加对应 phase 与 golden 比对逻辑；
     - 该 phase 必须通过与 CLI 相同的 stage helper / formatter 获取输出，禁止测试和 CLI 各自拼接不同文本。
  4. 新增 refactor effect-facts fixtures，至少覆盖：
     - `tests/fixtures/effect_facts/direct_and_fun_value_call.scoop`
     - `tests/fixtures/effect_facts/dispatch_and_resume_call.scoop`
     - `tests/fixtures/effect_facts/handle_perform.scoop`
     - `tests/fixtures/effect_facts/handle_finally_boundary.scoop`
     - `tests/fixtures/effect_facts/nested_handle_self_contained_vs_outward.scoop`
     - `tests/fixtures/effect_facts/single_case_impl_plan.scoop`
     - `tests/fixtures/effect_facts/dynamic_fallback_widening.scoop`
     - 若这些样本已在 P3 的 `mir_refactor` 目录中存在同名 `.scoop`，允许直接复制并复用源码；
     - 但 golden 必须是 effect-facts 专属输出，禁止复用 `.mir` golden。
  5. 明确 P4 -> P5 handoff contract。
     - 必须在代码注释或等价文档实体中明确写出：
       - P5 的 canonical 输入是“当前 canonical MIR snapshot + `MaterializedEffectFacts`”；
       - P5 不得再回 P2/P3/HIR/typecheck 补 effect 语义；
       - P4 不提供 late-lowered `Step` IR，仅提供 schema/facts。
  6. 若 facts stage 对 opt level / budgets 的行为有用户可见差异，formatter 或测试必须显式锁定至少一种 `O0` 与一种较高优化级别下的代表性输出。

- 必须遵从的约束：
  - 禁止让 `dump-effect-facts` 继续读取 HIR 或 typecheck 结果作为主输出来源。
  - 禁止把 effect-facts golden 与 legacy MIR/HIR golden 混在同一目录或同一扩展名下。
  - 禁止只做 Rust unit tests 而没有任何用户可见 dump/snapshot 入口。

- 验证：
  1. 运行新增的 effect-facts snapshot / fixture 测试入口；
  2. 运行：
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-effect-facts tests/fixtures/effect_facts/dispatch_and_resume_call.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-effect-facts tests/fixtures/effect_facts/handle_perform.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/effect_facts/dispatch_and_resume_call.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/effect_facts/handle_perform.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/effect_facts/nested_handle_self_contained_vs_outward.scoop`
   3. 额外验证 legacy unsupported 诊断（若按本任务要求实现为拒绝）：
      - `cargo run -p scoop --no-default-features -- --effect-pipeline legacy dump-effect-facts tests/fixtures/effect_facts/dispatch_and_resume_call.scoop`

- 完成条件：
  - `dump-effect-facts` 已存在并稳定输出；
  - 仓库中已有 dedicated effect-facts snapshot/golden 基线；
  - P4 -> P5 的 handoff contract 已通过代码与测试锁定。
- 依赖：P4-T04R
- 完成记录：
  - （执行时填写）

## P4-T05R：Review P4 阶段退出条件，确认 P5 可以只消费 MIR + facts 完成 lowering 决策

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P4，§2/P5，§3
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.13, §5.4, §5.5.1-§5.5.6, §6, §7.3, §8
- 重点：
  - `MaterializedEffectFacts` 是否已经完整存在并成为独立子系统；
  - `StepSchema` / `ContinuationSchema` / callable-block-site facts 是否都已显式化；
  - `resolved_outward_cases` / `needs_reentry` / `impl_plan` 是否已全部收口到 facts；
  - `dump-effect-facts` 与 snapshot/golden 是否已建立；
  - P5 是否已经可以在不回 HIR 的前提下，只消费 P3 MIR snapshot + P4 facts 做全部 lowering 决策。

- 验证：
  - 重新运行 P4-T01 ~ P4-T05 的全部定向测试与命令；
  - 不再额外执行 `cargo test -p scoop` / `cargo test -p scoopc` 全 crate 测试；保持本阶段只做定向验证。

- 完成条件：
  - review 能明确说明：P4 已经完成“effect facts 与 `resolved_outward_cases` 分析落地”的阶段目标；
  - P5 可以在不重新讨论 effect schema、site contract、nested handle 分类、或 outward case 求解策略的前提下直接进入 late-lowering 实现。
- 依赖：P4-T05
- 完成记录：
  - （执行时填写）
