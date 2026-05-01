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

## P4-T01：建立 refactor effect-facts stage 与独立 side-table 子系统边界

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
  - （执行时填写）

## P4-T01R：Review facts stage 边界，确认没有把新 facts 混进 legacy `effect` / `summary` / `ProgramFacts`

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
  - （执行时填写）

## P4-T02：落地 schema identity、canonical schema pool 与 callable-level facts 壳层

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
  - （执行时填写）

## P4-T02R：Review schema pool 与 callable facts，确认 identity 和 case contract 已经固定

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
  - （执行时填写）

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
- 依赖：P4-T02R
- 完成记录：
  - （执行时填写）

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
