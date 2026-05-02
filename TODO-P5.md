# TODO（P5：late-lowered `Step` 路径落地）

> 生成时间：2026-05-02  
> 设计基线：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 前置条件：`TODO-P4.md` 的原始主线任务已完整完成；新增的 `P4-T05b` contract 纠偏任务需在继续推进 `P5-T05` 前完成；refactor `effect-facts` stage、`MaterializedEffectFacts`、`dump-effect-facts` 与 P4 -> P5 handoff contract 已存在并稳定。  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 本阶段目标：在 refactor 新路径上把 P3 的 direct-style MIR 与 P4 的 `MaterializedEffectFacts` 统一转换为 LLVM 之前的 late-lowered internal representation；该 representation 必须显式承载 `Step_F` enum、canonical dynamic `invoke(args_tuple) -> Step_F`、continuation object、internal resume interfaces、整函数 state graph、frame schema、boundary/resume 映射，以及 `ImplPlan` 驱动下的具体版本形态，同时保持“不接 LLVM、不重做高层 effect 分析、不按 code shape 分叉 lowering”的边界。

## 全局约束

- [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 是本阶段唯一设计基线；若实现过程中需要改变主张，必须先回写该文档，再继续实现。
- [`PLAN.md`](./PLAN.md) 与 [`TODO-P0.md`](./TODO-P0.md)、[`TODO-P1.md`](./TODO-P1.md)、[`TODO-P2.md`](./TODO-P2.md)、[`TODO-P3.md`](./TODO-P3.md)、[`TODO-P4.md`](./TODO-P4.md) 是本阶段执行前提；P5 不得重新开启 P0-P4 已收敛的 CLI / dispatcher / typed HIR / direct-style MIR / effect facts 讨论。
- 本阶段只处理 refactor 新路径上的 late effect lowering。
  - 明确禁止：在 P5 中实现 LLVM IR 生成、runtime ABI 物理布局、GC root lowering、stackmap 发射、或任何 backend-only 逻辑；这些属于 P6。
  - 明确禁止：在 P5 中重新求解 `resolved_outward_cases` / `needs_reentry` / `impl_plan`；这些在 P4 已经闭合，P5 只能消费结果。
- P5 的 canonical 输入必须固定为“当前 canonical materialized MIR snapshot + `MaterializedEffectFacts`”。
  - 允许消费：
    - P4 stage 输出中的 canonical MIR 查询面；
    - 与其绑定的 `TypeStore` / materialized callable metadata；
    - `MaterializedEffectFacts` 中的 `StepSchema`、`ContinuationSchema`、callable/block/site facts；
    - 显式外部输入：opt level、feature flags、target ABI、预算参数。
  - 明确禁止：
    - 回 AST / HIR / typecheck 内部缓存补语义；
    - 回 P2 typed HIR side tables 重新解释 `resume` / `perform` / `handle`；
    - 回 LLVM codegen / runtime bridge / legacy handler stack 实现重建 lowering 合同。
- 本阶段若需要新模块，推荐新增独立模块树：`crates/scoopc/src/effect_lowered/`。
  - 推荐最小拆分：`mod.rs`、`ir.rs`、`builder.rs`、`segment.rs`、`frame.rs`、`materialize.rs`、`opt.rs`、`dump.rs`；
  - 若 P0-P4 实际落地时采用了不同命名，可使用等价位置；
  - 但必须在完成记录中明确写出“当前仓库中的实际模块路径 <-> 本 TODO 推荐路径”的映射，避免后续 agent 误判。
- P5 必须产出一个**独立的 late-lowered stage 输出类型**，不能把结果散落成：
  - 零散 side tables；
  - `ProgramFacts`/`InstanceSummary` 附加字段；
  - `MaterializedMir` 上几个 ad-hoc debug helper；
  - 或仅存在于 LLVM codegen 调试路径中的临时结构。
- 所有 effectful callable 都必须先进入**同一套** late-lowering 框架。
  - `NoOutward` 只能理解为同一框架内的退化结果，而不是“因为函数简单所以跳过 P5”；
  - 明确禁止：保留“单 `perform` 快路径”“线性 body 专用 lowering”“仅 `handle` 局部状态机主线”“tail-`resume` 专用通道”等第二套 lowering 入口；
  - 若未来要优化，也只能作为统一 transformation 之后的压缩/消除/特化 pass，而不是绕过主 transformation。
- `Step_F` 的 canonical 形状必须严格由 `StepSchema(F)` 决定，并在 P5 物化为编译器内部 `enum`。
  - `Complete` 与每个 case 的 variant 必须一一对应；
  - `CaseTag` 必须沿用 P4 schema 的稳定编号；
  - `SingleCase(case_tag)` 只能缩小**可达 case 集/内部 dispatch**，不能引入第二个“窄版 `Step` 类型”，也不能重排 tag。
- canonical dynamic callable surface 必须按 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.9 固定为：
  - `invoke(args_tuple) -> Step_F`；
  - 不允许把 dynamic boundary 降级成 erased `Signal { tag, payload }`；
  - 不允许在 P5 就设计两套不同的用户可见 surface（例如“optimized direct entry”与“canonical dynamic entry”并存）。
- continuation object 必须按 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.2-§5.3.5 建模。
  - continuation 是编译器生成的内部对象；
  - 它实现对应 effect family 的内部 resume interfaces；
  - 每个 resume method 的返回类型统一为同一个 `Step_F<T>`；
  - method 参数类型来自对应 case 的 `ContinuationSchema.resume_tuple_ty`；
  - interface method 集必须完整；对于不可能合法调用到的方法，允许 body 为 `unreachable`，但不能从类型上删掉。
- capture 链必须被吸收到 continuation/state-machine 模型本身。
  - 明确禁止：在 P5 的新主线中继续把 ambient TLS handler stack / snapshot / bridge 当作语义前提；
  - 明确禁止：继续依赖 `crates/scoopc/src/llvm/codegen/effect/state_machine_bridge.rs` 这一类 backend bridge 作为 P5 correctness 前提。
- dropped continuation 语义必须严格遵守 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.7。
  - 被 drop 的 continuation 表示“剩余语言级计算被放弃”；
  - 任何尚未执行到的 pending `finally` / cleanup block 都不再执行；
  - runtime/GC `cleanup hook` 不属于 continuation 继续执行语义，P5 不得把它编织回 state graph。
- runtime error 必须继续被视为普通 effect 分支的一部分，遵守 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.9。
  - `ContinuationAlreadyResumed` 等路径必须进入普通 outward case / `Step_F` 分支语义；
  - 明确禁止：在 P5 中发明“runtime error hidden trap channel”作为第二条传播通道。
- P5 消费 `ContinuationSchema` 时必须继续区分 source-visible `surface_ty` 与 internal `out_step_schema`。
  - `surface_ty` 的 effect 参数只表示源码层 `Continuation<ResumeTuple, Answer, eff Out>` 中的 residual `Out`；
  - compiler-generated one-shot runtime-error case 可以只存在于 `out_step_schema` / `StepSchema`，而不体现在 `surface_ty.eff`；
  - P5 不得从 `cases(out_step_schema)` 反推或扩大 `surface_ty`，也不得因为 `surface_ty` 未显式包含 runtime error 就漏掉 one-shot lowering。
- `Managed ABI` / `extern` / FFI 不是本阶段目标，遵守 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.8。
  - P5 不得尝试让 extern / Managed ABI 承载 `Step_F`、continuation 或 resume interface；
  - 若现有实现中这些边界仍是 pure-only，P5 必须保持该约束不变。
- 本阶段可以参考当前 legacy 模块，但不能把它们直接当作 refactor authoritative 实现：
  - `crates/scoopc/src/effect/state_machine/mod.rs`
  - `crates/scoopc/src/effect/state_machine/analysis.rs`
  - `crates/scoopc/src/effect/state_machine/segments.rs`
  - `crates/scoopc/src/effect/state_machine/transform.rs`
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_bridge.rs`
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
  - 允许做法只有两种：
    1. 把其中完全中立、且不含 code-shape 分叉/LLVM 依赖的部分抽成共享基础设施；
    2. 若做不到，则在 refactor 新路径中重建/复制逻辑。
  - 明确禁止：让 refactor P5 直接调用 legacy `effect/state_machine/**` 主逻辑，然后只在外层换一层壳。
- 所有优化级别必须共用同一条 late-lowering 管线。
  - `O0` / debug build 不允许切到单独的 effect lowering 通道；
  - 差异只允许体现在：`impl_plan`（P4 已决定）、后续窄优化是否开启、以及优化预算；
  - 不允许因为 `O0` 就重新走 legacy 局部状态机/LLVM bridge。
- 本阶段不做 full regression。
  - 只做 P5 要求的 late-lowered 单元测试、snapshot/golden、以及必要的 refactor CLI smoke；
  - 不执行 `cargo test --all`；
  - 不执行 `cargo run -p scoop -- test` 的全量 fixture 扫描；
  - 不执行 P6/P7/P8 的 LLVM / run-pass / runtime_gc / spec-fixtures 完整矩阵。
- 所有需要触发新路径的验证，都必须通过 `--effect-pipeline refactor` 进入，或通过与该 CLI 路径共用同一 stage helper 的 Rust 测试入口进入；禁止新增只在测试中存在的语义旁路。

## [DONE] P5-T01：建立 refactor late-lowering stage 与独立 late-lowered representation 边界

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P5
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.10, §4.16, §5.5.1, §8
  - 当前实现参考：
    - `crates/scoopc/src/effect/state_machine/**`
    - `crates/scoopc/src/effect_facts/**`
    - `crates/scoopc/src/mir/pass_view.rs`
    - `crates/scoopc/src/llvm/codegen/effect/**`
- 目标：
  - 在 refactor 新路径上建立一个明确的 late-lowering stage；
  - 让 P5 的输入/输出边界从一开始就独立于 legacy `effect/state_machine/**` 与 LLVM backend；
  - 为后续任务提供一个稳定的 stage 输出类型，使 P6 能只消费该输出，不再回 P3/P4 重做 segmentation / frame lifting / boundary 识别。

- 必须实现的内容：
  1. 在 `scoopc` 中新增一套承载 P5 主线的独立模块树。
     - 推荐位置：`crates/scoopc/src/effect_lowered/`；
     - 推荐最小模块：
       - `mod.rs`
       - `ir.rs`
       - `builder.rs`
       - `segment.rs`
       - `frame.rs`
       - `materialize.rs`
       - `opt.rs`
       - `dump.rs`
     - 若采用等价拆分也可，但必须保证：
       - late-lowered representation 不混在 legacy `effect/state_machine/**`；
       - 不混在 LLVM codegen 模块；
       - P6 能从一个清晰、稳定的编译器中层入口消费它。
  2. 在 `crates/scoopc/src/lib.rs` 中为该子系统建立正式模块入口。
     - 它必须服务 refactor pipeline 的生产阶段；
     - 不能只作为测试 helper 或 LLVM emitter 的内部私货。
  3. 在 refactor pipeline 下新增 late-lowering stage 模块。
     - 推荐位置：`crates/scoopc/src/effect_refactor_pipeline/effect_lowering_stage.rs`；
     - 输入必须是 P4 的 refactor effect-facts stage 输出；
     - 明确禁止：在 legacy `effect/state_machine/transform.rs` 或 `llvm/frontend.rs` 里加入 `if pipeline == Refactor` 作为替代。
  4. 定义一个 refactor late-lowering stage 输出类型。
     - 名称可自定，例如 `RefactorEffectLoweredStageOutput`；
     - 该输出至少要显式承载：
       - 绑定到本次 lowering 的 canonical MIR snapshot 查询面；
       - 绑定的 `MaterializedEffectFacts`（或对其稳定引用）；
       - 最终 `LateLoweredProgram` / 等价容器；
       - 供 P6 使用的稳定访问 API/formatter。
     - 该输出类型的文档注释中必须明确写出：
       - 输入是 P4 stage 输出；
       - P5 不回 HIR/typecheck；
       - P6 只应把该输出翻译到 LLVM，而不是再做高层 effect lowering 设计。
  5. 明确 late-lowering stage 与 P4 facts 的生命周期绑定关系。
     - 同一次 stage 输出只绑定到一个 canonical MIR snapshot；
     - 若 P5 内部后续 pass 做了结构性改写，应在自己的 late-lowered representation 内继续工作，而不是倒回去 patch P3/P4 产物；
     - 对外暴露的 stage 输出不允许含有“部分 callable 已降低、部分仍停留在 direct-style”的混合半成品状态。
  6. 为该 stage 提供共同入口，供以下调用方复用：
     - 后续 `dump-effect-lowered` CLI；
     - P5 的 snapshot/golden 测试；
     - P6 的 LLVM lowering stage；
     - Rust 单元测试。
     - 明确禁止：CLI、测试、P6 各自绕过 stage 自己拼装输入。
  7. 若 P0-P4 实际模块命名与本 TODO 推荐值不同，本任务必须在代码注释或完成记录中写清等价映射，避免后续 agent 根据旧 TODO 名称找不到入口。

- 必须遵从的约束：
  - 禁止在 `crates/scoopc/src/effect/state_machine/**` 的 legacy 业务实现里直接加 pipeline 分支，把 P5 新逻辑塞进去。
  - 禁止让 P5 stage 输出只存在于 LLVM frontend/backend 的局部变量里。
  - 禁止把 P5 结果作为 `MaterializedMir` 上几个 ad-hoc side table 暗藏，而没有独立 stage 输出类型。
  - 禁止把“先让 P6 自己重建一部分 state graph，等后面再整理”为可接受路线。

- 验证：
  1. 新增/更新单元测试，推荐命名：`refactor_effect_lowered_stage_*`，至少覆盖：
     - late-lowering stage 输出类型可构造；
     - stage 显式接收 P4 effect-facts stage 输出；
     - 该 stage 不依赖 LLVM emitter 或 legacy `effect/state_machine` 主入口。
  2. 运行：
      - `cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`
  3. 若需要最小 smoke，允许额外运行：
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-effect-facts tests/fixtures/effect_facts/dispatch_and_resume_call.scoop`
     - 该 smoke 只是确认上游输入已存在，不是 P5 完成条件。

- 完成条件：
  - refactor 新路径已拥有独立的 late-lowering stage 与 stage 输出类型；
  - P6 已经有一个明确、稳定、非 LLVM 的中层输入可接；
  - 后续 P5-T02 及之后的任务都能只在这条新主线上推进。
- 依赖：`TODO-P4.md` 最后一项 review 完成
- 完成记录：
  - 2026-05-02：完成 `P5-T01`，新增独立的 `crates/scoopc/src/effect_lowered/` 子系统，并在 `crates/scoopc/src/effect_refactor_pipeline/effect_lowering_stage.rs` 中建立 refactor late-lowering stage，使 P5 新路径从一开始就拥有独立于 legacy `effect/state_machine/**` 与 LLVM backend 的正式阶段入口。
  - `crates/scoopc/src/effect_lowered/{mod,ir,builder,dump}.rs` 现已固定本任务的最小模块树：`ir.rs` 定义 `LateLoweredProgram` / `LateLoweredCallable` 作为独立 late-lowered representation 容器，`builder.rs` 统一承接“canonical MIR snapshot + MaterializedEffectFacts -> LateLoweredProgram”的初始组装入口，`dump.rs` 提供稳定 formatter，`mod.rs` 记录了与 TODO 推荐拆分的映射关系。当前实际落地中，TODO 推荐的 `materialize.rs` 最小职责由 `builder.rs` 承接；`segment.rs` / `frame.rs` / `opt.rs` 留待后续 P5 任务按顺序补入，而不是在本任务提前伪造空壳。
  - `crates/scoopc/src/lib.rs` 已新增 `pub mod effect_lowered;`，确保该子系统属于 `scoopc` 正式 middle-end API，而不是测试或 LLVM emitter 的局部私货。
  - `RefactorEffectLoweredStageOutput` 已显式承载：P4 的 `RefactorEffectFactsStageOutput`、与之绑定的 canonical MIR snapshot 查询面、authoritative `MaterializedEffectFacts`、以及最终 `LateLoweredProgram`。其文档注释明确写死：输入必须是 P4 stage 输出、P5 不回 HIR/typecheck、结构性 rewrite 必须继续在 late-lowered IR 内工作、P6 只应把该输出翻译到 LLVM 而不是再重做高层 effect lowering 设计。
  - `crates/scoopc/src/effect_refactor_pipeline/mod.rs` / `refactor.rs` 现已提供共同入口 `build_effect_lowered_stage_output(...)` 与 `load_effect_lowered_stage_output_for_dump(...)`，供后续 `dump-effect-lowered` CLI、P5 snapshot/golden、P6 lowering stage 与 Rust 单测复用；它们统一以 P4 的 `RefactorEffectFactsStageOutput` 为输入，而不是让调用方各自拼装 stage 输入。
  - 代码边界复核：本任务没有改动 `crates/scoopc/src/effect/state_machine/**` 或 `crates/scoopc/src/llvm/codegen/effect/**` 的业务实现；new stage 仅通过新的 `effect_lowered` 子系统和 `effect_refactor_pipeline` glue 接入，保持了“P5 不借壳 legacy state-machine，也不混入 LLVM backend”的边界约束。
  - 新增/更新测试：`crates/scoopc/src/effect_refactor_pipeline/effect_lowering_stage.rs` 中的 `refactor_effect_lowered_stage_output_is_constructible`、`refactor_effect_lowered_stage_explicitly_consumes_p4_effect_facts_stage_output`、`refactor_effect_lowered_stage_has_no_legacy_state_machine_or_llvm_imports`，以及 `crates/scoopc/src/effect_refactor_pipeline/mod.rs` 中的 `refactor_effect_lowered_stage_dispatcher_loads_stage_output`。
  - 2026-05-02：按详细任务文件的完成判定规则复验后，已补齐本任务标题的 `[DONE]` 标记，并同步更新 `TODO.md` 索引；`PLAN.md` 无需改动。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`、`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。

## [DONE] P5-T01R：Review late-lowering stage 边界，确认新路径没有借壳 legacy `effect/state_machine` 或 LLVM backend

- 参考：
  - [`PLAN.md`](./PLAN.md) §0，§2/P5
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.10, §4.16, §5.5.1, §8
- 重点：
  - refactor late-lowering stage 是否已经成为独立阶段，而不是换壳调用 legacy `effect/state_machine/**`；
  - stage 输出是否已经独立存在，并成为后续 P6 唯一允许消费的中层入口；
  - 是否仍避免把新逻辑混入 LLVM emitter / legacy transform。
- 必须检查的文件/位置：
  - 新增的 `crates/scoopc/src/effect_lowered/**`
  - 新增的 `crates/scoopc/src/effect_refactor_pipeline/effect_lowering_stage.rs`
  - `crates/scoopc/src/lib.rs`
  - `crates/scoopc/src/effect/state_machine/**`
  - `crates/scoopc/src/llvm/codegen/effect/**`

- 验证：
  - 重新运行 P5-T01 的全部测试与命令；
  - 额外搜索：
    - `rg "EffectPipelineMode|refactor|legacy" crates/scoopc/src/effect_lowered crates/scoopc/src/effect_refactor_pipeline crates/scoopc/src/effect/state_machine crates/scoopc/src/llvm/codegen/effect`
  - 要求：
    - 允许命中：新 stage/新模块、测试、注释；
    - 不允许命中：在 legacy `effect/state_machine/**` 或 `llvm/codegen/effect/**` 业务主实现里新增 refactor 分支并把 P5 主线混进去。

- 完成条件：
  - review 能明确证明：P5 新主线已从 legacy state-machine 主实现与 LLVM backend 分离；
  - 可进入 P5-T02。
- 依赖：P5-T01
- 完成记录：
  - 2026-05-02：完成 `P5-T01R` review。复核 `crates/scoopc/src/effect_lowered/**`、`crates/scoopc/src/effect_refactor_pipeline/effect_lowering_stage.rs`、`crates/scoopc/src/effect_refactor_pipeline/refactor.rs` 与 `crates/scoopc/src/lib.rs` 后，确认 P5 late-lowering 新路径已通过独立 `effect_lowered` 子系统与 refactor stage glue 落地，而不是换壳调用 legacy `effect/state_machine/**`。
  - `git diff --name-only HEAD^ HEAD -- crates/scoopc/src/effect_lowered crates/scoopc/src/effect_refactor_pipeline crates/scoopc/src/effect/state_machine crates/scoopc/src/llvm/codegen/effect` 结果显示，上一提交只触碰了 `effect_lowered/**` 与 `effect_refactor_pipeline/**`；没有把 refactor late-lowering 逻辑写进 legacy `effect/state_machine/**` 或 `llvm/codegen/effect/**` 业务实现。
  - 文本搜索复核：在 `crates/scoopc/src/effect/state_machine/**` 与 `crates/scoopc/src/llvm/codegen/effect/**` 中搜索 `EffectPipelineMode|refactor|legacy` 后，未发现新增的 refactor/pipeline 分支被混入 legacy 业务主线；命中的 `legacy` 仅来自既有实现命名、注释或原有 ABI helper。
  - 依赖复核：`crates/scoopc/src/effect_lowered/**` 未引用 `crate::effect::state_machine` 或 `crate::llvm`；`crates/scoopc/src/effect_refactor_pipeline/effect_lowering_stage.rs` 也仅依赖 `effect_facts`、`effect_lowered`、`mir` 与 `ty`，并通过单测 `refactor_effect_lowered_stage_has_no_legacy_state_machine_or_llvm_imports` 锁定该边界。
  - refactor 路径复核：`crates/scoopc/src/effect_refactor_pipeline/refactor.rs` 中 `StageKind::LateLowering` 的入口直接调用 `effect_lowering_stage::run(...)`，说明 refactor late-lowering stage 已成为正式阶段入口；后续 P6 可继续只消费 `RefactorEffectLoweredStageOutput`。
  - 2026-05-02：按 detailed TODO 完成判定规则补齐本任务标题的 `[DONE]` 标记，并同步更新 `TODO.md` 索引；`PLAN.md` 无需改动。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`、`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。

## [DONE] P5-T02：定义 late-lowered representation 的最终目标形状，包括 version key、state graph、frame schema、`Step` / continuation carrier 壳层

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P5
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.9, §5.2, §5.3.2-§5.3.5, §5.5.1, §7.2-§7.3, §8
  - 当前实现参考：
    - `crates/scoopc/src/effect_facts/schema.rs`
    - `crates/scoopc/src/effect_facts/facts.rs`
    - `crates/scoopc/src/mir/materialize.rs`
- 目标：
  - 在真正实现 whole-function transformation 之前，先把 P5 输出 representation 的目标形状钉死；
  - 让后续任务始终往这套最终形状上填内容，而不是边做边发明临时结构；
  - 明确 `Step_F`、continuation object、resume interface、callable version、state graph、frame schema 的键空间与容器边界。

- 必须实现的内容：
  1. 定义 late-lowered 顶层容器。
     - 推荐名称：`LateLoweredProgram`；
     - 它至少要包含：
       - materialized `Step` type definitions / schema 映射；
       - internal resume interface definitions；
       - continuation object definitions；
       - late-lowered callable versions；
       - 如需调试输出，则提供稳定 formatter 所需的元信息。
  2. 定义 callable version identity。
     - 必须显式区分 surface instance 与实现版本；
     - 推荐至少包含：
       - surface callable identity（P4 的 callable key / `InstanceKey` 或其等价包装）
       - `allowed_row` 家族身份
       - `impl_plan`
       - `needs_reentry`
     - 若当前实现仍希望保留更接近 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §7.2 的 `BodyVersionKey = (symbol, type_args, allowed_row, impl_ops, needs_reentry)` 形态，也可以；
     - 但必须保证：
       - 不跨不同 `allowed_row` 共享版本；
       - `SingleCase(case_tag)` 的版本 identity 至少区分具体 `case_tag`；
       - `NoOutward` / `CanonicalFull` 也各自可稳定区分。
  3. 定义 late-lowered callable 的核心结构。
     - 推荐名称：`LateLoweredCallable` / `LateLoweredBodyVersion`；
     - public 形状至少要显式包含：
       - `body_version_key`
       - `step_schema`
       - `impl_plan`
       - `dynamic_invoke_entry`
       - `state_graph`
       - `frame_schema`
       - `boundary_map`
       - `resume_state_map`
       - 与 continuation object / resume interfaces 的关联
     - 不允许这些信息散落在多个松散 side table 里让 P6 自己拼装。
  4. 定义 state graph 的 identity 与最小公共结构。
     - 至少要有：
       - `StateId`
       - `BoundaryId`
       - `FrameSlotId`
       - callable version 内的 entry/complete/cleanup/drop 等稳定标识
     - `StateId` 必须只在当前 callable version 内有意义；
     - `BoundaryId` 必须能稳定映射回触发它的 `SiteId` 或 boundary kind；
     - 明确禁止：仅靠 block index / debug 顺序号 / dump 行号当作外部可依赖 identity。
  5. 定义 frame schema 与 slot 分类。
     - 最低要求必须能区分：
       - 源码 local
       - 编译器临时值 / 中间表达式结果
       - join/phi-like 值
       - `handle` binder
       - resume payload / replayed answer/result slot
       - 系统字段（state tag、resume payload carrier、cleanup flag、one-shot flag、completion tag 等）
     - 若实现中使用单独的 `FrameFieldKind` / `SystemSlotKind` 枚举，必须稳定且可 dump。
  6. 定义 `Step_F` materialization 的中层表示。
     - 必须体现：
       - 每个 `StepSchemaId` 对应一个内部 `enum` 定义；
       - `Complete` variant；
       - 每个 case 对应的 variant；
       - 对 `payload_tuple_ty == ()` 或 `complete_ty == ()` 的零载荷表示能力。
     - 明确禁止：把 `Step_F` 表示成 erased `tag + payload blob` 结构作为最终中层合同。
  7. 定义 internal resume interface 与 continuation object 的中层表示壳层。
     - resume interface 必须显式表示：
       - interface family identity
       - 完整 method 集
       - 每个 method 的 `resume_tuple_ty`
       - 统一的 `Step_F<T>` 返回类型
     - continuation object 定义至少要显式表示：
       - object identity / 所属 callable version
       - 它实现了哪些 interface families
       - 它捕获哪些 frame/context 引用
       - 哪些 methods 为可达实现，哪些是 `unreachable`
  8. 为 canonical dynamic callable surface 预留稳定表示。
     - 必须显式建模 `invoke(args_tuple) -> Step_F`；
     - direct/static path 当前允许直接复用同一 entry；
     - 不允许在 P5 里再发明第二套用户可见 surface。
  9. 定义稳定的内部 formatter / Debug helper。
     - 当前不要求 CLI 子命令；
     - 但 Rust tests 必须已经能稳定看到上述 representation 的关键字段；
     - 以便 T03-T06 在没有最终 `dump-effect-lowered` 命令前也能锁定结构。

- 必须遵从的约束：
  - 禁止把 `SingleCase` 做成一个“缩小 variant 数量”的第二套 `Step` 类型；canonical `Step_F` 仍由 `StepSchema` 决定。
  - 禁止把 callable version identity 设计成可能跨不同 `allowed_row` 混用的键。
  - 禁止在 representation 里使用 `Any`、`Todo(...)`、裸字符串 effect 名/FQN 作为最终的 step/interface/callable 身份。
  - 禁止把 continuation object 仅表示成“以后 codegen 再想办法生成的黑盒”；P5 中层必须已具备可比较、可 dump、可测试的显式定义。

- 验证：
  1. 新增/更新单元测试，推荐命名：
     - `refactor_late_lowered_ir_*`
     - `refactor_body_version_key_*`
     - `refactor_step_materialization_shell_*`
     - `refactor_resume_interface_shell_*`
  2. 测试至少覆盖：
     - callable version key 不跨 `allowed_row` 冲突；
     - `SingleCase(case_tag)` 与 `CanonicalFull` 版本可区分；
     - canonical `Step_F` 仍保留完整 case/tag 身份；
     - continuation object / resume interface 壳层已可见且方法集完整；
     - frame slot 分类可稳定输出。
  3. 运行：
      - `cargo test -p scoopc --no-default-features refactor_late_lowered_ir`
      - `cargo test -p scoopc --no-default-features refactor_body_version_key`

- 完成条件：
  - late-lowered representation 的最终目标形状已经固定；
  - 后续 T03-T06 只需把算法与内容填进这套形状，而不再发明临时 IR；
  - P6 未来可直接依赖这一 representation 生成 LLVM。
- 依赖：P5-T01R
- 完成记录：
  - 2026-05-02：完成 `P5-T02`。`crates/scoopc/src/effect_lowered/ir.rs` 现已把 late-lowered representation 扩展为明确的最终目标骨架：`LateLoweredProgram` 顶层容器显式承载 `step_types`、`resume_interfaces`、`continuation_objects` 与 `callables`；`LateLoweredBodyVersionKey` 显式固定 surface instance、`allowed_row`、`impl_plan`、`needs_reentry`；`LateLoweredCallable` 显式挂载 `dynamic_invoke_entry`、`state_graph`、`frame_schema`、`boundary_map`、`resume_state_map`、`continuation_object` 与 `resume_interfaces`，从而把 P6 未来需要消费的关键 identity / container 边界全部收口到同一套 IR 上。
  - `StateId` / `BoundaryId` / `FrameSlotId`、`LateLoweredStateGraph`、`LateLoweredBoundaryMap`、`LateLoweredResumeStateMap`、`LateLoweredFrameSchema`、`LateLoweredFrameSlotKind`、`SystemSlotKind` 已在 `effect_lowered/ir.rs` 中固定为正式数据模型。当前仓库的实际模块映射仍沿用 P5-T01 的最小拆分：`ir.rs` 同时承载 TODO 推荐的核心 IR + frame/boundary shell，`builder.rs` 负责从 P4 输出构造这些 shell，`dump.rs` 提供稳定 formatter；没有额外拆出独立 `frame.rs`/`segment.rs`，避免在 T02 提前制造空模块。
  - `Step_F` shell 已固定为 `LateLoweredStepType` + `LateLoweredStepCase`：每个 `StepSchemaId` 都对应一个独立内部 step 定义，显式保留 `Complete` 结果类型、continuation object type，以及每个 case 的 `CaseTag`、`ConcreteOpKey`、payload tuple type、`ContinuationSchemaId`。`SingleCase(case_tag)` 只影响 callable version 的 `ImplPlan` 与 continuation method reachability；不会缩成第二套窄 `Step` 类型。
  - internal resume interface / continuation object shell 已固定为 `LateLoweredResumeInterface`、`LateLoweredResumeMethod`、`LateLoweredContinuationObject`、`LateLoweredContinuationMethod` 与 `LateLoweredContinuationCapture`。其中 resume interface 显式保留 interface identity、完整 method 集、每个 method 的 `resume_tuple_ty` 与统一返回的 `StepSchemaId`；continuation object 显式保留 object identity、所属 body version、实现的 interface families、capture 引用以及 method 的 reachable/unreachable 标记，避免把 continuation 留到 P6 再当黑盒补想象。
  - `crates/scoopc/src/effect_lowered/builder.rs` 现已实际使用 P4 的 `MaterializedEffectFacts` 填充上述壳层：从 `step_schemas()` 构造 canonical step shells；从 `continuation_schemas()` 构造 resume interface methods；为每个 callable version 创建 continuation object shell，并按 `ImplPlan` 把 method 可达性固定下来；同时以统一的最小 `state_graph/frame/boundary/resume` 空骨架承接后续 T03-T06，而不是再另起新的临时 representation。
  - `crates/scoopc/src/effect_lowered/dump.rs` 已扩展稳定 formatter，使 Rust 单测在没有最终 `dump-effect-lowered` CLI 前，仍能稳定断言 step/interface/continuation/frame/state/boundary 的关键字段。`FrameSlotKind` dump 现在显式暴露 `SourceLocal`、`CompilerTemporary`、`JoinValue`、`HandleBinder`、`ResumePayload` 与全部系统字段分类。
  - 新增/更新测试：`refactor_body_version_key_keeps_allowed_row_in_identity`、`refactor_body_version_key_distinguishes_single_case_and_canonical_full_versions`、`refactor_late_lowered_ir_step_materialization_shell_keeps_canonical_cases_for_single_case_versions`、`refactor_late_lowered_ir_resume_interface_shell_records_complete_methods_and_reachability`、`refactor_late_lowered_ir_stable_dump_exposes_frame_slot_categories`、`refactor_late_lowered_ir_builder_materializes_program_shells_from_effect_facts`。其中 manual shell tests 负责锁定 multi-case `Step`/continuation shape，真实 stage test 通过 `tests/fixtures/effect_facts/single_case_impl_plan.scoop` 复验 P4->P5 stage 已能发布这套 shell。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_late_lowered_ir`、`cargo test -p scoopc --no-default-features refactor_body_version_key`、`cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`、`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。
  - 2026-05-02：按详细 TODO 的完成判定规则补齐本任务标题的 `[DONE]` 标记，并同步更新 `TODO.md` 索引；`PLAN.md` 无需改动。

## [DONE] P5-T02R：Review late-lowered representation，确认 version key / `Step` / continuation carrier 已按最终形态固定

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.9, §5.2, §5.3.2-§5.3.5, §7.2-§7.3
  - [`PLAN.md`](./PLAN.md) §2/P5
- 重点：
  - callable version key 是否已正确保留 `allowed_row` 家族边界；
  - canonical `Step_F` 是否仍由 `StepSchema` 决定，而没有被 `SingleCase` 改造成第二套窄类型；
  - continuation object / resume interface 是否已经是显式可 dump 的 carrier，而不是留给 P6 的黑盒。
- 必须检查的文件/位置：
  - 新增的 `crates/scoopc/src/effect_lowered/ir.rs`
  - 新增的 `crates/scoopc/src/effect_lowered/mod.rs`
  - 与 body version key / step / continuation shell 相关的实现位置

- 验证：
  - 重新运行 P5-T02 的全部测试与命令；
  - 额外搜索：
    - `rg "Signal \{|Any|Todo\(|SingleCase.*Step|CanonicalFull.*Step" crates/scoopc/src/effect_lowered crates/scoopc/src/effect_refactor_pipeline`
  - 要求：
    - 允许命中：测试、注释；
    - 不允许命中：把 erased signal、`Any`、`Todo(...)` 当成新的 representation 最终合同。

- 完成条件：
  - review 能明确说明：P5 representation 已按最终目标形态固定；
  - 可进入 P5-T03。
- 依赖：P5-T02
- 完成记录：
  - 2026-05-02：完成 `P5-T02R` review。复核 `crates/scoopc/src/effect_lowered/ir.rs`、`crates/scoopc/src/effect_lowered/builder.rs`、`crates/scoopc/src/effect_lowered/dump.rs` 与 `crates/scoopc/src/effect_lowered/mod.rs` 后，确认 late-lowered representation 已按最终目标形态固定：`LateLoweredBodyVersionKey` 显式保留 `surface_instance + allowed_row + impl_plan + needs_reentry`，`LateLoweredCallable` 继续把 `dynamic_invoke_entry`、`state_graph`、`frame_schema`、`boundary_map`、`resume_state_map`、`continuation_object` 与 `resume_interfaces` 作为统一 IR 容器字段暴露给后续 P5/P6。
  - version key 复核：`LateLoweredBodyVersionKey` 以 `allowed_row` 参与相等性/哈希，`ImplPlan::SingleCase(case_tag)` / `ImplPlan::CanonicalFull` / `ImplPlan::NoOutward` 也都进入同一稳定键空间；`refactor_body_version_key_keeps_allowed_row_in_identity` 与 `refactor_body_version_key_distinguishes_single_case_and_canonical_full_versions` 重新验证了这些边界，确认不会跨不同 `allowed_row` 或不同 `impl_plan` 共享 body version。
  - canonical `Step_F` 复核：`LateLoweredStepType` / `LateLoweredStepCase` 继续直接按 `StepSchema` 物化完整 case/tag 集；`LateLoweredProgramBuilder::build_step_type(...)` 总是遍历 `step_schema.cases()` 构建 canonical step shell，而 `build_continuation_object(...)` 只把 `ImplPlan::SingleCase` 下沉到 continuation method reachability。`refactor_late_lowered_ir_step_materialization_shell_keeps_canonical_cases_for_single_case_versions` 重新确认 `SingleCase` 不会收缩成第二套窄 `Step` 类型。
  - continuation carrier 复核：`LateLoweredResumeInterface`、`LateLoweredResumeMethod`、`LateLoweredContinuationObject`、`LateLoweredContinuationMethod`、`LateLoweredContinuationCapture` 已把 interface family、完整 method 集、统一返回的 `StepSchemaId`、capture 引用以及 reachable/unreachable method 形态显式固化在中层 IR 中；`dump.rs` 的稳定 formatter 也会把 version key、frame slot 分类、continuation object 与 method reachability 一并输出，说明它们不是留给 P6 再补的黑盒。`refactor_late_lowered_ir_resume_interface_shell_records_complete_methods_and_reachability` 与 `refactor_late_lowered_ir_stable_dump_exposes_frame_slot_categories` 已重新覆盖这些 contract。
  - 额外搜索复核：执行 `rg "Signal \{|Any|Todo\(|SingleCase.*Step|CanonicalFull.*Step" crates/scoopc/src/effect_lowered crates/scoopc/src/effect_refactor_pipeline` 后，仅命中 `crates/scoopc/src/effect_refactor_pipeline/hir_stage.rs` 中既有的 `StmtKind::Todo(_)` / `ExprKind::Todo(_)` typed-HIR 遍历分支；这些命中属于上游语法节点处理，不是 late-lowered representation 最终合同。`effect_lowered/**` 与 late-lowering stage 中未发现 erased signal、`Any`、`Todo(...)` 占位，亦未发现基于 `SingleCase` / `CanonicalFull` 派生第二套 `Step` 类型。
  - 相关验证重新通过：`cargo test -p scoopc --no-default-features refactor_late_lowered_ir`、`cargo test -p scoopc --no-default-features refactor_body_version_key`、`cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`、`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。
  - 2026-05-02：按 detailed TODO 完成判定规则补齐本任务标题的 `[DONE]` 标记，并同步更新 `TODO.md` 索引；`PLAN.md` 无需改动。

## [DONE] P5-T03：依据 `MaterializedEffectFacts` 实现 boundary 选择与 whole-function segmentation，产出 owner-state / resume-state 骨架

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P5
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.16, §5.4.7, §5.5.2-§5.5.4, §8
  - 当前实现参考：
    - `crates/scoopc/src/effect_facts/facts.rs`
    - `crates/scoopc/src/mir/mod.rs`
    - `crates/scoopc/src/mir/pass_view.rs`
    - `crates/scoopc/src/effect/state_machine/segments.rs`（只能作 legacy 参考，不得直接当主线实现）
- 目标：
  - 用 P4 的 callable/block/site facts 决定每个 callable version 的真正 boundary 集；
  - 以 whole-function CFG segmentation 算法，把整个 direct-style body 重写成“可编号状态 + 显式边”的骨架；
  - 保证 boundary 不论出现在独立语句、条件、循环还是更大表达式求值上下文中，都能被统一切分成 owner-state + resume-state。

- 必须实现的内容：
  1. 建立 boundary 选择逻辑，且其 authoritative 输入只能是 P4 facts。
     - boundary 集至少包括：
       - `Perform` site；
       - `Call`/`invoke` site 中 `resolved_cases` 非空者；
       - `Resume` site；
       - ordinary runtime error outward boundary；
       - `Handle` site 中被 P4 标记为 `MaySuspendOutward` 的 nested handle boundary。
     - 明确排除：
       - `resolved_cases = ∅` 的普通 call/invoke；
       - 被 P4 分类为 `SelfContained` 的 nested handle。
  2. 为 boundary 建立稳定映射。
     - 至少要显式记录：
       - `SiteId -> BoundaryId`
       - `BoundaryId -> boundary kind`
       - `BoundaryId -> owner_state`
       - `BoundaryId -> resume_state`
     - 该映射必须进入 late-lowered representation，而不是只存在 builder 临时局部变量中。
  3. 落地统一的 whole-function segmentation 算法。
     - 过程必须符合 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.5.3：
       - 先定位 boundary；
       - 从 boundary 所在 region 切开；
       - 若 boundary 位于条件分支、循环、局部 block、表达式求值上下文或 nested region 内部，则递归向外扩展，直到整个函数都被显式 state 化；
       - 每个 boundary 都必须有唯一 owner state；
       - 每个 boundary 之后的继续执行位置都必须有唯一 resume state。
  4. 明确 state graph 中“普通 segment”与“boundary segment”的关系。
     - boundary 之间的 straight-line 代码形成普通 state segment；
     - 条件分支、循环回边、局部返回、nested region 不再依赖源码结构维持，而要表现为显式 state edge / branch / dispatch；
     - 不能让 P6 再根据源码 shape 还原控制结构。
  5. 正确处理 boundary 位于更大表达式内部的场景。
     - 本任务必须支持至少以下形状：
       - boundary 位于 call 实参求值内部；
       - boundary 位于 `if` / `when` 条件或分支表达式内部；
       - boundary 位于局部初始化或更大表达式中间子表达式内部。
     - 这些场景必须依赖 P3 已显式化的 temporaries / blocks / join points 进行切分；
     - 明确禁止：因为 boundary 不在独立 statement 上，就把样本排除出 P5 目标范围。
  6. `NoOutward` callable 也必须进入同一 segmentation 框架。
     - 它可以最终退化为“单 entry + 单 complete path”的极简 state graph；
     - 但不能因为没有 outward case 就完全绕开 P5 主 transformation。
  7. 若 legacy `effect/state_machine/segments.rs` 中有可复用算法，必须先满足以下条件后才能下沉复用：
     - 不依赖 legacy HIR/LLVM bridge；
     - 不预设“只处理单 handle 内部局部状态机”；
     - 不按 code shape 分叉；
     - 能直接消费 P4 facts + P3 MIR。
     - 若任一条件不满足，则必须在 refactor 新路径中重建算法。

- 必须遵从的约束：
  - 禁止根据源码 `Span` / 表达式原始 AST 形状重新识别 boundary；必须只看 P3/P4 显式化结果。
  - 禁止把 `SelfContained` nested handle 也无差别地向外层扩散成 boundary。
  - 禁止只支持“boundary 恰好在独立 statement 上”的简单情形。
  - 禁止为 `single perform` / 线性函数 / 无循环函数保留另一条更短的 segmentation 入口。

- 验证：
  1. 新增/更新单元测试，推荐命名：
     - `refactor_late_boundary_selection_*`
     - `refactor_late_segmentation_*`
     - `refactor_owner_resume_state_*`
  2. 测试至少覆盖：
     - `perform` / `call` / `resume` / runtime error / outward nested-handle 五类 boundary；
     - `SelfContained` nested handle 不向外层切分；
     - boundary 位于 `if` / loop / nested expr / argument evaluation 时，仍能得到唯一 owner/resume state；
     - `NoOutward` callable 仍走同一入口并退化成极简 state graph。
  3. 运行：
      - `cargo test -p scoopc --no-default-features refactor_late_boundary_selection`
      - `cargo test -p scoopc --no-default-features refactor_late_segmentation`
      - `cargo test -p scoopc --no-default-features refactor_owner_resume_state`
  4. 单元测试样本源码可直接复用或读取以下 P3/P4 样本：
     - `tests/fixtures/mir_refactor/dispatch_and_resume_call.scoop`
     - `tests/fixtures/mir_refactor/handle_finally_boundary.scoop`
     - `tests/fixtures/mir_refactor/effect_boundary_inside_expr_context.scoop`
     - `tests/fixtures/effect_facts/nested_handle_self_contained_vs_outward.scoop`

- 完成条件：
  - boundary 集已完全由 facts 驱动；
  - whole-function segmentation 已能为每个 boundary 生成稳定的 owner-state / resume-state 骨架；
  - 后续 T04/T05 不再需要重新识别 boundary 或重新切 CFG。
- 依赖：P5-T02R
- 完成记录：
  - 2026-05-02：完成 `P5-T03`。新增 `crates/scoopc/src/effect_lowered/segment.rs`，把 P5-T02 预留的空 `state_graph` / `boundary_map` / `resume_state_map` 骨架替换为真实的 whole-function segmentation 结果；当前仓库中的实际模块映射为：`effect_lowered/segment.rs` 对应本 TODO 推荐的 `segment.rs`，`effect_lowered/builder.rs` 继续承接 `materialize.rs` 职责，`frame.rs` / `opt.rs` 仍按顺序留待后续任务落地。
  - boundary 选择现已严格由 P4 `BodyEffectFacts` + canonical MIR 决定：`resolved_cases` 非空的 `Call`/`invoke` site 会发布 `Call` boundary，`Perform` site 会发布 `Perform` boundary，`Resume` site 会同时发布 `Resume` boundary 与其 ordinary `RuntimeError` boundary，只有落在其它 handle region 内且被 P4 标记为 `MaySuspendOutward` 的 nested `Handle` site 才会发布 `Handle` boundary；`SelfContained` nested handle 明确不会向外层切分。
  - `LateLoweredStateGraph` 现已不再是无内容的 `minimal_shell()`：`LateLoweredState` 新增 `LateLoweredStateSlice` 与 `successors`，能够稳定记录“当前 state 覆盖哪个 basic block / statement slice / terminator 片段，以及它在 direct-style CFG skeleton 上的后继 state”。这让 statement-level call/resume boundary 能在同一个 basic block 内切出独立的 owner-state / terminator-only resume-state，同时也让 argument evaluation、`if` 条件、loop condition 与 nested handle/body/finally 里的 boundary 都沿着 P3 已显式化的 temporaries/blocks 进入同一套 segmentation 框架。
  - owner/resume mapping 已作为 authoritative late-lowered contract 固化到 `LateLoweredBoundaryMap` / `LateLoweredResumeStateMap`：每个 boundary 都有稳定 `BoundaryId`、source kind、owner-state 与 resume-state；`NoOutward` callable 也仍经由同一 segmentation builder，只是自然退化为“entry state + complete state + 无 boundary”的极简 skeleton，而不是绕开 P5 transformation。
  - `crates/scoopc/src/effect_lowered/builder.rs` 现已按 callable body 是否存在选择真实 segmentation 或 declaration-only minimal shell；同时修复了一个直接阻塞本任务的 stage 契约问题：late-lowering 不再强制要求 canonical pass-view family 数量与 `callable_facts` 数量完全一致，而是跳过仅声明无 body/无 callable facts 的 family，并继续对“有 canonical body 却缺 facts”的情况报错。这使 `dispatch_and_resume_call` 一类含 declaration-only family 的 canonical snapshot 也能进入 P5 segmentation。
  - 新增/更新测试：`refactor_late_boundary_selection_marks_call_resume_runtime_error_perform_and_outward_handle_boundaries`、`refactor_late_boundary_selection_skips_self_contained_nested_handle_boundaries`、`refactor_late_segmentation_splits_statement_boundaries_into_suffix_resume_states`、`refactor_late_segmentation_keeps_expression_argument_and_if_context_boundaries_distinct`、`refactor_owner_resume_state_tracks_loop_condition_boundaries`、`refactor_owner_resume_state_keeps_no_outward_callables_in_same_framework`、`refactor_owner_resume_state_builder_consumes_only_p4_facts_and_mir_shape`，并同步更新了 `effect_lowering_stage` 现有测试对 declaration-only family 的计数断言。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_late_boundary_selection`、`cargo test -p scoopc --no-default-features refactor_late_segmentation`、`cargo test -p scoopc --no-default-features refactor_owner_resume_state`、`cargo test -p scoopc --no-default-features refactor_late_lowered_ir`、`cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`、`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。
  - 2026-05-02：按 detailed TODO 完成判定规则补齐本任务标题的 `[DONE]` 标记，并同步更新 `TODO.md` 索引；`PLAN.md` 无需改动。

## [DONE] P5-T03R：Review segmentation 骨架，确认 boundary 识别与 owner/resume 状态只由 facts 驱动

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.16, §5.5.2-§5.5.4
  - [`PLAN.md`](./PLAN.md) §2/P5
- 重点：
  - boundary 集是否只由 `CallableEffectFacts` / `SiteEffectFacts` 决定；
  - owner-state / resume-state 是否已经成为显式 mapping；
  - segmentation 是否覆盖 boundary-in-expression，而不是只覆盖独立 statement。
- 必须检查的文件/位置：
  - `crates/scoopc/src/effect_lowered/segment.rs`
  - `crates/scoopc/src/effect_lowered/builder.rs`
  - `crates/scoopc/src/effect_facts/facts.rs`
  - 若有引用 legacy helper，则检查对应共享抽取位置

- 验证：
  - 重新运行 P5-T03 的全部测试与命令；
  - 额外搜索：
    - `rg "Span|hir::|single perform|tail-resume|linear body|statement-only" crates/scoopc/src/effect_lowered crates/scoopc/src/effect_refactor_pipeline`
  - 要求：
    - 允许命中：注释、测试字符串；
    - 不允许命中：P5 主实现仍以这些条件作为事实来源或分流入口。

- 完成条件：
  - review 能明确说明：segmentation 骨架已独立于源码 shape 与 legacy 旁路；
  - 可进入 P5-T04。
- 依赖：P5-T03
- 完成记录：
  - 2026-05-02：完成 `P5-T03R` review。复查 `crates/scoopc/src/effect_lowered/segment.rs` / `builder.rs` / `effect_facts/facts.rs` 后确认，P5 segmentation 的 boundary 选择直接读取 `BodyEffectFacts::site(...)` 与 nested-handle classification，并把 `BoundaryId -> owner_state / resume_state` 显式固化到 `LateLoweredBoundaryMap` / `LateLoweredResumeStateMap`；切分骨架使用 canonical MIR `BasicBlockId + statement_index` cursor 递进，不依赖源码 AST 形状、`Span`、名字或 HIR fallback。
  - 额外搜索 `rg -n "Span|hir::|single perform|tail-resume|linear body|statement-only" crates/scoopc/src/effect_lowered crates/scoopc/src/effect_refactor_pipeline` 后确认：`effect_lowered` 主实现未命中这些 source-shape/legacy 回退条件；仅 `effect_lowered/ir.rs` 测试代码使用 `Span` 构造样本；`effect_refactor_pipeline` 的命中位于前序 HIR stage/dispatcher，不构成 P5 segmentation 的事实来源或分流入口。
  - 重新验证通过：`cargo test -p scoopc --no-default-features refactor_late_boundary_selection`、`cargo test -p scoopc --no-default-features refactor_late_segmentation`、`cargo test -p scoopc --no-default-features refactor_owner_resume_state`、`cargo test -p scoopc --no-default-features refactor_late_lowered_ir`、`cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`、`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。
  - review 未发现需要在 `P5-T03R` 前插入的新前置任务；可进入 `P5-T04`。

## [DONE] P5-T04：实现 frame lifting，以及 `return` / `break` / `continue` / `finally` / cleanup / dropped continuation 的显式状态机合同

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P5
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.7, §5.3.9, §5.5.5-§5.5.6
  - 当前实现参考：
    - `crates/scoopc/src/mir/mod.rs`
    - `crates/scoopc/src/mir/escape.rs`
    - `crates/scoopc/src/effect/state_machine/analysis.rs`（只能作 legacy 参考）
- 目标：
  - 在 segmentation 骨架之上完成统一的 frame lifting；
  - 把跨 boundary 存活的值全部纳入 frame/object fields；
  - 同时把 `return` / `break` / `continue` / `finally` / cleanup / handler arm 续点 / dropped continuation 这些控制转移显式编码进 late-lowered state graph。

- 必须实现的内容：
  1. 落地跨 cut 存活值分析。
     - 判据必须严格遵守 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.5.5：
       - 只要某个值跨越某个 boundary；
       - 且在之后的 resume/re-entry/cleanup 路径上仍会被读取；
       - 就必须进入 frame。
     - 该分析不得只看源码具名 local；必须同时覆盖：
       - 编译器临时值；
       - 中间表达式结果；
       - CFG 合流后的 join/phi-like 值；
       - `handle` arm binder；
       - resume payload / replayed answer/result slot。
  2. 为跨 cut 值分配 frame slots，并把它们接入 state graph。
     - 必须稳定记录：
       - 原值来源（local / temp / join / binder / resume payload / result slot / system field）
       - 对应 `FrameSlotId`
       - 保存点与恢复/读取点
     - late-lowered representation 必须能显式查询“某个源值最终提升到哪个 frame slot”。
  3. 显式加入系统字段。
     - 至少包括：
       - state tag
       - resume payload carrier
       - cleanup flag
       - one-shot flag
       - completion tag
     - 若实现还需要其它系统槽位（例如 boundary scratch / pending answer carrier），可以新增；
     - 但必须稳定命名、稳定归类，并在 dump 中可见。
  4. `return` / `break` / `continue` 必须进入显式 state edge 或 completion path。
     - `return` 不能再是“emit 时自然终止”；
     - loop 的 `break` / `continue` 不能依赖 direct-style CFG 原样保留；
     - 它们必须成为 late-lowered state graph 中的显式跳转目标。
  5. `finally` / cleanup / handler arm 续点必须进入显式状态机合同。
     - `handle` body、arm、finally 各自结束后跳向哪里，必须在 state graph 中可追踪；
     - cleanup path 何时运行、运行后回到哪里，也必须是显式 state edge；
     - 明确禁止：留到 P6/LLVM emit 时再凭 direct-style MIR 或源码结构猜。
  6. dropped continuation 语义必须按 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.7 物化。
     - drop 后表示“剩余语言级计算被放弃”；
     - 任何尚未执行到的 pending `finally` / cleanup 都**不再执行**；
     - 若 stage output 中需要显式表示该结果，推荐加入稳定的 `Abandoned` / `Dropped` completion path 或等价表示；
     - 但不能让它继续流向 ordinary cleanup path，更不能与 GC cleanup hook 混为一谈。
  7. ordinary runtime error outward 必须按普通 effect 分支路径接入 state graph。
     - 不能作为“异常捷径”直接绕过 `Step_F` / boundary contract；
     - 对 `resume` 重入错误等场景，必须能在 late-lowered graph 中追踪到其普通 outward case 路径。
  8. `NoOutward` callable 的 frame 处理要保持退化但正确。
     - 若没有任何跨 cut 存活值，则允许 frame 为空或退化到最小系统形态；
     - 禁止为了统一而强行给无 boundary 的 callable 分配大量无用 frame slots。

- 必须遵从的约束：
  - 禁止只 lift 源码 local，而忽略编译器临时值、join 值或 binder/result slot。
  - 禁止把 dropped continuation 继续接到 pending `finally` / cleanup 执行路径上。
  - 禁止把 runtime error 重新降级为隐藏 trap channel。
  - 禁止让 `return` / `break` / `continue` / `finally` / cleanup 仍依赖 P3 direct-style CFG 的隐式含义，而在 P5 中不可见。

- 验证：
  1. 新增/更新单元测试，推荐命名：
     - `refactor_frame_lifting_*`
     - `refactor_late_control_flow_*`
     - `refactor_dropped_continuation_*`
     - `refactor_runtime_error_boundary_*`
  2. 测试至少覆盖：
     - locals / temporaries / join values / binders / resume slots / system fields 都能被正确 lift；
     - `return` / `break` / `continue` 进入显式 state edge；
     - `finally` / cleanup / arm 续点被显式编码；
     - dropped continuation 不执行剩余 `finally` / cleanup；
     - runtime error outward 仍走普通 effect 分支。
  3. 运行：
      - `cargo test -p scoopc --no-default-features refactor_frame_lifting`
      - `cargo test -p scoopc --no-default-features refactor_late_control_flow`
      - `cargo test -p scoopc --no-default-features refactor_dropped_continuation`
      - `cargo test -p scoopc --no-default-features refactor_runtime_error_boundary`
  4. 测试样本源码推荐至少覆盖：
     - `tests/fixtures/mir/while_break_continue.scoop`
     - `tests/fixtures/mir_refactor/handle_finally_boundary.scoop`
     - `tests/fixtures/mir_refactor/effect_boundary_inside_expr_context.scoop`
     - 新增 `tests/fixtures/effect_lowered_src/dropped_continuation_abandons_remaining_work.scoop`
     - 新增 `tests/fixtures/effect_lowered_src/continuation_resume_runtime_error_boundary.scoop`

- 完成条件：
  - frame lifting 已覆盖跨 cut 的全部值种类；
  - late-lowered state graph 已显式编码 `return` / `break` / `continue` / `finally` / cleanup / dropped continuation / runtime error 路径；
  - 后续 T05 只需在这套显式图上物化 `Step` / continuation / invoke contract。
- 依赖：P5-T03R
- 完成记录：
  - 2026-05-03：完成 `P5-T04`。`crates/scoopc/src/effect_lowered/frame.rs` 已新增独立 frame-lifting pass，并由 `builder.rs` 在 segmentation 之后、continuation shell 物化之前统一调用；它只消费 canonical MIR snapshot、P4 effect facts、`StepSchema`/`ContinuationSchema` 与当前 late-lowered state skeleton，不回 HIR/typecheck，也不借壳 legacy `effect/state_machine/**`。
  - `crates/scoopc/src/effect_lowered/ir.rs` 现已把 P5-T04 需要的显式合同固化到正式 IR：`LateLoweredState` 新增 `LateLoweredStateTerminator`，显式区分 `Suspend` / `Goto` / `Branch` / `Return` / `HandleDispatch` / `ResumeUnwind` / `Abandon`；`LateLoweredFrameSlot` 新增 `write_points` / `read_points`；`LateLoweredFrameSlotKind` 扩展为 `SourceLocal`、`CompilerTemporary`、`JoinValue`、`HandleBinder`、`ResumePayload`、`BoundaryResult` 与系统槽位，满足“frame schema 可查询某个 lifted value 落到哪个 slot，以及在哪些 state 写入/读取”的要求。
  - `crates/scoopc/src/effect_lowered/segment.rs` 已不再只发布裸 successor skeleton：statement/call/resume/perform boundary 会物化成显式 `Suspend` terminator；loop/branch/return 会保留 `Goto` / `Branch` / `Return`；`Handle` terminator 会发布 `HandleDispatch`，并显式携带 body/arm/finally/exit target；带 cleanup 的 suspend path 会把 cleanup edge 固定到 state graph 中，而不是留给 P6/LLVM emit 再猜。
  - frame lifting 现已覆盖本任务要求的值种类。具体为：
    - 源码 local：按 boundary owner state 的 `live_out` 求解进入 `SourceLocal` slot；
    - 编译器临时值：基于 direct-style MIR `tmp*` canonical temp local 进入 `CompilerTemporary` slot；
    - CFG 合流值：对多定义且在 merge state 后继续跨后续 boundary 读取的 local 发布 `JoinValue` slot；
    - handler arm binder：依据 MIR `Handle` arm block input local 显式发布 `HandleBinder` slot；
    - resume payload：按当前 callable version 的 reachable outward case 集，为每个 `BoundaryId + CaseTag` 发布 `ResumePayload` slot；
    - replayed answer/result：对 call/resume/perform boundary 的 result local 发布 `BoundaryResult` slot；
    - 系统字段：统一发布 `StateTag`、`ResumePayloadCarrier`、`CleanupFlag`、`OneShotFlag`、`CompletionTag`。
  - dropped continuation 合同已显式落地：只要 callable 存在 outward boundary，late-lowered graph 就会新增独立 `Drop` state，并把所有 `Suspend` / outward `HandleDispatch` terminator 的 `drop_state` 指向该 `Abandon` path；含 pending cleanup 的 suspend state 同时保留独立 `cleanup_state`，二者不再混淆，因此 dropped continuation 不会再落入 pending `finally` / cleanup path。
  - runtime error outward 合同已显式落地：resume site 的 ordinary runtime error boundary 仍继续发布在 `boundary_map` 中，并与对应 `Resume` boundary 共用同一个 `Suspend` terminator / owner state / resume state，而不是被降级成 hidden trap channel。
  - `crates/scoopc/src/effect_lowered/dump.rs` 已扩展稳定 formatter，显式输出 state terminator、drop/cleanup target、frame slot 写入点/读取点与新增 slot kinds，便于后续 `dump-effect-lowered` / snapshot / review 直接锁定这些 contract。
  - 新增/更新测试：
    - `crates/scoopc/src/effect_lowered/frame.rs`：`refactor_frame_lifting_lifts_locals_temporaries_resume_slots_and_system_fields`、`refactor_frame_lifting_marks_handle_binders_that_cross_nested_boundaries`、`refactor_frame_lifting_marks_phi_like_join_values_that_cross_later_boundaries`；
    - `crates/scoopc/src/effect_lowered/segment.rs`：`refactor_late_control_flow_encodes_loop_break_continue_as_explicit_state_edges`、`refactor_late_control_flow_keeps_handle_body_arm_finally_and_cleanup_edges_explicit`、`refactor_dropped_continuation_uses_dedicated_drop_state_instead_of_cleanup`、`refactor_runtime_error_boundary_stays_inside_explicit_suspend_contract`；
    - 新增 fixture：`tests/fixtures/effect_lowered_src/dropped_continuation_abandons_remaining_work.scoop`、`tests/fixtures/effect_lowered_src/continuation_resume_runtime_error_boundary.scoop`。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`、`cargo test -p scoopc --no-default-features refactor_late_boundary_selection`、`cargo test -p scoopc --no-default-features refactor_owner_resume_state`、`cargo test -p scoopc --no-default-features refactor_late_lowered_ir`、`cargo test -p scoopc --no-default-features refactor_frame_lifting`、`cargo test -p scoopc --no-default-features refactor_late_control_flow`、`cargo test -p scoopc --no-default-features refactor_dropped_continuation`、`cargo test -p scoopc --no-default-features refactor_runtime_error_boundary`、`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。

## [DONE] P5-T04a：为 frame lifting 建立稳定的 MIR local 来源分类，避免把源码 `tmp*` local 误判为 compiler temporary

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P5
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.5.5
  - `crates/scoopc/src/mir/mod.rs`
  - `crates/scoopc/src/mir/lower.rs`
  - `crates/scoopc/src/effect_lowered/frame.rs`
- 背景：
  - `P5-T04R` 审阅发现：`crates/scoopc/src/effect_lowered/frame.rs` 目前通过 `LocalDecl.name.starts_with("tmp")` 把 lifted local 分成 `SourceLocal` 与 `CompilerTemporary`；
  - 但 `crates/scoopc/src/mir/lower.rs` 中源码具名 local 与编译器 temp 共用同一个 `LocalDecl.name` 字段，源码里合法命名为 `tmp` / `tmp0` / `tmp_value` 的 local 会被错误归类为 compiler temporary；
  - 这违反了 P5-T04 对“frame slot 来源稳定、可查询、不可依赖名字猜测”的要求，也会让后续 dump/review 对 slot 来源的判断失真。
- 目标：
  - 为 MIR local 建立稳定的“源码 local / 编译器 temporary”来源标记；
  - 让 frame lifting 仅消费该稳定来源信息，不再依赖 local 名字前缀猜测。

- 必须实现的内容：
  1. 为 MIR `LocalDecl` 或等价稳定元数据补充 local 来源分类。
     - 至少要能区分：源码具名 local、编译器 temporary；
     - 该信息必须随 canonical MIR body 一起存在，不能只在 lowering 临时上下文里可见。
  2. 让 `push_named_local` / 参数 local / `val` / `var` / binder / 其它源码来源 local 保留为源码 local。
  3. 让 `push_temp_local` 及其它编译器引入的 expression temp 明确标记为 compiler temporary。
  4. 更新 `crates/scoopc/src/effect_lowered/frame.rs` 的 slot 分类逻辑。
     - 明确禁止继续使用 `name.starts_with("tmp")` 或其它字符串启发式；
     - 源码 local 即使名字恰好是 `tmp` / `tmp0` / `tmp_value`，也必须保持 `SourceLocal`；
     - 编译器 temp 仍必须稳定归类到 `CompilerTemporary`。
  5. 补充回归测试。
     - 至少新增一个 effectful 样例，使源码 local 名字以 `tmp` 开头且跨 boundary 存活；
     - 断言该 local 进入 frame 后仍被标记为 `SourceLocal`，而真正的编译器临时值仍被标记为 `CompilerTemporary`。

- 验证：
  - `cargo test -p scoopc --no-default-features refactor_frame_lifting`
  - `cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`
  - `cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`

- 完成条件：
  - frame slot 来源分类不再依赖 local 名字拼写；
  - 源码 `tmp*` local 不会再被误判为 `CompilerTemporary`；
  - `P5-T04R` 可继续审阅 frame lifting/control-flow contract，而不再被来源分类噪音阻塞。
- 依赖：P5-T04
- 完成记录：
  - 2026-05-03：完成 `P5-T04a`。`crates/scoopc/src/mir/mod.rs` 已为 canonical MIR `LocalDecl` 新增稳定来源枚举 `LocalSourceKind::{SourceLocal, CompilerTemporary}`，使“源码 local / 编译器 temporary”分类成为 body 常驻元数据，而不是 lowering 临时上下文里的隐式约定。
  - `crates/scoopc/src/mir/lower.rs` 现已把来源写死到 local 分配入口：`push_named_local` 统一发布 `SourceLocal`，`push_temp_local` 统一发布 `CompilerTemporary`；参数 local、`val`/`var`、binder、closure capture/env 等源码来源 local 保持为 `SourceLocal`，expression temp 不再借由名字前缀模拟来源。
  - `crates/scoopc/src/effect_lowered/frame.rs` 已删除 `LocalDecl.name.starts_with("tmp")` 启发式，frame slot 分类改为只消费 MIR `LocalSourceKind`；因此源码名恰好为 `tmp` / `tmp0` / `tmp_value` 的 local 仍会进入 `LateLoweredFrameSlotKind::SourceLocal`，真正的 MIR temp 才会进入 `CompilerTemporary`。
  - 为保持测试与辅助构造路径一致，手工构造 `LocalDecl` 的位置已补齐来源字段，包括 `crates/scoopc/src/mir/{materialize,inline,escape,mod}.rs` 中的测试/辅助代码，避免新元数据只在 lowering 主路径可见而在其它 canonical MIR 构造入口缺失。
  - 新增回归测试 `crates/scoopc/src/effect_lowered/frame.rs::refactor_frame_lifting_uses_stable_mir_local_source_metadata`：样例中源码 local 名 `tmp_seed` 跨 boundary 存活，同时存在真正跨 boundary 的 compiler temporary；测试断言 canonical MIR 把 `tmp_seed` 标成 `SourceLocal`，frame schema 继续把它发布为 `SourceLocal(localX)`，并且至少保留一个真正的 `CompilerTemporary(localY)` slot。
  - 2026-05-03：按详细 TODO 完成判定规则补齐本任务标题的 `[DONE]` 标记，并同步更新 `TODO.md` 索引；`PLAN.md` 无需改动。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_frame_lifting`、`cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`、`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。

## [DONE] P5-T04R：Review frame lifting 与控制流合同，确认没有残留 direct-style 隐式语义或错误的 dropped-continuation 行为

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.7, §5.3.9, §5.5.5-§5.5.6
  - [`PLAN.md`](./PLAN.md) §2/P5
- 重点：
  - frame lifting 是否已覆盖 locals 之外的 temporaries / join values / binders / result slots；
  - `return` / `break` / `continue` / `finally` / cleanup 是否已成为显式状态机合同；
  - dropped continuation 是否确实放弃剩余计算，而不是落入 cleanup 路径。
- 必须检查的文件/位置：
  - `crates/scoopc/src/effect_lowered/frame.rs`
  - `crates/scoopc/src/effect_lowered/builder.rs`
  - 与 dropped continuation / completion path 相关的实现位置

- 验证：
  - 重新运行 P5-T04 的全部测试与命令；
  - 额外搜索：
    - `rg "Todo\(|pending finally|pending cleanup|handler stack|cleanup hook" crates/scoopc/src/effect_lowered crates/scoopc/src/effect_refactor_pipeline`
  - 要求：
    - 允许命中：注释、测试、legacy 模块；
    - 不允许命中：P5 新主线仍以这些 pending/TLS/cleanup-hook 语义作为正确性前提。

- 完成条件：
  - review 能明确说明：frame lifting 与显式控制流合同已经闭合；
  - 可进入 P5-T05。
- 依赖：P5-T04a
- 完成记录：
  - 2026-05-03：审阅发现 blocker。`crates/scoopc/src/effect_lowered/frame.rs` 当前使用 `LocalDecl.name.starts_with("tmp")` 区分 `SourceLocal` / `CompilerTemporary`，但 `crates/scoopc/src/mir/lower.rs` 会把源码具名 local 也原样写入同一个 `name` 字段，因此合法源码名 `tmp` / `tmp0` / `tmp_value` 会被误判为 compiler temporary。
  - 这会破坏 P5-T04 要求的 stable frame-slot 来源分类，并让后续 dump/review 对 lifted value 来源的判断失真；因此新增前置任务 `P5-T04a`，待修复后再继续完成本 review。
  - 2026-05-03：完成 `P5-T04R` review。基于 `P5-T04a` 已提供的稳定 `LocalSourceKind` 元数据，复核 `crates/scoopc/src/effect_lowered/{frame,segment,builder,ir,dump}.rs` 后，确认 frame lifting 已稳定覆盖 `SourceLocal` / `CompilerTemporary` / `JoinValue` / `HandleBinder` / `BoundaryResult` / `ResumePayload` / 系统槽位，且分类来源只消费 canonical MIR 与 P4 facts，不再依赖 `tmp*` 名字启发式、HIR/typecheck fallback 或 legacy `effect/state_machine/**`。
  - 显式控制流合同复核通过：`LateLoweredStateTerminator` 现已稳定发布 `Suspend` / `Goto` / `Branch` / `Return` / `HandleDispatch` / `ResumeUnwind` / `Abandon`；`segment.rs` 会把 loop `break`/`continue`、handle body/arm/finally、cleanup edge、resume runtime error outward 与 dedicated drop path 一并写入 late-lowered state graph，因此 P5 新主线不再依赖 direct-style CFG 的隐式语义来推断这些跳转。
  - 文本搜索复核：按本任务要求在 `crates/scoopc/src/effect_lowered` 与 `crates/scoopc/src/effect_refactor_pipeline` 搜索 `Todo\(|pending finally|pending cleanup|handler stack|cleanup hook`，命中仅剩 generic MIR `Todo(_)` 分支、测试断言字符串，或 `effect_lowering_stage.rs` 中确认“未导入 legacy state_machine/llvm”的 review 测试；未发现 P5 新主线把 pending-finally / pending-cleanup / handler-stack / cleanup-hook 语义当作 correctness 前提。
  - 2026-05-03：按 detailed TODO 完成判定规则补齐本任务标题的 `[DONE]` 标记，并同步更新 `TODO.md` 索引；`PLAN.md` 无需改动。
- 验证通过：`cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`、`cargo test -p scoopc --no-default-features refactor_late_boundary_selection`、`cargo test -p scoopc --no-default-features refactor_owner_resume_state`、`cargo test -p scoopc --no-default-features refactor_late_lowered_ir`、`cargo test -p scoopc --no-default-features refactor_frame_lifting`、`cargo test -p scoopc --no-default-features refactor_late_control_flow`、`cargo test -p scoopc --no-default-features refactor_dropped_continuation`、`cargo test -p scoopc --no-default-features refactor_runtime_error_boundary`、`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。

## [DONE] P5-T04b：对齐 late lowering 对 `ContinuationSchema.surface_ty` / `out_step_schema` 的消费边界，避免在 continuation 物化时重新引入 surface-row 漂移

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.1, §5.3.9, §5.4.2, §5.5.4
  - [`PLAN.md`](./PLAN.md) §2/P4, §2/P5
  - [`TODO-P4.md`](./TODO-P4.md) 中 `P4-T05b`
  - 当前实现参考：
    - `crates/scoopc/src/effect_lowered/builder.rs`
    - `crates/scoopc/src/effect_lowered/ir.rs`
    - `crates/scoopc/src/effect_lowered/dump.rs`
- 背景：
  - `P5-T05` 将首次真正物化 continuation object、resume interface 与 one-shot runtime-error lowering；
  - 若 P5 把 `ContinuationSchema.surface_ty` 和 `out_step_schema` 继续当作同一层 contract，就会在 continuation object / dump / ABI 组织中重新引入“把 one-shot runtime-error upper bound 并回 `Continuation<..., eff Out>`”的错误；
  - 因此在继续推进 `P5-T05` 之前，必须先把 P5 对这两层 contract 的消费边界显式锁定。
- 目标：
  - 确保 P5 只用 `resume_tuple_ty` / `answer_ty` / `out_step_schema` 驱动 internal `Step` / continuation lowering；
  - 同时把 `surface_ty` 保持为源码层 `Continuation<..., eff Out>` 的可见 contract，而不是 internal step upper bound 的镜像。

- 必须实现的内容：
  1. 逐点明确 P5 消费 `ContinuationSchema` 的职责分工。
     - internal resume interface、continuation object、boundary lowering、one-shot runtime-error outward 路径，必须以 `resume_tuple_ty`、`answer_ty`、`out_step_schema` 为 authoritative 输入；
     - `surface_ty` 只用于保留/显示源码层 continuation contract，不能作为 internal runtime-error case 集或 resume ABI 的主来源。
  2. 对齐 late-lowered builder / IR / dump 的 contract 假设。
     - 若当前 P5 shell/dump 仍把 `surface_ty` 视作与 `out_step_schema` 等价，必须在本任务中修正；
     - 若某些字段/注释会误导后续实现把 `surface_ty.eff` 当成 one-shot runtime-error 判据，也必须一并修正。
  3. 固定 one-shot lowering 的判断规则。
     - 即使 `surface_ty.eff` 不含 `Raise<RuntimeError>`，只要 `out_step_schema` / `StepSchema` 含 compiler-generated ordinary runtime-error case，continuation object 的重复恢复路径仍必须走普通 outward `Step_F` case；
     - 反过来，也禁止仅因 `surface_ty` 的 effect 参数含 `Raise<RuntimeError>` 就跳过 `out_step_schema` / site facts 给出的 canonical boundary contract。
  4. 更新定向测试与后续任务前提。
     - 至少要为 `P5-T05` 准备一个样本：`surface_ty` 保持 `Pure` 或 `Boom`，但 `out_step_schema`/`StepSchema` 额外含 one-shot runtime-error case；
     - `P5-T05` / `P5-T05R` 的任务描述、依赖或 review 重点若默认把 `surface_ty` 与 internal upper bound 视为同一层，必须在本任务中修正。

- 必须遵从的约束：
  - 禁止把 `surface_ty.eff` 当作 one-shot runtime-error lowering 的唯一判据；
  - 禁止因为 `surface_ty` 未显式带 `Raise<RuntimeError>` 就回退到 hidden channel / pseudo case；
  - 禁止为了规避问题而把 `surface_ty` 从 P5 IR / dump contract 中删掉。

- 验证：
  1. 新增/更新单元测试，推荐命名：
     - `refactor_resume_interface_uses_out_step_schema_not_surface_ty_*`
     - `refactor_continuation_object_one_shot_runtime_error_preserves_surface_row_*`
     - `refactor_effect_lowered_stage_surface_ty_does_not_control_runtime_error_case_*`
  2. 测试至少覆盖：
     - `surface_ty = Continuation<..., eff Pure>` 或 `eff Boom` 时，只要 `out_step_schema` 带 one-shot runtime-error case，P5 仍能物化对应 outward path；
     - late-lowered dump / shell 中 source-visible continuation type 不会被无端扩大成 `eff (Out + Raise<RuntimeError>)`；
     - source residual row 本来就含 runtime error 的样本不会被误收窄。
  3. 运行：
      - `cargo test -p scoopc --no-default-features refactor_resume_interface`
      - `cargo test -p scoopc --no-default-features refactor_continuation_object`
      - `cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`

- 完成条件：
  - P5 已显式锁定：internal one-shot/runtime-error lowering 看 `out_step_schema` / `StepSchema`，source-visible continuation contract 看 `surface_ty`；
  - `P5-T05` 可以在不重新讨论这两层边界的前提下继续物化 continuation object 与 boundary lowering。
- 依赖：P4-T05b，P5-T04R
- 完成记录：
  - 2026-05-03：完成 `P5-T04b`。`crates/scoopc/src/effect_lowered/ir.rs` 现已新增 `LateLoweredContinuationContract`，把 `ContinuationSchema` 的双层 contract 显式固化到 P5 shell 中：`resume_tuple_ty` / `answer_ty` / `out_step_schema` 作为 internal `resume(...) -> Step_F` lowering 的 authoritative 输入，`surface_ty` 单独保留为 source-visible `Continuation<..., eff Out>` contract，不再让后续实现只能从 `StepSchema` 或 runtime-error upper bound 倒推 surface row。
  - `crates/scoopc/src/effect_lowered/builder.rs` 现已通过 `build_continuation_contract(...)` 统一消费 `ContinuationSchema`：step case、resume interface method、continuation object method 都直接从 authoritative `resume_tuple_ty` / `answer_ty` / `out_step_schema` / `surface_ty` 建壳；同时新增一致性校验，若某个 case 的 `ContinuationSchema.out_step_schema` 与当前 step return contract 不一致，或其 `answer_ty` 与 return-step `complete_ty` 不一致，会立即在 P5 builder 报错，而不是等 `P5-T05` 继续把错误 contract 静默带进 continuation 物化。
  - `crates/scoopc/src/effect_lowered/dump.rs` 已把 `surface_ty` / `out_step_schema` / `answer_ty` 写入 stable dump，因此 late-lowered dump 现在能直接公开“source-visible continuation type”和“internal step upper bound”是两层不同 contract；这避免后续任务在 dump/ABI 组织里重新把 compiler-generated one-shot runtime-error upper bound 并回 `Continuation<..., eff Out>`。
  - 已补齐定向测试样本：
    - `refactor_resume_interface_uses_out_step_schema_not_surface_ty_for_runtime_error_case` 使用 `dispatch_and_resume_call.scoop` 锁定 `surface_ty = Continuation<Int, Unit, eff fixtures.mir.Boom>` 仍可对应含 ordinary runtime-error case 的 `out_step_schema`；
    - `refactor_continuation_object_one_shot_runtime_error_preserves_surface_row_in_shell` 使用 `single_case_impl_plan.scoop` 锁定 continuation object shell 在 callable step schema 含 compiler-generated runtime-error case 时，source-visible surface 仍保持 `eff sample.Ping`；
    - `refactor_effect_lowered_stage_surface_ty_does_not_control_runtime_error_case_contracts` 锁定 source residual row 本就含 `Raise<RuntimeError>` 的样本不会被误收窄，同时 stable dump 会显式暴露 `surface_ty` / `out_step_schema` / `answer_ty` 字段。
  - 已同步修正 `P5-T05R` 的 review 重点：review 除了检查单一 `Step` / continuation ABI 外，还必须确认 `surface_ty` 继续只表达 source-visible continuation contract，而 internal one-shot/runtime-error lowering 仍由 `out_step_schema` 驱动。
  - 2026-05-03：按 detailed TODO 完成判定规则补齐本任务标题的 `[DONE]` 标记，并同步更新 `TODO.md` 索引；`PLAN.md` 无需改动。
  - 验证通过：`cargo fmt --all`、`cargo test -p scoopc --no-default-features refactor_resume_interface`、`cargo test -p scoopc --no-default-features refactor_continuation_object`、`cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`、`cargo test -p scoopc --no-default-features refactor_late_lowered_ir`、`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。

## P5-T05：物化 `Step_F` enum、canonical dynamic `invoke`、continuation object、internal resume interfaces，并按 `ImplPlan` 完成 boundary lowering

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P5
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.9, §5.2, §5.3.1-§5.3.5, §5.4.2, §5.5.4, §7.3, §8
  - 当前实现参考：
    - `crates/scoopc/src/effect_facts/schema.rs`
    - `crates/scoopc/src/effect_facts/facts.rs`
    - `crates/scoopc/src/effect/state_machine/transform.rs`（只能作 legacy 对照）
- 目标：
  - 在 T03/T04 的 state graph 与 frame schema 基础上，真正把 outward `Step`、dynamic invoke surface、continuation object 与 resume interfaces 物化为 late-lowered representation；
  - 对 `NoOutward` / `SingleCase(case_tag)` / `CanonicalFull` 三档 `ImplPlan` 给出清晰、统一、可验证的 lowering 结果；
  - 使 P6 无需再次发明 call/perform/resume/handle 的 effectful ABI。

- 必须实现的内容：
  1. 物化 canonical `Step_F` enum 定义。
     - 每个 `StepSchemaId` 必须对应一个内部 `enum`；
     - 该 `enum` 必须包含：
       - `Complete(complete_ty)` 或零载荷 `Complete` variant；
       - 每个 case 对应的 variant，且保留 canonical `CaseTag` 顺序；
       - variant 的 payload 与 continuation object 类型由 `StepSchema` 决定。
  2. 为每个 effectful callable 物化 canonical dynamic entry：
     - surface 必须固定为 `invoke(args_tuple) -> Step_F`；
     - 当前阶段 direct/static path 允许直接复用该 entry；
     - 不允许在 P5 设计第二个用户可见 “optimized direct ABI”。
  3. 按 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.2-§5.3.5 物化 internal resume interfaces。
     - 需要先为 `ConcreteOpKey` 建立稳定的 effect-family 分组键；
     - 该分组只能基于当前 materialized program snapshot 中已有的 monomorphic callable/effect metadata 与 P4 facts 推出；
     - 明确禁止：回 HIR/typecheck/源码字符串重新猜 effect owner。
     - 每个 interface method 必须满足：
       - 参数类型 = 对应 case 的 `resume_tuple_ty`
       - 返回类型 = 同一 `Step_F<T>`
       - identity 稳定，可被 dump / 后续优化引用。
  4. 物化 continuation object。
     - continuation object 的具体类型对同一个 callable version 固定；
     - 它必须捕获：
       - 恢复所需 frame/context；
       - 必要的 system fields；
       - one-shot 状态；
       - 与当前 case/continuation schema 对应的恢复入口信息。
     - 它的方法体负责把 `k.op$ret(resume_tuple)` 重新送回同一个 `Step_F<T>` 协议。
  5. continuation object 必须完整实现对应的 interface method 集。
     - 即使当前 continuation 实际只会合法响应少数 case，类型上仍必须保留完整 method 集；
     - 对永远不会合法调用到的 methods，允许 body 直接为 `unreachable`；
     - 不允许从 object/interface 定义中删掉这些方法。
  6. 按统一骨架完成 boundary lowering。
     - 所有 boundary 都必须落入统一模型：
       - `state_before -> boundary(site) -> resume_state -> next states`
     - 对不同 boundary 的最低要求：
       - `Perform`：直接构造 outward `Step` case，并生成当前 callable 的 continuation object；
       - effectful `Call` / `invoke`：调用 callee 的 canonical `invoke(args_tuple) -> Step_F`，对返回的 `Step_F` 做显式分派：
         - `Complete(answer)` -> 填入 result slot，跳到本 boundary 的 `resume_state`；
         - outward case -> 按当前 callable 的 `StepSchema` / site facts 构造向外传播的 `Step_F`，并捕获当前 continuation；
       - `Resume`：调用 continuation object 的 resume interface method，返回 `Step_F<Answer>`，再按与 call boundary 相同的显式分派处理；
       - ordinary runtime error outward：构造成普通 outward case；
       - `MaySuspendOutward` nested handle：作为真正 boundary 进入同一 outward `Step` 模型；
       - `SelfContained` nested handle：保留为内部子图，不向外层 step boundary 扩散。
  7. 明确 `ImplPlan` 三档的 lowering 结果。
     - `NoOutward`：
       - outward case 集为空；
       - canonical entry 只能产生 `Complete`；
       - `needs_reentry = false`；
       - 但它仍是统一 late-lowering 框架下的退化 callable version，不是跳过 P5。
     - `SingleCase(case_tag)`：
       - 允许内部省掉多分支 case dispatch；
       - 但 outward 返回类型仍是 canonical `Step_F`；
       - 不能重排 tag，不能生成单独“窄 Step 类型”。
     - `CanonicalFull`：
       - 对 `StepSchema` 全集保持完整 case dispatch。
  8. 把 capture 链吸收到 continuation/state-machine 模型本身。
     - continuation object / state graph 必须显式承载恢复所需 handler/context；
     - 明确禁止：继续依赖 ambient TLS handler stack、snapshot bridge 或 caller 现场补链。
  9. one-shot 语义必须在 continuation object / resume path 中显式表达。
     - 首次恢复后，后续非法再次恢复必须走 ordinary runtime error outward；
     - 不能留给 P6/backend 才“顺便”检查。
   10. `payload_tuple_ty == ()` / `resume_tuple_ty == ()` 的 case 必须在中层正确退化。
       - 允许物理零载荷 variant / method 形态；
       - 但语义上必须仍与 P4 schema 对齐，不能在 P5 私自省略 case/方法身份。
   11. `ContinuationSchema.surface_ty` 与 `out_step_schema` 的 contract 边界必须被保持。
       - internal resume interface / continuation object / one-shot runtime-error lowering 必须由 `resume_tuple_ty` / `answer_ty` / `out_step_schema` 驱动；
       - `surface_ty` 继续只表示源码层 `Continuation<..., eff Out>`，不能因为 internal one-shot runtime-error case 而被无端扩大。

- 必须遵从的约束：
  - 禁止把 call/resume boundary 的 outward 传播写成 ad-hoc helper，而不经统一 `Step_F` 分派模型。
  - 禁止将 `SingleCase` 理解为“换一种 ABI/type/tag”，它只能改变内部可达分支与 dispatch 复杂度。
  - 禁止继续依赖 TLS handler stack / handler snapshot / backend bridge 作为 continuation 语义前提。
  - 禁止把 impossible resume methods 直接从接口中删掉；只能保留并标记为 `unreachable`。
  - 禁止为 dynamic callable / continuation 分别发明两套互不兼容的内部 carrier 协议；两者都应收口为 object + tuple input + `Step` return 的统一内部模型，但语义 surface 不得混淆。

- 验证：
  1. 新增/更新单元测试，推荐命名：
     - `refactor_step_materialization_*`
     - `refactor_boundary_lowering_*`
     - `refactor_continuation_object_*`
     - `refactor_impl_plan_lowering_*`
     - `refactor_resume_interface_completeness_*`
  2. 测试至少覆盖：
     - `Step_F` enum 形状与 `StepSchema` 一一对应；
     - dynamic `invoke(args_tuple) -> Step_F` surface 正确；
     - continuation object 完整实现 interface method 集；
     - impossible methods 以 `unreachable` 表示，而不是被删除；
     - `perform` / `call` / `resume` / runtime error / outward nested-handle 都走统一 boundary lowering；
     - `NoOutward` / `SingleCase` / `CanonicalFull` 三档结果各自正确；
     - one-shot 重复 `resume` 进入 ordinary runtime error outward；
     - `()` payload / `()` resume tuple 的零载荷 case 正确。
  3. 运行：
      - `cargo test -p scoopc --no-default-features refactor_step_materialization`
      - `cargo test -p scoopc --no-default-features refactor_boundary_lowering`
      - `cargo test -p scoopc --no-default-features refactor_continuation_object`
      - `cargo test -p scoopc --no-default-features refactor_impl_plan_lowering`
      - `cargo test -p scoopc --no-default-features refactor_resume_interface_completeness`
  4. 测试样本源码推荐至少覆盖：
     - `tests/fixtures/effect_facts/single_case_impl_plan.scoop`
     - `tests/fixtures/effect_facts/dynamic_fallback_widening.scoop`
     - `tests/fixtures/mir_refactor/dispatch_and_resume_call.scoop`
     - `tests/fixtures/mir_refactor/handle_perform.scoop`
     - `tests/fixtures/effect_lowered_src/continuation_resume_runtime_error_boundary.scoop`

- 完成条件：
  - P5 已真正物化 `Step_F`、dynamic invoke、continuation object、resume interfaces；
  - 所有 boundary 已按统一 `Step`/continuation 模型完成 lowering；
  - P6 不再需要设计新的 effectful ABI 或 continuation carrier。
- 依赖：P5-T04b
- 完成记录：
  - （执行时填写）

## P5-T05R：Review `Step` / continuation 物化结果，确认没有第二套 ABI、没有 TLS 依赖、没有删减接口方法

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.9, §5.2, §5.3.2-§5.3.5, §7.3
  - [`PLAN.md`](./PLAN.md) §2/P5
- 重点：
  - canonical `invoke(args_tuple) -> Step_F` 是否已经成为唯一动态 surface；
  - `SingleCase` 是否仍保持 canonical `Step_F` 类型与 `CaseTag`；
  - continuation object 是否完整实现了对应 interface method 集；
  - `ContinuationSchema.surface_ty` 是否仍只表达 source-visible continuation contract，而 internal one-shot/runtime-error lowering 继续由 `out_step_schema` 驱动；
  - 新主线是否仍摆脱 TLS handler stack / bridge 依赖。
- 必须检查的文件/位置：
  - `crates/scoopc/src/effect_lowered/materialize.rs`
  - `crates/scoopc/src/effect_lowered/ir.rs`
  - 与 resume interface family / continuation object 实现相关的位置

- 验证：
  - 重新运行 P5-T05 的全部测试与命令；
  - 额外搜索：
    - `rg "handler_stack|snapshot|tls|bridge|Signal \{|unreachable" crates/scoopc/src/effect_lowered crates/scoopc/src/effect_refactor_pipeline crates/scoopc/src/llvm/codegen/effect`
  - 要求：
    - `unreachable` 在 continuation impossible methods 中允许出现；
    - `handler_stack|snapshot|tls|bridge|Signal {` 不允许成为 P5 新主线的 correctness 前提。

- 完成条件：
  - review 能明确说明：P5 已收口到统一的 `Step` / continuation ABI 模型；
  - 可进入 P5-T06。
- 依赖：P5-T05
- 完成记录：
  - （执行时填写）

## P5-T06：在 late-lowered representation 上加入窄的 devirtualization / inlining / DCE 后处理

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P5
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.10, §5.3.5, §5.5.7, §8（第 7 步）
  - 当前实现参考：
    - `crates/scoopc/src/mir/inline.rs`
    - `crates/scoopc/src/mir/escape.rs`
    - `crates/scoopc/src/mir/pass_view.rs`
- 目标：
  - 在统一 late-lowering 之后，对编译器自己引入的 interface/icall/object 抽象层跑一轮窄的后处理；
  - 尽可能消掉显然可知的 `invoke` / `k.op$ret(...)` 动态调用、不可达 resume methods、无用 frame slots 与死状态；
  - 但绝不重新回高层 effect 语义分析，也不重新选择 `ImplPlan`。

- 必须实现的内容：
  1. 新增 late-lowered 专属优化 pass 模块。
     - 推荐位置：`crates/scoopc/src/effect_lowered/opt.rs`；
     - 它必须只消费 `LateLoweredProgram` / 等价 representation；
     - 明确禁止：重新读取 HIR/P3 MIR/P4 solver 结果作为主输入。
  2. 实现 late-lowered devirtualization。
     - 优先处理编译器内部生成的调用点：
       - 已知 target 的 canonical `invoke` 调用；
       - 已知 concrete continuation object 的 `k.op$ret(...)` 调用；
       - 已知不会逃逸的 continuation object/interface 调用。
     - 若 target 不确定，则保持保守，不得强行推断。
  3. 实现 late-lowered inlining。
     - 至少要支持：
       - 很小的 compiler-generated resume method body；
       - 薄 wrapper / adapter；
       - 明显无副作用的 trivial dispatch wrapper。
     - 该 pass 只能作用于 P5 生成的 internal object/interface 层，不能变成重新设计高层 lowering 的借口。
  4. 实现 late-lowered DCE / cleanup。
     - 至少要删除：
       - 永远不可达的 resume methods；
       - 已被 devirt/inlining 消掉后的死 wrapper；
       - 已无读者的 frame slots；
       - 已无前驱或已折叠的死状态；
       - 仅为完整接口而保留、但在当前闭世界下可删除的 vtable/interface 分量（若 representation 有此层次）。
  5. 明确该优化 pass 的不变式。
     - 它不能改变：
       - `StepSchema`
       - `CaseTag`
       - `ImplPlan`
       - canonical dynamic surface
       - continuation surface contract
     - 它只能删除冗余抽象层、内联已知实现、删死代码。
  6. 若当前已有可复用的 MIR-level inlining / escape analysis 工具，可以在满足“完全中立、不了解自己被哪条线调用”的前提下抽共享 helper；否则在 `effect_lowered/` 内部单独实现。
  7. 将 P5 stage 的最终公开输出固定为“后处理完成后的 final late-lowered representation”。
     - 若需要对比 pre-opt 与 post-opt，允许在测试内部保留辅助 dump；
     - 但对外 stage output 与未来 CLI dump 的 canonical 版本应以 post-opt final 结果为准。

- 必须遵从的约束：
  - 禁止在该优化 pass 中重新运行 `resolved_outward_cases` / `needs_reentry` / `impl_plan` 求解。
  - 禁止因为优化而重新发明第二套 lowering 入口或重新切 segmentation。
  - 禁止把 late-lowered 优化 pass 写成对 LLVM IR 的预处理；它必须发生在 LLVM 之前、且完全停留在 P5 representation 内。
  - 禁止让优化改变 `Step_F` 的类型身份、case tag、或 canonical dynamic surface。

- 验证：
  1. 新增/更新单元测试，推荐命名：
     - `refactor_late_opt_devirt_*`
     - `refactor_late_opt_inline_*`
     - `refactor_late_opt_dce_*`
     - `refactor_late_opt_preserves_contract_*`
  2. 测试至少覆盖：
     - 已知 continuation object 的 `k.op$ret(...)` 被 devirtualize；
     - trivial resume wrapper / invoke wrapper 被 inline；
     - 不可达 methods / 死状态 / 死 frame slots 被删除；
     - 优化前后 `StepSchema` / `CaseTag` / `ImplPlan` / canonical invoke contract 不变；
     - 优化不会重新进入 P4 solver 或重新切 state graph。
  3. 运行：
      - `cargo test -p scoopc --no-default-features refactor_late_opt_devirt`
      - `cargo test -p scoopc --no-default-features refactor_late_opt_inline`
      - `cargo test -p scoopc --no-default-features refactor_late_opt_dce`
      - `cargo test -p scoopc --no-default-features refactor_late_opt_preserves_contract`

- 完成条件：
  - late-lowered representation 上已存在一轮窄的 post-lowering 优化；
  - 局部样本上可观察到 compiler-generated interface/icall 抽象层被消解；
  - 但 canonical effect contract 与 `ImplPlan` 保持不变。
- 依赖：P5-T05R
- 完成记录：
  - （执行时填写）

## P5-T06R：Review late-lowered 后处理，确认它只做抽象层收缩，不重新回到高层 effect 分析

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.5, §5.5.7, §8
  - [`PLAN.md`](./PLAN.md) §2/P5
- 重点：
  - devirt/inline/DCE 是否只作用于 late-lowered representation；
  - 是否错误地重新运行了 outward-case 求解、`ImplPlan` 选择或 segmentation；
  - 优化是否保持 canonical `Step` / continuation / invoke contract 不变。
- 必须检查的文件/位置：
  - `crates/scoopc/src/effect_lowered/opt.rs`
  - `crates/scoopc/src/effect_lowered/materialize.rs`
  - refactor late-lowering stage 模块

- 验证：
  - 重新运行 P5-T06 的全部测试与命令；
  - 额外搜索：
    - `rg "resolved_outward_cases|needs_reentry|impl_plan|segment|solver|SCC" crates/scoopc/src/effect_lowered/opt.rs crates/scoopc/src/effect_lowered`
  - 要求：
    - 允许命中：读取已有字段用于断言/保留 contract；
    - 不允许命中：在 late opt pass 中重跑求解器、重新切 segmentation、或改写 `ImplPlan`。

- 完成条件：
  - review 能明确说明：P5 后处理只是在统一 transformation 之后做收缩，不是第二套 lowering；
  - 可进入 P5-T07。
- 依赖：P5-T06
- 完成记录：
  - （执行时填写）

## P5-T07：新增 `dump-effect-lowered` / snapshot 基线，并冻结 P5 -> P6 handoff contract

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P5，§2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.10, §5.2, §5.5, §7.3, §8
  - 当前 CLI/fixture 入口参考：
    - `crates/scoop/src/cli.rs`
    - `crates/scoop/src/commands/mod.rs`
    - `crates/scoop/src/fixtures/mod.rs`
- 目标：
  - 为 refactor late-lowered representation 提供稳定的用户可见 dump 入口；
  - 建立 dedicated snapshot/golden 基线；
  - 把 P6 “只翻译 late-lowered representation 到 LLVM，不再重做高层 effect lowering”的合同写死到代码与测试中。

- 必须实现的内容：
  1. 在 `scoop` CLI 上新增 `dump-effect-lowered` 子命令。
     - 必须修改：
       - `crates/scoop/src/cli.rs`
       - `crates/scoop/src/commands/mod.rs`
       - 新增 `crates/scoop/src/commands/dump_effect_lowered.rs`
     - `refactor` 路径必须显式进入 P5 late-lowering stage；
     - `legacy` 路径若当前无等价能力，必须给出稳定、可测试的“legacy pipeline 暂不支持该命令”诊断；
     - 禁止静默回退到 `dump-ir`、LLVM、或 legacy `effect/state_machine` 调试输出。
  2. 为 late-lowered representation 提供稳定 formatter。
     - 输出至少要稳定展示：
       - callable versions / `impl_plan`
       - `Step_F` enum definitions
       - internal resume interface definitions
       - continuation object definitions
       - state graph
       - boundary -> owner/resume mapping
       - frame schema / lifted slots
       - completion / cleanup / dropped-continuation paths
       - post-opt final 结果
     - 若 raw `Debug` 输出中的内部 id 太不稳定，则必须实现自定义 formatter，而不是把不稳定输出硬塞进 golden。
  3. 为 late-lowered 输出建立 dedicated fixture phase。
     - 推荐目录：`tests/fixtures/effect_lowered/**`
     - 推荐 golden 扩展：`.effectlowered`
     - 必须更新 `crates/scoop/src/fixtures/mod.rs`，增加对应 phase 与 golden 比对逻辑；
     - 该 phase 必须与 CLI 共用同一 stage helper / formatter。
  4. 新增 late-lowered fixtures，至少覆盖：
     - `tests/fixtures/effect_lowered/direct_and_fun_value_call.scoop`
     - `tests/fixtures/effect_lowered/dispatch_and_resume_call.scoop`
     - `tests/fixtures/effect_lowered/handle_perform.scoop`
     - `tests/fixtures/effect_lowered/handle_finally_boundary.scoop`
     - `tests/fixtures/effect_lowered/effect_boundary_inside_expr_context.scoop`
     - `tests/fixtures/effect_lowered/nested_handle_self_contained_vs_outward.scoop`
     - `tests/fixtures/effect_lowered/single_case_impl_plan.scoop`
     - `tests/fixtures/effect_lowered/dynamic_fallback_widening.scoop`
     - `tests/fixtures/effect_lowered/dropped_continuation_abandons_remaining_work.scoop`
     - `tests/fixtures/effect_lowered/continuation_resume_runtime_error_boundary.scoop`
     - 若前置阶段已存在同名 `.scoop` 源文件，允许直接复制复用源码；
     - 但 golden 必须是 late-lowered 专属输出，不能复用 `.mir` 或 `.effectfacts` golden。
  5. 把 P5 -> P6 handoff contract 写入代码注释或等价文档实体。
     - 至少要明确：
       - P6 的 canonical 输入是 P5 stage 输出；
       - P6 可以消费其中的 type 信息、state graph、frame schema、entry/interface definitions；
       - P6 不得重新做 boundary 识别、whole-function segmentation、frame lifting、continuation capture 合同设计、或 `ImplPlan` 选择；
       - P5 仍不提供 LLVM 物理布局，这部分属于 P6。
  6. 若 `dump-effect-lowered` 输出受 opt level 影响，至少锁定一组 `O0` 与一组较高优化级别的代表性样本，证明：
     - `ImplPlan` 差异已在 P4 决定；
     - P5 只按既定计划物化与优化；
     - CLI/golden 可稳定反映这种差异。

- 必须遵从的约束：
  - 禁止让 `dump-effect-lowered` 继续依赖 HIR/P3 MIR/P4 facts 自己拼文本，而不经过 P5 stage 输出。
  - 禁止把 late-lowered golden 与 legacy MIR/effect-facts golden 混在同一 phase 或扩展名下。
  - 禁止只做 Rust 单元测试而没有任何用户可见 dump/snapshot 入口。
  - 禁止把 `dump-effect-lowered` 实现为 `dump-ir`/LLVM 的别名或近似输出。

- 验证：
  1. 运行新增的 late-lowered snapshot / fixture 测试入口；
  2. 运行：
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/effect_lowered/dispatch_and_resume_call.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/effect_lowered/handle_finally_boundary.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/effect_lowered/dispatch_and_resume_call.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/effect_lowered/handle_perform.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/effect_lowered/single_case_impl_plan.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/effect_lowered/dropped_continuation_abandons_remaining_work.scoop`
   3. 额外验证 legacy unsupported 诊断：
      - `cargo run -p scoop --no-default-features -- --effect-pipeline legacy dump-effect-lowered tests/fixtures/effect_lowered/dispatch_and_resume_call.scoop`

- 完成条件：
  - `dump-effect-lowered` 已存在并稳定输出；
  - 仓库中已有 dedicated late-lowered snapshot/golden 基线；
  - P5 -> P6 handoff contract 已通过代码与测试锁定。
- 依赖：P5-T06R
- 完成记录：
  - （执行时填写）

## P5-T07R：Review P5 阶段退出条件，确认 P6 只需把 late-lowered representation 翻译到 LLVM

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P5，§2/P6，§3
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.9, §4.10, §4.16, §5.2, §5.3.2-§5.3.7, §5.5, §7.3, §8
- 重点：
  - refactor late-lowering stage 是否已独立存在，并成为 CLI / 测试 / P6 的共同入口；
  - `Step_F` / dynamic invoke / continuation object / resume interfaces 是否已形成完整中层合同；
  - whole-function segmentation、frame lifting、control/completion/cleanup/drop 合同是否已闭合；
  - post-lowering devirt/inline/DCE 是否已存在，且不重跑高层分析；
  - `dump-effect-lowered` 与 dedicated snapshot/golden 是否已建立；
  - P6 是否已可以只消费 P5 stage 输出做 LLVM lowering，而不再重做 boundary 识别、segment、frame lifting、或 `ImplPlan` 选择。

- 验证：
  - 重新运行 P5-T01 ~ P5-T07 的全部定向测试与命令；
  - 不再额外执行 `cargo test -p scoop` / `cargo test -p scoopc` 全 crate 测试；保持本阶段只做定向验证。

- 完成条件：
  - review 能明确说明：P5 已完成“late-lowered `Step` 路径落地（尚不接 LLVM）”的阶段目标；
  - P6 可以在不重新讨论 high-level effect lowering 设计的前提下，直接进入 LLVM codegen 新路径对接。
- 依赖：P5-T07
- 完成记录：
  - （执行时填写）
