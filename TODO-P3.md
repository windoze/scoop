# TODO（P3：direct-style MIR 新路径落地）

> 生成时间：2026-05-02  
> 设计基线：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 前置条件：`TODO-P2.md` 已完整完成，refactor typed HIR stage 已存在，且 P2 产出的 typed HIR effect/continuation side tables 已成为新路径上的显式 handoff contract。  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 本阶段目标：在 refactor 新路径上建立 production-grade 的 direct-style MIR，使 `Call / Perform / Resume / Handle` 保持一等语义节点，`SiteId` 与 CFG/control-flow 显式到足以支撑 P4 effect facts 构建；同时彻底停止“缺 MIR 语义就退回 HIR / span / 名字推断”的旧做法。

## 全局约束

- [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 是本阶段唯一设计基线；若实现过程中要改变主张，必须先回写该文档，再继续实现。
- [`PLAN.md`](./PLAN.md) 与 [`TODO-P0.md`](./TODO-P0.md)、[`TODO-P1.md`](./TODO-P1.md)、[`TODO-P2.md`](./TODO-P2.md) 是本阶段执行前提；P3 不得重新开启 P0-P2 已经收敛的 CLI / dispatcher / AST / typed HIR contract 讨论。
- 本阶段只处理 refactor 新路径上的 direct-style MIR。
  - 明确禁止：在 P3 中实现 `StepSchema`、`ContinuationSchema`、`MaterializedEffectFacts`、`resolved_outward_cases`、late-lowered `Step_F`、state-machine object、LLVM lowering；这些属于 P4-P6。
  - 明确禁止：让 refactor MIR 提前落成 `Step` IR、`Step_F` enum、resume interface object，或调用当前 legacy `crates/scoopc/src/effect/state_machine/**`、`crates/scoopc/src/llvm/codegen/effect/**` 作为“提前完成”的替代路线。
- 本阶段必须继续遵守 P0 的“共享模块 vs 复制实现”原则。
  - 若 `crates/scoopc/src/mir/mod.rs`、`mir/materialize.rs`、`mir/pass_view.rs`、`mir/inline.rs` 中的某部分可以通过**完全中立的单一 API**共享，则允许复用；
  - 若旧 MIR lowering / materialization / inline 逻辑仍混有 legacy HIR fallback、旧 effect 假设、或 dump-only contract，则必须复制到 refactor 路线上，不允许在原业务函数里加 pipeline 分支。
- refactor 新路径在 late lowering 之前必须保持 direct-style。
  - 允许显式 CFG、locals、temporaries、`Call / Perform / Resume / Handle` 节点、`SiteId`、cleanup edge、以及 MIR 附带的 typed contract side table；
  - 不允许提前把这些节点重写成 code-shape-specific 的局部状态机或 `Step` carrier。
- P3 结束后，effect/continuation 相关结构必须在 MIR 层成为 source of truth。
  - P4 不允许再回 AST / HIR / typecheck 内部缓存来判断 `resume` 的输入输出 contract、`perform` 的 payload tuple、`handle` 的 binder/result/finally 关系；
  - 因此这些信息必须在 P3 被下沉到 MIR 节点 metadata 或 MIR-attached side table 中，并作为 P3 stage 输出的一部分显式暴露。
- refactor 路径不得继续依赖当前 `crates/scoopc/src/mir/lower.rs` 中那种“基于 `Span` / 名字 / 稀疏 side table 的推断式 MIR lowering contract”。
  - 明确禁止：用 `Continuation.resume` 的名字、`Span` 集合、或 HIR 原始源码形状在 MIR 阶段再次猜测 site 语义；
  - 必须直接消费 P2 产出的 typed HIR contract side tables。
- 所有 effect-sensitive site 在 refactor MIR 中都必须具备稳定身份。
  - `Call` / `Resume` 继续通过 `Rvalue::Call.site_id` 建模；
  - `Perform` / `Handle` 继续通过 terminator 上的 `site_id` 建模；
  - 若任何 MIR rewrite / clone / inlining 复制出新的 site，必须为复制体分配新的 `SiteId`，不能复用旧 id。
- 本阶段必须把 `return` / `break` / `continue` / `finally` / cleanup / boundary 后续点显式化到 MIR CFG 中。
  - 明确禁止：把这些控制流留成 `Todo(...)` 占位，或留到 P4/P5 再凭源码结构补建。
- 自 P3 起，refactor `dump-mir` 输出不要求继续与 legacy 完全 parity。
  - legacy `tests/fixtures/mir/**` 仍作为旧主线基线保留；
  - refactor MIR 若因为 metadata 更完整、CFG 更显式而产生新输出，必须通过**独立的 refactor snapshot/golden** 锁定，而不是覆盖 legacy baseline。
- 本阶段不做 full regression。
  - 只做 refactor `dump-mir`、MIR verifier、MIR lowering 单元测试、以及专门的 refactor snapshot/golden；
  - 不执行 `cargo test --all`；
  - 不执行 `cargo run -p scoop -- test` 的全量 fixture 扫描。
- 所有新路径验证都必须通过 `--effect-pipeline refactor` 进入，或通过与该 CLI 路径共用同一 stage helper 的 Rust 测试入口进入；禁止新增只在测试中存在的旁路实现。

## P3-T01：建立 refactor direct-style MIR stage 入口与显式 stage 输出，切断 `dump-mir` 对 legacy `mir::lower_for_dump` 的依赖

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P3
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.10, §4.11, §4.12, §8
- 目标：
  - 在 refactor 新路径上建立一个明确的 MIR stage，而不是继续让 `dump-mir` / 调试入口直接调用当前 legacy `crates/scoopc/src/mir/lower.rs::lower_for_dump(...)`；
  - 让 P3 的 stage 输出成为后续 P4 直接消费的 canonical handoff，而不是“打印一下 MIR Debug 然后各阶段各自再找入口”。

- 必须实现的内容：
  1. 在 refactor pipeline 下新增 MIR stage 模块。
     - 推荐位置：`crates/scoopc/src/effect_refactor_pipeline/mir_stage.rs`；
     - 若 P0/P2 最终把 refactor stage 模块放在别处，允许使用等价位置；
     - 但它必须是 refactor pipeline 的显式阶段入口，而不是 `crates/scoopc/src/mir/lower.rs` 里再加一层 pipeline 分支。
  2. 定义一个 refactor MIR stage 输出类型。
     - 名称可自定，例如 `RefactorMirStageOutput`；
     - 该输出必须至少承载：
       - 当前阶段 canonical 的 direct-style MIR 结果；
       - 与之配套的 `TypeStore`；
       - 供 P4 使用的 canonical `MaterializedMir` 快照或等价查询面（至少能稳定按 `InstanceKey` / callable body 身份访问 materialized bodies）；
       - 与 MIR 绑定的 effect/continuation contract handoff 容器（若信息直接沉到 MIR metadata，则至少要有稳定查询 API / formatter 暴露这些 metadata）。
     - 该输出类型的注释中必须明确写出本阶段 invariants：
       - 当前产物仍是 direct-style MIR，不是 late-lowered `Step` IR；
       - 所有效果敏感 site 都以 `SiteId` 锚定；
       - P4 必须以这份 stage 输出为输入，而不是回看 P2 原始 HIR 内部缓存。
  3. 为 refactor MIR stage 提供一个明确入口函数。
     - 输入必须是 P2 的 refactor typed HIR stage 输出，而不是重新从 AST / HIR dump 路径现拼；
     - 如果 P2 最终输出类型名称不是 `TypedHirEffectContracts` / `RefactorTypedHirStageOutput`，则使用该阶段的最终等价输出；
     - 要求：该入口函数必须成为后续 `dump-mir`、refactor snapshot tests、以及 P4 的共同入口。
  4. 修改 `crates/scoop/src/commands/dump_mir.rs` 与对应 dispatcher。
     - `legacy` 路径继续维持当前行为；
     - `refactor` 路径必须显式进入新的 MIR stage，再打印 stage 输出的稳定 Debug / formatter；
     - 禁止 `refactor dump-mir` 继续直接调用 legacy `scoopc::mir::lower_for_dump(...)`。
  5. 若当前 `crates/scoop/src/fixtures/mod.rs::mir_fixture(...)`、或 `scoopc` 内部 MIR 测试 helper，会直接调用 legacy `mir::lower_for_dump(...)`，且它们需要支持 refactor 路径，则必须为其增加“经 refactor MIR stage 获取输出”的新辅助层。
     - 允许保留 legacy helper；
     - 但 refactor 路径必须有独立入口，不能继续复用 legacy lowerer 的换壳调用。
  6. 若当前代码中通过 `hir::LoweredHir.materialized_mir` 隐式承载 production MIR 快照，则 refactor MIR stage 必须把这层 handoff 显式收口到自己的 stage 输出上。
     - 允许内部复用现有 `materialized_mir` 构造结果；
     - 但不允许让 P4 只能通过回到 `LoweredHir` 私有字段或旧 helper 才拿到 canonical MIR 快照。

- 必须遵从的约束：
  - 禁止在 `crates/scoopc/src/mir/lower.rs`、`mir/materialize.rs`、`mir/pass_view.rs` 这类旧业务模块里直接加入 `if pipeline == Refactor` 分支。
  - 禁止把 stage 语义藏在 `dump_mir.rs` 或 fixture runner 里；stage 构造必须属于 compiler crate。
  - 禁止把“目前先输出一个 `mir::File` Debug，以后再决定 production handoff”作为完成标准；P3-T01 必须把 P4 的 canonical MIR 输入出口明确下来。

- 验证：
  1. 新增/更新单元测试，推荐命名：`refactor_direct_mir_stage_*`，至少覆盖：
     - refactor MIR stage 输出类型可构造；
     - `dump-mir --effect-pipeline refactor` 确实进入新 stage；
     - legacy `dump-mir` 路径仍沿用原有实现。
  2. 运行：
      - `cargo test -p scoopc --no-default-features refactor_direct_mir_stage`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline legacy dump-mir tests/fixtures/mir/direct_zero_arg_call.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir/direct_zero_arg_call.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir/direct_and_fun_value_call.scoop`
  3. 要求：
     - legacy 命令输出继续成功；
     - refactor 命令能通过新 stage 产出稳定输出；
     - 相关测试能证明 refactor 没有回落到 legacy `mir::lower_for_dump`。

- 完成条件：
  - refactor 新路径已拥有独立的 MIR stage 入口与 stage 输出类型；
  - `dump-mir --effect-pipeline refactor` 已不再依赖 legacy `mir::lower_for_dump`；
  - P4 之后的任务已经有明确的 canonical MIR handoff 可接。
- 依赖：`TODO-P2.md` 最后一项 review 完成
- 完成记录：
  - 2026-05-02：完成 `P3-T01`，新增 `crates/scoopc/src/effect_refactor_pipeline/mir_stage.rs`，为 refactor 新路径建立独立的 direct-style MIR stage，并定义 `RefactorMirStageOutput` 作为 P3 -> P4 的显式 handoff 结构。
  - `RefactorMirStageOutput` 现在显式承载 canonical direct-style `LoweredMir`、配套 `TypeStore`、来自 P2 的 `TypedHirEffectContracts` handoff、按 callable FQN 稳定查询 body 的 `callable_body_indices`，以及从 `LoweredHir` 显式取出的可选 `materialized_mir` 快照；其注释明确固定了“当前仍是 direct-style MIR、effect-sensitive site 继续由 `SiteId` 锚定、P4 必须消费该 stage 输出而不是回看 P2 内部缓存”的 invariants。
  - `crates/scoopc/src/effect_refactor_pipeline/mod.rs` 新增 `load_direct_style_mir_stage_output_for_dump(...)`；refactor 模式下 `lower_direct_style_mir_for_dump(...)` 现已改为显式走 `TypedHirStageOutput -> mir_stage::run(...) -> RefactorMirStageOutput`，不再直接调用 legacy `crates/scoopc/src/mir/lower.rs::lower_for_dump(...)`；legacy 路径继续保持原有 `mir::lower_for_dump(...)` 行为不变。
  - `crates/scoop/src/commands/dump_mir.rs` 已改为在命令边界显式分流：`legacy` 继续打印 legacy `LoweredMir.file` Debug；`refactor` 通过 `load_direct_style_mir_stage_output_for_dump(...)` 获取新的 stage 输出，再用 `RefactorMirStageOutput::stable_dump()` 渲染，因此 `dump-mir --effect-pipeline refactor` 已不再是 legacy lowerer 的换壳调用。`scoop test` 的 `mir_fixture(...)` 继续通过 `effect_refactor_pipeline::lower_direct_style_mir_for_dump(...)` 进入，因此 refactor fixtures 也继承同一 stage 入口。
  - `crates/scoopc/src/hir/lower/types.rs` 新增 `LoweredHir::into_materialized_mir()`，把原先只藏在 `LoweredHir` 私有字段中的 canonical materialized MIR handoff 显式暴露给 refactor MIR stage，避免后续阶段只能回到旧 helper/私有字段取产物。
  - 本任务未改动 `TODO.md` 或 `PLAN.md`：任务索引、顺序与阶段计划保持不变。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_direct_mir_stage`、`cargo test -p scoopc --no-default-features effect_refactor_pipeline`、`cargo test -p scoop --no-default-features dump_mir`、`cargo test -p scoop --no-default-features parity`、`cargo run -p scoop --no-default-features -- --effect-pipeline legacy dump-mir tests/fixtures/mir/direct_zero_arg_call.scoop`、`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir/direct_zero_arg_call.scoop`、`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir/direct_and_fun_value_call.scoop`、`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`。

## P3-T01R：Review refactor MIR stage 入口，确认新路径已与 legacy `mir::lower_for_dump` 分离

- 参考：
  - [`PLAN.md`](./PLAN.md) §0，§2/P3
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.10, §4.11, §8
- 重点：
  - refactor `dump-mir` 是否真的通过新的 MIR stage 入口进入，而不是换壳调用 legacy lowerer；
  - stage 输出是否已经把 P4 会消费的 canonical MIR handoff 显式化；
  - 是否仍然避免在旧 `mir::*` 业务实现里写 pipeline 分支。
- 必须检查的文件/位置：
  - 新增的 `mir_stage` 模块
  - `crates/scoop/src/commands/dump_mir.rs`
  - `crates/scoop/src/fixtures/mod.rs` 中与 MIR 相关的 helper（若已调整）
  - `crates/scoopc/src/mir/lower.rs`
  - `crates/scoopc/src/mir/materialize.rs`
  - `crates/scoopc/src/hir/lower/types.rs` 或 P2 最终 MIR handoff 相关位置

- 验证：
  - 重新运行 P3-T01 的全部测试与命令；
  - 额外搜索：
    - `rg "EffectPipelineMode|effect_pipeline|refactor|legacy" crates/scoopc/src/mir crates/scoopc/src/effect_refactor_pipeline`
  - 允许命中：新 stage/dispatcher、测试、注释；
  - 不允许命中：旧 `mir/lower.rs` / `mir/materialize.rs` / `mir/pass_view.rs` 的业务函数里新增线路分支。

- 完成条件：
  - review 能明确证明：refactor MIR stage 已经是独立阶段，而不是 legacy lowerer 的换壳；
  - 可进入 P3-T02。
- 依赖：P3-T01
- 完成记录：
  - （执行时填写）

## P3-T02：把 P2 typed contract 下沉到 direct-style MIR，停止基于 span / 名字 / HIR fallback 猜测 `Call / Perform / Resume / Handle` 语义

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P3
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.11, §4.12, §4.13, §5.3.1, §5.3.9, §5.4.5-§5.4.7
  - 当前实现参考：`crates/scoopc/src/mir/lower.rs` 中 `MirLoweringFacts`、`PerformCallSiteInfo`、`ResumeMetadata`、`PerformMetadata` 一带
- 目标：
  - 让 refactor direct-style MIR 直接消费 P2 typed HIR side tables，并把 effect/continuation 相关 contract 下沉到 MIR；
  - 使 `Call / Perform / Resume / Handle` 真正成为 MIR 层的 source of truth，而不是“看 span / 看成员名 / 看 HIR side table 是否碰巧有记录”的半桥接状态。

- 必须实现的内容：
  1. 为 refactor MIR lowering 定义一份结构化输入 contract。
     - 当前 `crates/scoopc/src/mir/lower.rs::MirLoweringFacts` 里基于：
       - `continuation_resume_call_spans`
       - `non_pure_continuation_resume_call_spans`
       - `effect_op_call_sites`（按 `Span` 键）
       - 零散 `dispatch_call_sites`
       的模式，不能继续作为 refactor 主路径；
     - refactor 路径必须改为直接消费 P2 产出的 typed HIR effect/continuation side-table 容器；
     - 若当前 legacy `MirLoweringFacts` 仍要保留给旧路径，可保留，但 refactor stage 不得依赖它作为 authoritative 输入。
  2. 扩展 direct-style MIR 中 effect/continuation 相关 metadata 或 MIR-attached side table。
     - 至少要让 refactor MIR 对以下信息具备显式表达：
       - call site 的调用分类：direct / closure / fun-value / virtual / interface / continuation-resume；
       - `perform` site 的 concrete op 身份、payload tuple 形状、参数 canonicalization 结果；
       - `resume` site 的 `ResumeTuple`、`Answer`、`Out`、以及 runtime error ordinary effect 语义；
       - `handle` site 的 result type、每个 arm 的 handled op 身份、binder tuple / continuation binder 关系、`finally` 存在性；
       - 若某信息最终通过 MIR-attached side table 表达，则该 side table 必须以 `SiteId` 或 body-local 稳定键组织，并成为 stage 输出的一部分。
     - 明确禁止：在 P3 就引入 `StepSchema` / `ContinuationSchema` 完整求解；
     - 但也明确禁止：让 P4 还需要回 HIR/typecheck 恢复这些 typed contract。
  3. 在 refactor MIR lowering 中，`Call / Perform / Resume / Handle` 必须按 typed contract lower 成一等 MIR 节点。
     - `Resume` 继续使用 `Rvalue::Call { kind: CallKind::Resume, site_id, ... }`；
     - `Perform` 继续使用 `TerminatorKind::Perform { site_id, ... }`；
     - `Handle` 继续使用 `TerminatorKind::Handle { site_id, ... }`；
     - 但它们的 metadata 必须升级为足以支撑 P4 的形状，而不是当前“只够 dump 可看”的最小字段。
  4. 停止在 refactor 路径上使用名字/源码特判识别 continuation resume。
     - 明确禁止：通过成员名恰好叫 `resume`、或 `Span` 位于某个集合中，来判定这是 continuation resume；
     - 必须由 P2 typed contract 显式告知这是哪个 `resume` site，以及它的 typed contract 是什么。
  5. 把 P2 中已经固定的 runtime error ordinary effect 语义延续到 MIR contract。
     - `ContinuationAlreadyResumed` 一类 runtime error 在 MIR hook point 中仍必须属于 ordinary effect 传播合同的一部分；
     - 不能在 MIR 阶段悄悄退回到第二条隐藏错误通道。
  6. 若当前 MIR metadata 或 dump 仍保留 `resume callee lowering pending`、`call callee lowering pending`、或等价 effect/continuation 占位逻辑，则 refactor 路径必须在本任务中移除对这些占位的依赖。

- 必须遵从的约束：
  - 禁止在 refactor 路径上继续使用 `Span` / 名字推断 `resume` 或 effect-op 语义。
  - 禁止恢复 tuple payload 的多参数 `resume` 特例；`resume` 在 MIR 语义层必须继续表现为“恰好一个 tuple 参数”的普通调用。
  - 禁止为了“先跑通”而让 `Perform` / `Resume` / `Handle` 回退成普通 `Call`、后端临时特殊分支、或 `Todo(...)` rvalue。

- 验证：
  1. 新增/更新 unit tests，推荐命名：`refactor_mir_lowering_contract_*`，至少覆盖：
     - refactor MIR lowering 直接消费 P2 typed contract，而不是 legacy `MirLoweringFacts` 的 `Span` 集合；
     - `dispatch_and_resume_call` 中 direct / virtual / interface / resume 各类 site 都能产出明确 MIR metadata；
     - `perform` site 的 payload tuple / 参数顺序在 MIR 中显式可见；
     - runtime error ordinary effect contract 在 `resume` MIR contract 中仍可见。
  2. 新增/更新 refactor MIR 样本，推荐至少包括：
     - 复用 `tests/fixtures/mir/direct_and_fun_value_call.scoop`
     - 复用 `tests/fixtures/mir/dispatch_and_resume_call.scoop`
     - 复用 `tests/fixtures/mir/handle_perform.scoop`
     - 新增 `tests/fixtures/mir_refactor/continuation_resume_unit_sugar.scoop`
       - 场景：同时包含 `k.resume()` 与 `k.resume(())`
       - 目标：锁定二者在 refactor MIR 中使用同一 canonical 单参数调用合同
  3. 运行：
      - `cargo test -p scoopc --no-default-features refactor_mir_lowering_contract`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir/direct_and_fun_value_call.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir/dispatch_and_resume_call.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir/handle_perform.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/continuation_resume_unit_sugar.scoop`
   4. 额外抽样验证 legacy 不受影响：
      - `cargo run -p scoop --no-default-features -- --effect-pipeline legacy dump-mir tests/fixtures/mir/dispatch_and_resume_call.scoop`

- 完成条件：
  - refactor MIR 已直接承接 P2 typed contract；
  - `Call / Perform / Resume / Handle` 在 refactor MIR 中拥有足以支撑 P4 的显式 contract；
  - 新路径不再依赖 span/name/HIR fallback 猜语义。
- 依赖：P3-T01R
- 完成记录：
  - （执行时填写）

## P3-T02R：Review direct-style MIR contract，下沉信息是否已足够并且不再依赖 span / 名字猜测

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.12, §4.13, §5.3.1, §5.3.9
  - [`PLAN.md`](./PLAN.md) §2/P3
- 重点：
  - refactor MIR lowering 是否已经以 P2 typed contract 为 authoritative 输入；
  - `Resume` / `Perform` / `Handle` 相关 typed 信息是否已经下沉到 MIR metadata 或 MIR-attached side table；
  - 是否已经停止依赖 `Span` 集合、成员名 `resume`、或 HIR fallback 猜测语义。
- 必须检查的文件/位置：
  - `crates/scoopc/src/mir/lower.rs`
  - `crates/scoopc/src/mir/mod.rs`
  - `crates/scoopc/src/mir/materialize.rs`（若需要同步承接 metadata）
  - refactor MIR stage 模块
  - P2 typed contract side-table 定义位置

- 验证：
  - 重新运行 P3-T02 的全部测试与命令；
  - 额外搜索：
    - `rg "continuation_resume_call_spans|non_pure_continuation_resume_call_spans|is_continuation_resume_call|resume callee lowering pending" crates/scoopc/src/mir crates/scoopc/src/effect_refactor_pipeline`
  - 允许命中：legacy helper、测试、注释；
  - 不允许命中：refactor MIR stage 或其直接调用的 lowering 逻辑仍依赖这些旧式猜测入口。

- 完成条件：
  - review 能明确说明：P3 的 typed contract 已经真正落到了 MIR，而不是继续由 HIR 暗中托底；
  - 可进入 P3-T03。
- 依赖：P3-T02
- 完成记录：
  - （执行时填写）

## P3-T03：显式化 boundary 所在的 CFG / cleanup / evaluation context，并为 `SiteId` 与 refactor MIR 形状建立 verifier

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P3
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.12, §5.5.1-§5.5.6
  - 当前实现参考：`crates/scoopc/src/mir/lower.rs` 中 `lower_perform_expr(...)`、`lower_handle_expr(...)`、`lower_break_stmt(...)`、`lower_continue_stmt(...)`，以及 `crates/scoopc/src/mir/mod.rs::Body::validate_cfg`
- 目标：
  - 让 refactor direct-style MIR 对 boundary、`finally` / cleanup、`return` / `break` / `continue`、以及 boundary 位于更大表达式内部时的求值顺序都已经显式 CFG 化；
  - 同时建立一套专门验证 refactor MIR 形状的 verifier，确保 `SiteId`、cleanup edge、以及 effect-sensitive `Todo(...)` 不会在新路径里悄悄漂移。

- 必须实现的内容：
  1. 完成 refactor 路径上的 `handle` lowering，去掉当前与 P3 目标直接冲突的占位 terminator / rvalue。
     - 至少要处理掉当前 `crates/scoopc/src/mir/lower.rs` 中的：
       - `Rvalue::Todo("handle result pending")`
       - `TerminatorKind::Todo("handle body exit pending")`
       - `TerminatorKind::Todo("handle arm exit pending")`
       - `TerminatorKind::Todo("handle finally exit pending")`
     - refactor 路径必须把 `handle` body、arm、finally、结果汇合点、以及必要的 cleanup edge 显式编码进 MIR CFG；
     - 不允许继续把这些续点留给 P4/P5 回源码补建。
  2. 完成 refactor 路径上的 `perform` unwinding / cleanup 显式化。
     - 当前 `UnwindAction::Todo("perform unwind pending")` 不能继续作为 refactor 主路径的最终形态；
     - 必须根据当前 direct-style MIR contract，把“无 cleanup”“先进入 cleanup block 再继续 unwind”这两类情况显式区分；
     - 若某场景当前无法支持 cleanup block，也必须在 verifier 中被显式拒绝，而不是继续静默保留 `Todo`。
  3. 把 boundary 位于更大表达式内部时的 evaluation context 显式化到 MIR。
     - 至少要覆盖以下形状：
       - boundary 位于 call 实参求值内部；
       - boundary 位于 `if` / `when` 条件或分支表达式内部；
       - boundary 位于局部初始化或更大表达式中间子表达式内部；
     - 这类场景必须通过 temporaries、额外 blocks、显式 join/result slot 进入 MIR；
     - 禁止只支持“boundary 恰好是独立语句”的简单 shape。
  4. 把 `return` / `break` / `continue` / `finally` / cleanup 的控制转移显式化到 MIR CFG。
     - `while_break_continue` 一类样本中的 loop edges 必须已经是普通 MIR block/edge；
     - `handle` arm 结束后如何回到续点、`finally` 结束后如何继续，也必须是显式 edge；
     - 不允许在 emit/backend 阶段再临时依赖源码结构恢复这些控制转移。
  5. 建立 refactor MIR verifier。
     - 可以扩展 `Body::validate_cfg()`，也可以新增专门的 `validate_refactor_direct_style(...)` / 等价 API；
     - 但必须至少校验：
       - 每个 body 内 `SiteId` 唯一；
       - `Call / Perform / Handle / Resume` 的 site identity 完整；
       - cleanup target 合法且目标 block 的 `is_cleanup` 标记一致；
       - `Handle` body/arm/finally target 合法；
       - refactor 受支持 shape 中不再残留 effect/control 相关 `Todo(...)`；
       - CFG 从 `start` 出发结构合法、无悬空 target。
  6. 处理 MIR rewrite / clone / inlining 对 `SiteId` 的影响。
     - 至少检查并修正 `crates/scoopc/src/mir/inline.rs`；
     - 若 refactor 路径允许 inlining/clone 触碰 effect-sensitive body，则必须为克隆出的 `Call / Perform / Handle / Resume` site 分配 fresh `SiteId`；
     - 若当前还不能安全克隆某类 effect-sensitive node，则必须在 refactor 路径上显式阻止相关 pass 处理它们，并用测试锁定这一限制，直到支持 fresh-id 克隆为止。

- 必须遵从的约束：
  - 禁止把 `finally` / cleanup / evaluation context 的显式化留给 P5；P5 只负责 state-machine transformation，不负责回 HIR 重建 direct-style 求值语境。
  - 禁止以“这个函数 shape 很简单”为理由保留第二套 quick path；所有受支持样本都必须走同一种 direct-style CFG lowerer。
  - 禁止让 refactor 路径在 effect/control 相关位置继续输出 `Todo(...)` 作为“暂时可接受”的主线结果。

- 验证：
  1. 新增/更新 unit tests，推荐命名：
     - `refactor_mir_cfg_*`
     - `refactor_mir_site_id_*`
     - `refactor_mir_handle_finally_*`
  2. 新增/更新 refactor MIR 样本，推荐至少包括：
     - 复用 `tests/fixtures/mir/while_break_continue.scoop`
     - 复用 `tests/fixtures/mir/if_when.scoop`
     - 新增 `tests/fixtures/mir_refactor/handle_finally_boundary.scoop`
       - 场景：`handle { ... } with { ... } finally { ... }`
       - 目标：锁定 body/arm/finally/join/cleanup 的显式 CFG
     - 新增 `tests/fixtures/mir_refactor/effect_boundary_inside_expr_context.scoop`
       - 场景：boundary 位于更大表达式内部（如 call 实参、条件、局部初始化中的 effectful call / resume / handle）
       - 若推荐写法在当前语法下不合法，允许选择语义等价、且能通过 typecheck 的最小样本替代，但必须在完成记录中写明替代样本对应的 shape
  3. 运行：
      - `cargo test -p scoopc --no-default-features refactor_mir_cfg`
      - `cargo test -p scoopc --no-default-features refactor_mir_site_id`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir/while_break_continue.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/handle_finally_boundary.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/effect_boundary_inside_expr_context.scoop`
  4. 若 refactor 允许 effect-sensitive body 进入 MIR inline/clone pass，再额外运行对应定向测试，证明 fresh `SiteId` 与 verifier 仍通过。

- 完成条件：
  - refactor direct-style MIR 已显式表达 boundary、cleanup、`finally`、以及 boundary-in-expression 的 CFG 形状；
  - P4/P5 不再需要回 HIR 重建求值顺序或 cleanup 边；
  - `SiteId` 与 refactor MIR 形状已有 verifier 保护。
- 依赖：P3-T02R
- 完成记录：
  - （执行时填写）

## P3-T03R：Review CFG / cleanup / `SiteId` invariants，确认 refactor MIR 已经语义闭包

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.12, §5.5.3-§5.5.6
  - [`PLAN.md`](./PLAN.md) §2/P3
- 重点：
  - `handle` / `finally` / cleanup 是否已经变成显式 MIR CFG，而不再依赖 `Todo(...)`；
  - boundary 在更大表达式内部时，是否已经通过 temporaries + blocks 显式化；
  - `SiteId` 是否在 body 内唯一，且 clone/inlining 语义明确。
- 必须检查的文件/位置：
  - `crates/scoopc/src/mir/lower.rs`
  - `crates/scoopc/src/mir/mod.rs`
  - `crates/scoopc/src/mir/inline.rs`
  - `crates/scoopc/src/mir/materialize.rs`
  - 新增的 refactor MIR verifier 位置

- 验证：
  - 重新运行 P3-T03 的全部测试与命令；
  - 额外搜索：
    - `rg "handle result pending|handle body exit pending|handle arm exit pending|handle finally exit pending|perform unwind pending" crates/scoopc/src`
  - 要求：
    - 这些字符串若仍存在于 legacy 路径，可接受；
    - refactor MIR stage 与其直连实现不能再依赖它们作为主线产物。

- 完成条件：
  - review 能明确说明：refactor MIR 已经对 P4/P5 所需的 direct-style CFG 语义闭包；
  - 可进入 P3-T04。
- 依赖：P3-T03
- 完成记录：
  - （执行时填写）

## P3-T04：建立 refactor 专属 `dump-mir` snapshot / golden 矩阵，并冻结 P3 -> P4 的 MIR handoff contract

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P3，§2/P4
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.13.1a, §4.15, §5.4.6-§5.4.8, §8
- 目标：
  - 形成一套专门锁定 refactor direct-style MIR 的 snapshot / golden 机制；
  - 明确 legacy `tests/fixtures/mir/**` 与 refactor MIR baseline 分离，避免在 P3-P6 覆盖旧主线基线；
  - 把“P4 只消费 P3 stage 输出”的 handoff contract 固化到代码与测试中。

- 必须实现的内容：
  1. 为 refactor MIR 建立独立的 snapshot / golden 路径。
     - 推荐方案：新增 `tests/fixtures/mir_refactor/**`，并在 `crates/scoop/src/fixtures/mod.rs` 中增加对应 phase；
     - 每个样本都必须通过 `--effect-pipeline refactor` 进入新的 MIR stage；
     - 现有 `tests/fixtures/mir/**` 继续保留给 legacy baseline，禁止直接改造成 refactor goldens。
  2. 冻结 refactor `dump-mir` 的稳定 formatter。
     - 输出至少要稳定展示：
       - direct-style MIR body / CFG
       - `SiteId`
       - `Call / Perform / Resume / Handle` 的关键 metadata 或 MIR-attached contract 引用
       - cleanup / finally target
     - 若某些 contract 没有直接体现在 MIR Debug 结构中，则必须为 refactor `dump-mir` 追加一个稳定 side-table debug 区块；
     - 绝对路径、临时 id、时间戳等不稳定字段必须正规化。
  3. 把 P3 -> P4 handoff contract 写入代码注释或等价文档实体。
     - 至少要明确：
       - P4 的 canonical 输入是 refactor MIR stage 输出；
       - 其中 materialized body / `InstanceKey` / `SiteId` / MIR-attached contract 才是 authoritative 输入；
       - P4 不得再回看 P2 原始 HIR side tables 做语义判断；
       - P3 阶段尚未提供 `StepSchema` / `ContinuationSchema` / `MaterializedEffectFacts`。
  4. 建立 refactor MIR 的最小验证矩阵，至少覆盖：
     - direct call / callable value：`direct_and_fun_value_call`
     - dispatch + resume：`dispatch_and_resume_call`
     - perform + handle：`handle_perform`
     - handle + finally：`handle_finally_boundary`
     - control-flow：`while_break_continue`、`if_when`
     - boundary inside expr context：`effect_boundary_inside_expr_context`
     - `SiteId` 稳定性：使用 Rust unit tests 锁定 clone/inlining/fresh-id 行为
  5. 确保 CLI 与自动化验证使用同一 formatter / stage 输出。
     - `scoop dump-mir --effect-pipeline refactor`
     - `scoop test --fixtures tests/fixtures/mir_refactor/...`
     - Rust snapshot/unit tests
     必须共用同一 stage helper 或 formatter，不允许各自拼接不同文本。

- 必须遵从的约束：
  - 禁止覆盖 legacy `tests/fixtures/mir/*.mir` 作为 refactor baseline。
  - 禁止把“refactor 只要命令能跑通”当成验证通过；必须有稳定 snapshot/golden。
  - 禁止在 P3 就把 refactor `dump-mir` 变成 `dump-ir` / LLVM 输出；验证对象仍必须是 direct-style MIR。

- 验证：
  1. 运行新增的 refactor MIR snapshot / golden 测试入口；
  2. 运行：
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/mir_refactor/direct_and_fun_value_call.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/mir_refactor/dispatch_and_resume_call.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/mir_refactor/handle_perform.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/mir_refactor/handle_finally_boundary.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/mir_refactor/effect_boundary_inside_expr_context.scoop`
   3. 额外 CLI smoke：
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/dispatch_and_resume_call.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/handle_finally_boundary.scoop`
   4. legacy 基线抽样验证：
      - `cargo run -p scoop --no-default-features -- --effect-pipeline legacy test --fixtures tests/fixtures/mir/dispatch_and_resume_call.scoop`

- 完成条件：
  - 仓库中已有独立的 refactor MIR snapshot/golden 机制；
  - P3 -> P4 的 canonical MIR handoff contract 已被代码与测试锁定；
  - legacy MIR baseline 继续稳定保留。
- 依赖：P3-T03R
- 完成记录：
  - （执行时填写）

## P3-T04R：Review P3 阶段退出条件，确认 P4 可以只消费 MIR 而不回 HIR

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P3，§2/P4，§3
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.10-§4.13, §5.5, §8
- 重点：
  - refactor direct-style MIR stage 是否已经独立存在，并成为 `dump-mir` 与后续分析的共同入口；
  - `Call / Perform / Resume / Handle` 的 typed contract 是否已经下沉到 MIR 层；
  - CFG / cleanup / `finally` / boundary-in-expression 是否已经显式化；
  - refactor snapshot/golden 是否已与 legacy baseline 分离；
  - P4 是否已经可以只消费 P3 stage 输出，而不再回 P2/HIR 猜语义。

- 验证：
  - 重新运行 P3-T01 ~ P3-T04 的全部定向测试与命令；
  - 不再额外执行 `cargo test -p scoop` / `cargo test -p scoopc` 全 crate 测试；保持本阶段只做定向验证。

- 完成条件：
  - review 能明确说明：P3 已经完成“direct-style MIR 新路径落地”的阶段目标；
  - P4 可以在不重新讨论 `resume` surface contract、`perform` payload contract、`handle` CFG shape、或 boundary 求值顺序的前提下直接进入 effect facts 落地。
- 依赖：P3-T04
- 完成记录：
  - （执行时填写）
