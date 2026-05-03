# TODO（P6：LLVM codegen 新路径对接）

> 生成时间：2026-05-02  
> 设计基线：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 前置条件：`TODO-P5.md` 已完整完成；refactor late-lowering stage、`dump-effect-lowered`、以及 P5 -> P6 handoff contract 已存在并稳定；P5 产出的 late-lowered representation 已成为 LLVM 前唯一允许消费的 effect/continuation 中层合同。  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 本阶段目标：把 P5 产出的 late-lowered representation 接到新的 LLVM codegen 路径；在不切默认主线的前提下，让 `--effect-pipeline refactor` 下的 `build` / `run` / `--emit-llvm` / `--emit-obj` / `--emit-asm` 能端到端生成正确 IR 和可运行程序，同时保持“backend 只翻译 P5 state graph / frame schema / boundary contract，而不再重新做高层 effect lowering 设计”的边界。

## 全局约束

- [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 是本阶段唯一设计基线；若实现过程中需要改变主张，必须先回写该文档，再继续实现。
- [`PLAN.md`](./PLAN.md) 与 [`TODO-P0.md`](./TODO-P0.md)、[`TODO-P1.md`](./TODO-P1.md)、[`TODO-P2.md`](./TODO-P2.md)、[`TODO-P3.md`](./TODO-P3.md)、[`TODO-P4.md`](./TODO-P4.md)、[`TODO-P5.md`](./TODO-P5.md) 是本阶段执行前提；P6 不得重新开启 P0-P5 已收敛的 selector / typed HIR / direct-style MIR / effect facts / late-lowered representation 讨论。
- 本阶段只处理 refactor 新路径上的 LLVM codegen 对接。
  - 明确禁止：在 P6 中切换默认主线、执行 full regression、或删除 legacy 路径；这些属于 P7/P8。
  - 明确禁止：在 P6 中重新设计 `StepSchema` / `ContinuationSchema` / `resolved_outward_cases` / `impl_plan` / whole-function segmentation / frame lifting；这些在 P4/P5 已经闭合。
- P6 的 canonical 输入必须固定为 P5 的 refactor late-lowering stage 输出。
  - 允许消费：
    - 当前 callable version 的 late-lowered state graph；
    - frame schema / boundary map / resume-state map；
    - `StepSchema` / `ContinuationSchema` / `impl_plan` / callable-block-site facts 的只读查询面；
    - 与之绑定的 `TypeStore`、source map、entry identity、target triple、data layout、runtime symbol tables。
  - 明确禁止：
    - 回 AST / HIR / typecheck / P2 side table 补 effect 语义；
    - 回 P3 direct-style MIR 重新识别 boundary、再切 CFG、再做 frame lifting；
    - 回 P4 solver 重新选择 `resolved_outward_cases` / `needs_reentry` / `impl_plan`；
    - 在 backend 现场发明第二套 state-machine transformation。
- refactor LLVM backend 若需要新模块，推荐新增独立模块树：`crates/scoopc/src/llvm/codegen/effect_refactor/`。
  - 推荐最小拆分：
    - `mod.rs`
    - `types.rs`
    - `layout.rs`
    - `body.rs`
    - `calls.rs`
    - `gc.rs`
    - `runtime.rs`
    - `verify.rs`
  - 若实际落地名称不同，允许使用等价位置；
  - 但必须在完成记录中明确写出“实际路径 <-> 本 TODO 推荐路径”映射，避免后续 agent 误判。
- refactor LLVM 新路径必须拥有一个显式 stage 入口。
  - 推荐位置：`crates/scoopc/src/effect_refactor_pipeline/llvm_codegen_stage.rs`；
  - 允许使用等价位置；
  - 但它必须成为 `build` / `run` / build fixtures / P7 切主线前 smoke 的共同入口，而不是继续靠 legacy `emit_minimal_main_*_from_production_lowered_hir*` 换壳进入。
- 允许复用现有 `llvm/` 中**完全中立**的基础设施，例如：
  - target 初始化
  - LLVM pass pipeline
  - 通用 object/type/layout helper
  - 通用 call ABI / enum lowering / GC object header 辅助逻辑
  前提是这些 helper 不承担 effect/continuation 语义决策。
- 明确禁止把以下 legacy effect backend 作为 refactor authoritative 主线：
  - `crates/scoopc/src/llvm/codegen/effect/contract.rs`
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_bridge.rs`
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
  - `crates/scoopc/src/effect/state_machine/**`
  - `crates/scoopc/src/llvm/frontend.rs` 中以 `hir::LoweredHir` / production lowered HIR 作为 effect lowering 主输入的旧入口
  - `crates/scoopc/src/llvm/mod.rs` / `llvm/emit.rs` 中 `emit_minimal_main_*_from_production_lowered_hir*` 这一族旧 API 作为 refactor effect backend 的真正实现入口
- 可以接受的复用方式只有两种：
  1. 先把完全中立、且不含 legacy effect contract 的 LLVM helper 抽出来共享；
  2. 若做不到，则在 refactor 新路径中重建对应逻辑。
- `Step_F` 的 LLVM 物理表示必须继续严格服从 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.2。
  - 每个 `StepSchema(F)` 对应一个固定的内部 `enum` 形状；
  - `Complete` 与每个 case variant 必须一一对应；
  - `CaseTag` 顺序必须沿用 P4/P5 canonical schema；
  - `SingleCase(case_tag)` 只能改变可达分支与内部 dispatch 复杂度，不能改变 `Step_F` 的类型身份、tag 编号或用户可见/内部 canonical ABI。
- canonical dynamic callable surface 必须继续固定为 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.9 所述：`invoke(args_tuple) -> Step_F`。
  - direct/static path 允许调用已知 concrete entry；
  - 但这只能是相同合同下的直接调用优化，不能形成第二套 effect-specific ABI。
- continuation object 与 internal resume interfaces 的 LLVM lowering 必须严格服从 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.2-§5.3.6。
  - continuation object 是编译器内部对象；
  - 它完整实现对应 effect family 的 resume interfaces；
  - 每个 method 的参数类型由 `ContinuationSchema.resume_tuple_ty` 决定；
  - 每个 method 的返回类型统一为同一个 `Step_F<T>`；
  - 对不可能合法调用到的方法，允许 body 为 `unreachable`；
  - 但不能在接口或对象定义中删掉这些方法。
- `Unit` / `()` 的 codegen 规则必须遵守 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.1 附近关于 codegen 的约束（见 §5.3 段落中 930-999 行说明）。
  - `f()` 与 `f(())`、`k.resume()` 与 `k.resume(())` 在 lowering 后允许共享无额外 `Unit` 载荷的实现路径；
  - `Unit` 局部、参数、返回值在 codegen 层通常不必 materialize 为真实值；
  - 明确禁止：仅为满足 surface 语法而在 LLVM ABI 中引入有意义的 `Unit` 物理载荷或独立存储。
- runtime error 必须继续被视为普通 effect 分支的一部分，遵守 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.9。
  - `ContinuationAlreadyResumed` 等路径必须进入普通 outward case / `Step_F` 分支语义；
  - backend 明确禁止发明“隐藏 trap channel”“只在 LLVM backend 中存在的 outcome side channel”或其它第二传播通道。
- dropped continuation 语义必须继续遵守 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.7。
  - dropped continuation 表示剩余语言级计算被放弃；
  - 任何尚未执行到的 pending `finally` / cleanup 都不再执行；
  - `cleanup hook` 只是 runtime/GC 内部机制，不是继续执行 dropped continuation 的语义路径。
- Managed ABI / extern 边界必须继续遵守 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.8。
  - P6 不得让 `Step_F` / continuation / resume interface 穿过 Managed ABI / extern 边界；
  - 若当前 effectful extern 仍不支持，refactor LLVM 新路径必须保持同样的显式拒绝或诊断，不得偷偷绕过。
- refactor LLVM backend 只翻译 P5 的 state graph / frame schema / boundary contract。
  - 明确禁止：在 LLVM backend 再重新识别源码 shape、再切一次 CFG、再重新决定 owner-state/resume-state，或再重建“单 handle 局部状态机”。
  - 明确禁止：在 body emitter 中根据 `Span`、`hir::HandleExpr`、成员名 `resume`、或旧 `effect_op_call_sites` side table 猜测 effect 语义。
- refactor LLVM backend 必须逐步摆脱 legacy handler-stack / effect-outcome contract。
  - 新路径不得以 `scoop_effect_handler_stack_top`、`scoop_effect_handler_stack_swap_top`、`scoop_effect_outcome_consume_current`、`LegacyEffectBoundary`、`EffectSignal`、`EffectOutcome` 这一类 legacy runtime contract 作为 correctness 前提；
  - 若某些完全中立的 runtime helper 仍可复用，必须先抽离到不含 legacy effect 语义的层级；
  - 否则 refactor 新路径必须改为直接 lower P5 的 `Step` / continuation / state graph 合同。
- GC / stackmap / roots 规则在 refactor 新路径上必须与现有 LLVM pipeline 一致接通。
  - 允许复用 `crates/scoopc/src/llvm/pipeline.rs`、`llvm/stackmap.rs`、`llvm/codegen/gc.rs`、`llvm/codegen/runtime_symbols.rs`、`llvm/codegen/runtime_abi.rs` 等通用设施；
  - 但 refactor 路径必须确保 frame slots、continuation captures、`Step_F` payload、以及任何跨 safepoint/statepoint 活跃的 GC 引用都按当前支持的根模型可追踪；
  - 不得因为“当前只做定向验证”就接受 moving-GC 下已知错误的 root 形态。
- 所有优化级别必须共用同一条 refactor LLVM lowering 管线。
  - `O0` / debug build 不允许切到单独的 legacy effect backend；
  - 差异只允许体现在已有 `impl_plan` 结果、late-lowered post-opt 结果、以及 LLVM pass pipeline 优化级别上。
- 本阶段不做 full regression。
  - 只做 refactor LLVM 单元测试、build fixtures、定向 run-pass/runtime_gc/effect 验证，以及必要的 CLI smoke；
  - 不执行 `cargo test --all`；
  - 不执行 `cargo run -p scoop -- test` 的全量 fixture 扫描；
  - 不切默认主线。
- 所有验证都必须通过 `--effect-pipeline refactor` 进入，或通过与该 CLI 路径共用同一 stage helper 的 Rust 测试入口进入；禁止新增只在测试中存在的语义旁路。

## [DONE] P6-T01：建立 refactor LLVM codegen stage 入口，并让 `build` / `run` / `--emit-llvm` 新路径不再回落到 `production_lowered_hir`

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.10, §4.11, §4.16, §8
  - 当前实现参考：
    - `crates/scoopc/src/llvm/mod.rs`
    - `crates/scoopc/src/llvm/emit.rs`
    - `crates/scoopc/src/llvm/frontend.rs`
    - `crates/scoop/src/commands/build.rs`
    - `crates/scoop/src/commands/run.rs`
    - `crates/scoop/src/fixtures/mod.rs`
- 目标：
  - 在 refactor 新路径上建立一个明确的 LLVM codegen stage；
  - 让 `--effect-pipeline refactor` 下的 `build` / `run` / build fixtures 通过该 stage 消费 P5 stage 输出；
  - 切断 refactor LLVM 新路径对 `emit_minimal_main_*_from_production_lowered_hir*`、`prepare_single_file_codegen_unit_*`、以及 `hir::LoweredHir` effect lowering 兼容入口的依赖。

- 必须实现的内容：
  1. 在 refactor pipeline 下新增 LLVM codegen stage 模块。
     - 推荐位置：`crates/scoopc/src/effect_refactor_pipeline/llvm_codegen_stage.rs`；
     - 若最终实际路径不同，允许使用等价位置；
     - 但它必须是 refactor LLVM 新路径的显式阶段入口，而不是在 legacy `llvm/frontend.rs` / `llvm/emit.rs` 里插入隐式 pipeline 分支后继续用旧数据流推进。
  2. 为 refactor LLVM stage 定义一个明确的共享入口 API。
     - 该入口必须直接接收 P5 late-lowering stage 输出，或与之等价的结构化输入；
     - 它必须成为以下调用方的共同入口：
       - `scoop build --effect-pipeline refactor`
       - `scoop run --effect-pipeline refactor`
       - build fixtures / run-pass fixtures / runtime_gc fixtures 中经由 build 的新路径
       - Rust 单元测试
     - 明确禁止：CLI、fixtures、测试各自绕开 stage 自己重建输入。
  3. 若 `inkwell::Module` 的生命周期使“拥有型 stage 输出类型”实现困难，允许使用等价的 owning builder / callback-style API；
     - 但阶段边界必须仍然显式，且至少能稳定暴露：
       - 当前 target info / data layout
       - 由 P5 输入得到的 LLVM module builder 查询面
       - 供 `.ll` / `.o` / `.s` 三类产物共用的统一生成路径
       - 供单元测试读取 IR/module 的稳定入口
  4. 在 `crates/scoopc/src/llvm/emit.rs` 与 `crates/scoopc/src/llvm/mod.rs` 中新增 refactor 专属 emit 入口。
     - 名称可自定；
     - 但必须与当前 legacy `emit_minimal_main_ir_from_production_lowered_hir*` / `emit_minimal_main_obj_to_file_from_production_lowered_hir*` / `emit_minimal_main_asm_to_file_from_production_lowered_hir*` 这类旧 API 明确分离；
     - 允许保留 legacy API；
     - 但 refactor 路径不得继续把这些旧 API 当成自己的真正实现入口。
  5. 调整 driver/CLI 入口。
     - `crates/scoop/src/commands/build.rs`：
       - 在 `--effect-pipeline legacy` 下继续维持当前行为；
       - 在 `--effect-pipeline refactor` 下，`Executable` / `LlvmIr` / `Obj` / `Asm` 必须进入新的 refactor LLVM stage；
       - 禁止在 refactor 模式下继续调用 `scoopc::llvm::emit_minimal_main_*_from_production_lowered_hir*`。
     - `crates/scoop/src/commands/run.rs`：
       - 必须通过同一个 refactor build path 获得产物；
       - 不允许为了 run 再开一条旧 backend 旁路。
  6. 若当前 build fixtures / Rust tests 只知道旧 `production_lowered_hir` 风格 helper，则必须为 refactor 路径补一层新 helper。
     - 允许保留 legacy helper；
     - 但 refactor 测试入口必须显式通过新 stage。
  7. 在 stage 注释或等价文档中明确写出 P6 的入口不变式：
     - 输入是 P5 late-lowered representation；
     - refactor LLVM backend 不再以 `hir::LoweredHir` 为 effect lowering 主输入；
     - P7 只切 selector / 做 full regression，不重新设计此入口。

- 必须遵从的约束：
  - 禁止在 `crates/scoopc/src/llvm/codegen/effect/**` 的 legacy 主实现里直接加入 refactor pipeline 分支并继续复用旧 contract。
  - 禁止把 refactor LLVM stage 语义藏在 `build.rs` / `run.rs` / fixtures runner 里；stage 构造必须属于 compiler crate。
  - 禁止把 refactor 新路径继续收口到 `hir::LoweredHir` 的 effect 兼容入口上，再让 backend 自己现场补 effect 语义。
  - 禁止把“先让 refactor CLI 路径仍走 legacy emit helper，后面再整理”为完成标准。

- 验证：
  1. 新增/更新单元测试，推荐命名：`refactor_llvm_codegen_stage_*`，至少覆盖：
     - refactor LLVM stage 入口存在且可被测试调用；
     - `build --effect-pipeline refactor` 确实进入新 stage；
     - legacy build 路径仍沿用原有实现；
     - refactor `LlvmIr` / `Obj` / `Asm` 三种 emit 共享同一 stage 入口。
  2. 运行：
     - `cargo test -p scoopc refactor_llvm_codegen_stage`
     - `cargo run -p scoop -- --effect-pipeline legacy build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p6_legacy_emit.ll`
     - `cargo run -p scoop -- --effect-pipeline refactor build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p6_refactor_emit.ll`
     - `cargo run -p scoop -- --effect-pipeline refactor build --emit-obj tests/fixtures/run-pass/minimal_main.scoop -o /tmp/p6_refactor_emit.o`
     - `cargo run -p scoop -- --effect-pipeline refactor build --emit-asm tests/fixtures/run-pass/minimal_main.scoop -o /tmp/p6_refactor_emit.s`
  3. 要求：
     - legacy 命令继续成功；
     - refactor 三种 emit 都能通过新的 stage 产物路径成功输出；
     - 相关测试能证明 refactor 没有回落到 `production_lowered_hir` 旧入口。

- 完成条件：
  - refactor LLVM 新路径已拥有独立 stage 入口；
  - `build` / `run` / `--emit-llvm` / `--emit-obj` / `--emit-asm` 在 refactor 模式下已不再回落到 legacy `production_lowered_hir` effect backend；
  - 后续 P6-T02 及之后的任务可以只围绕这条新 stage 继续推进。
- 依赖：`TODO-P5.md` 最后一项 review 完成
- 完成记录：
  - 已新增显式 stage：`crates/scoopc/src/effect_refactor_pipeline/llvm_codegen_stage.rs`；refactor `build` / `run` / build fixtures 现在统一先构造 `RefactorLlvmCodegenStageInput`，再在 compiler crate 内显式推进 `TypedHirStageOutput -> RefactorMirStageOutput -> RefactorEffectFactsStageOutput -> RefactorEffectLoweredStageOutput`，最后才进入 LLVM emit。
  - 已把 `crates/scoopc/src/effect_refactor_pipeline/mod.rs` 中的 `emit_production_llvm_artifact_to_file(...)` 改为按 stage 分发：legacy 继续走原有 `emit_minimal_main_*_from_production_lowered_hir*`；refactor 改走新的 `llvm_codegen_stage::emit_artifact_to_file(...)`，不再把旧 production HIR emit helper 当成真正实现入口。
  - 已在 `crates/scoopc/src/llvm/emit.rs` / `crates/scoopc/src/llvm/mod.rs` 新增 refactor 专属 emit 入口：
    - `emit_refactor_main_ir_to_file_from_stage_output(...)`
    - `emit_refactor_main_obj_to_file_from_stage_output(...)`
    - `emit_refactor_main_asm_to_file_from_stage_output(...)`
    - `build_refactor_main_module_from_stage_output(...)`（测试读取 IR/module 的稳定入口）
  - 已新增 `hir::LoweredHir::clone_hir_compat_scaffold_without_materialized_mir()`，把 refactor P6 入口里仍需复用的非 effect HIR side tables 明确降成过渡 scaffold；该 scaffold 不再携带 production pass-view，避免 refactor 路径回落到旧 `production_lowered_hir` contract。
  - 已显式保留 refactor LLVM backend 目录边界：推荐的 `crates/scoopc/src/effect_refactor_pipeline/llvm_codegen_stage.rs` 与实际路径一致；推荐的 `crates/scoopc/src/llvm/codegen/effect_refactor/**` 当前先落到 `crates/scoopc/src/llvm/codegen/effect_refactor/mod.rs` 占位根，P6-T02/P6-T03 将继续在该目录下填充 type/layout/body 细分实现；本任务里的 refactor emit 入口实际落在 `crates/scoopc/src/llvm/emit.rs`。
  - 已新增 `refactor_llvm_codegen_stage_*` 单测，覆盖：stage 可构造、refactor build helper 确实进入新 stage、legacy helper 继续沿用原有实现、以及 `.ll/.o/.s` 三种 emit 共用同一 stage 入口。
- 已运行验证：
  - `cargo test -p scoopc refactor_llvm_codegen_stage`
  - `cargo run -p scoop -- --effect-pipeline legacy build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p6_legacy_emit.ll`
  - `cargo run -p scoop -- --effect-pipeline refactor build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p6_refactor_emit.ll`
  - `cargo run -p scoop -- --effect-pipeline refactor build --emit-obj tests/fixtures/run-pass/minimal_main.scoop -o /tmp/p6_refactor_emit.o`
  - `cargo run -p scoop -- --effect-pipeline refactor build --emit-asm tests/fixtures/run-pass/minimal_main.scoop -o /tmp/p6_refactor_emit.s`
  - `cargo run -p scoop -- --effect-pipeline refactor run tests/fixtures/run-pass/minimal_main.scoop`
  - `cargo test -p scoop build_emit_llvm_writes_ll_file`
  - `cargo test -p scoop run_builds_and_executes_minimal_main`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## [DONE] P6-T01a：为 refactor LLVM stage 建立 fail-fast 守卫，禁止 effectful lowering 静默回落到 legacy handler-stack / `EffectOutcome` backend

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.9, §4.10, §4.11, §4.16, §5.2, §5.3.7-§5.3.9, §8
  - `crates/scoopc/src/effect_refactor_pipeline/llvm_codegen_stage.rs`
  - `crates/scoopc/src/llvm/emit.rs`
  - `crates/scoopc/src/llvm/codegen/{mod.rs,mir_body.rs,runtime_abi.rs}`
  - `crates/scoopc/src/llvm/codegen/effect/**`（只能作 legacy 对照）
- 背景：
  - `P6-T01R` 审阅发现：refactor stage 已显式构造 `RefactorLlvmCodegenStageOutput`，并把 `LateLoweredProgram`/pass-view handoff 传入 `llvm/emit.rs`；
  - 但当前 `crates/scoopc/src/llvm/emit.rs` 对 `late_lowered_program` 的消费只剩入口 callable 存在性校验，实际 lowering 仍统一走 `build_main_module_from_codegen_entry(...) -> CompilationUnitCodegenCx -> mir_body.rs`；
  - 该主路径仍会调用 `build_fun_callee_suspend_plan(...)`、`swap_effect_handler_stack_top(...)`，并依赖 `runtime_abi.rs` 中的 `ScoopEffectSignal` / `ScoopEffectOutcome` / handler-stack runtime contract；
  - 因此当前 refactor LLVM 路径还不能被审阅为“已与 old effect backend 分离”，必须先切断这种静默回落。
- 目标：
  - 在 P6-T02/P6-T03 真正落地 refactor type/layout/body lowering 之前，refactor LLVM stage 必须保证：
    - 共享的非 effect-neutral LLVM helper 可以继续复用；
    - 任何会进入 legacy effect state-machine / handler-stack / `EffectSignal` / `EffectOutcome` lowering 的请求，都必须在 refactor stage 边界被显式拒绝，而不是静默回落。

- 必须实现的内容：
  1. 基于 P5 handoff 为 refactor LLVM stage 增加一层显式 capability/unsupported-path 检查。
     - authoritative 输入必须来自 `RefactorEffectLoweredStageOutput` / `LateLoweredProgram` / 其稳定查询面；
     - 明确禁止仅靠 HIR 名字、`Span`、或 legacy side table 启发式决定是否允许 lowering。
  2. 对尚未迁移的 effectful lowering 给出结构化错误。
     - 当请求会落入 legacy `llvm/codegen/effect/**`、`crate::effect::state_machine/**`、handler-stack runtime helper、或 `EffectSignal` / `EffectOutcome` ABI 时，refactor 路径必须 fail fast；
     - 错误信息必须明确指出：这是 refactor LLVM backend 尚未迁移完成的 lowering 路径，而不是普通 frontend/typecheck 失败。
  3. 保持已验证的 non-effectful 共享子集继续可用。
     - `build --emit-llvm` / `--emit-obj` / `--emit-asm` / `run` 对当前 minimal/non-effectful 样例仍必须继续经同一 refactor stage 成功；
     - 禁止为 smoke 单独新增测试旁路。
  4. 补充定向测试与回归样例。
     - 至少覆盖：
       - non-effectful fixture 在 refactor stage 下继续成功；
       - 一个包含 `perform`/`handle` 或 `Continuation.resume` 的代表性 effectful fixture 在 refactor build 下不再静默进入 legacy backend，而是返回显式拒绝诊断。
  5. 在注释或文档里明确记录当前边界。
     - 在 P6-T02/P6-T03 完成前，refactor LLVM stage 允许复用中立 helper；
     - 但禁止把 legacy effect backend 当成 refactor correctness path。

- 必须遵从的约束：
  - 禁止把“当前先继续走 legacy effect backend，只是外面多包一层 stage”当作本任务完成标准。
  - 禁止通过缩小 fixture、隐藏 selector、或新增测试私有 helper 来掩盖 fallback。
  - 禁止把 fail-fast 检查写成只在测试里生效的断言；CLI 实际路径必须共享同一检查。

- 验证：
  - `cargo test -p scoopc refactor_llvm_codegen_stage`
  - `cargo run -p scoop -- --effect-pipeline refactor build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p6_refactor_emit.ll`
  - `cargo run -p scoop -- --effect-pipeline refactor build --emit-obj tests/fixtures/run-pass/minimal_main.scoop -o /tmp/p6_refactor_emit.o`
  - `cargo run -p scoop -- --effect-pipeline refactor build --emit-asm tests/fixtures/run-pass/minimal_main.scoop -o /tmp/p6_refactor_emit.s`
  - `cargo run -p scoop -- --effect-pipeline refactor run tests/fixtures/run-pass/minimal_main.scoop`
  - 新增一个包含 `perform`/`handle` 或 `Continuation.resume` 的 refactor build 定向验证，要求输出显式“未迁移 lowering，禁止回落到 legacy effect backend”的诊断

- 完成条件：
  - refactor LLVM stage 不再把 legacy effect state-machine / handler-stack / `EffectSignal` / `EffectOutcome` lowering 当作静默 correctness path；
  - P6-T01R 可以据此继续审阅“refactor 路径已与 old effect backend 分离”的入口边界；
  - P6-T02/P6-T03 可以在这个受保护的 stage 边界上继续实现真实 lowering。
- 依赖：P6-T01
- 完成记录：
  - 已在 `crates/scoopc/src/llvm/mod.rs` 新增 `LlvmEmitError::RefactorEffectLoweringUnsupported`，把“refactor LLVM backend 尚未迁移该 lowering，且已禁止回落到 legacy handler-stack / EffectOutcome backend”提升为结构化错误，而不再伪装成普通 frontend/typecheck 失败。
  - 已在 `crates/scoopc/src/llvm/emit.rs` 的 refactor stage emit 入口前增加 capability 检查：以 `entry main + reachable callees` 为范围，逐个回查 `LateLoweredProgram`；只要 callable 仍需 outward `Step_F` / boundary / resume-state lowering，就在进入 `CompilationUnitCodegenCx` body emission 前 fail fast。
  - 当前检查完全基于 P5 handoff 的稳定结构化信息（`resolved_outward_cases`、`boundary_map`、`resume_state_map`），没有回退到 HIR 名字、`Span`、或 legacy effect side table 启发式。
  - 已保持 non-effectful 共享子集可用：`build --emit-llvm`、`--emit-obj`、`--emit-asm`、`run` 在 refactor stage 下对 `emit_llvm_basic.scoop` / `minimal_main.scoop` 继续成功，且仍经同一 stage 路径推进。
  - 已在 `crates/scoopc/src/effect_refactor_pipeline/llvm_codegen_stage.rs` 新增回归测试 `refactor_llvm_codegen_stage_rejects_unmigrated_effect_lowering`，使用 handled `Raise.raise(...)` 程序验证 refactor build 会显式返回 `RefactorEffectLoweringUnsupported`，并指出 `perform boundary lowering` / `resume-state lowering` 尚未迁移。
  - 已运行验证：
    - `cargo test -p scoopc refactor_llvm_codegen_stage`
    - `cargo run -p scoop -- --effect-pipeline refactor build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p6_refactor_emit.ll`
    - `cargo run -p scoop -- --effect-pipeline refactor build --emit-obj tests/fixtures/run-pass/minimal_main.scoop -o /tmp/p6_refactor_emit.o`
    - `cargo run -p scoop -- --effect-pipeline refactor build --emit-asm tests/fixtures/run-pass/minimal_main.scoop -o /tmp/p6_refactor_emit.s`
    - `cargo run -p scoop -- --effect-pipeline refactor run tests/fixtures/run-pass/minimal_main.scoop`
    - `cargo run -p scoop -- --effect-pipeline refactor build --emit-llvm tests/fixtures/effect_facts/handle_perform.scoop -o /tmp/p6_refactor_effect_fail.ll`（预期失败，并输出显式“未迁移 lowering，禁止回落到 legacy effect backend”诊断）
    - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## [DONE] P6-T01R：Review LLVM stage 入口，确认 refactor 路径已与 legacy `production_lowered_hir` / old effect backend 分离

- 参考：
  - [`PLAN.md`](./PLAN.md) §0，§2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.10, §4.11, §4.16, §8
- 重点：
  - refactor LLVM codegen 是否真的以 P5 stage 输出为输入；
  - `build` / `run` / `emit-llvm` 是否已进入新的 stage，而不是继续走 legacy `production_lowered_hir` 入口；
  - 是否仍然避免把 refactor 逻辑混进旧 `llvm/codegen/effect/**` 主实现里。
- 必须检查的文件/位置：
  - 新增的 `crates/scoopc/src/effect_refactor_pipeline/llvm_codegen_stage.rs`
  - 新增的 `crates/scoopc/src/llvm/codegen/effect_refactor/**`
  - `crates/scoopc/src/llvm/emit.rs`
  - `crates/scoopc/src/llvm/mod.rs`
  - `crates/scoop/src/commands/build.rs`
  - `crates/scoop/src/commands/run.rs`
  - `crates/scoopc/src/llvm/frontend.rs`

- 验证：
  - 重新运行 P6-T01 的全部测试与命令；
  - 额外搜索：
    - `rg "production_lowered_hir|prepare_single_file_codegen_unit|LoweredHir|effect_pipeline|refactor|legacy" crates/scoopc/src/llvm crates/scoopc/src/effect_refactor_pipeline crates/scoop/src/commands`
  - 要求：
    - 允许命中：legacy API、测试、注释、driver 级 selector 分发；
    - 不允许命中：refactor LLVM 主实现仍把旧 `production_lowered_hir` 入口当作真正的 lowering 数据源。

- 完成条件：
  - review 能明确说明：refactor LLVM stage 已独立存在，且 refactor CLI 路径不再靠 legacy HIR effect backend 换壳推进；
  - 可进入 P6-T02。
- 依赖：P6-T01a
- 完成记录：
  - 2026-05-03：审阅发现 blocker。`crates/scoopc/src/llvm/emit.rs` 当前虽把 `RefactorEffectLoweredStageOutput` 带入 `LoweredCodegenEntry`，但 `late_lowered_program` 只在 `build_main_module_from_codegen_entry(...)` 中做“入口 callable 是否存在于 late-lowered program”校验；实际 lowering 仍统一经 `CompilationUnitCodegenCx` / `mir_body.rs` 推进。
  - 该主路径仍会触发 legacy effect lowering contract：例如 `crates/scoopc/src/llvm/codegen/mir_body.rs` 继续以 `build_fun_callee_suspend_plan(...)` 判定 effect-state-machine body，`crates/scoopc/src/llvm/codegen/{mir_body,call/dispatch.rs,call/resume.rs}` 仍会调用 `swap_effect_handler_stack_top(...)`，而 `crates/scoopc/src/llvm/codegen/runtime_abi.rs` / `effect/contract.rs` 仍保留 `ScoopEffectSignal`、`ScoopEffectOutcome` 与 handler-stack runtime ABI。
  - 复跑 `P6-T01` 的核心验证后（`cargo test -p scoopc refactor_llvm_codegen_stage`、legacy/refactor `build --emit-llvm`、refactor `build --emit-obj`、refactor `build --emit-asm`、refactor `run`），命令均通过；这说明当前测试矩阵只证明了 stage shell 和 non-effectful smoke 可用，尚不能证明 refactor LLVM 主实现已与 old effect backend 分离。
  - 因此新增前置任务 `P6-T01a`，先在 refactor LLVM stage 边界禁止 effectful lowering 静默回落到 legacy handler-stack / `EffectOutcome` backend；待该边界补齐后再继续本 review。
  - 2026-05-03：在 `P6-T01a` 落地后复审通过。`crates/scoopc/src/effect_refactor_pipeline/mod.rs` 中 `emit_production_llvm_artifact_to_file(...)` 已按 stage 分发：`legacy` 继续显式走 `emit_minimal_main_*_from_production_lowered_hir_with_entry_with_opt_level(...)`，`refactor` 则直接进入 `llvm_codegen_stage::emit_artifact_to_file(...)`，不再把旧 production HIR emit helper 当成 refactor 路径的真正入口。
  - `crates/scoopc/src/effect_refactor_pipeline/llvm_codegen_stage.rs` 已成为 refactor LLVM 的显式 stage：它把统一 frontend `LoweredHir` 明确推进到 `TypedHirStageOutput -> RefactorMirStageOutput -> RefactorEffectFactsStageOutput -> RefactorEffectLoweredStageOutput`，再产出 `RefactorLlvmCodegenStageOutput`；`hir_compat_scaffold` 明确去除了 `materialized_pass_view()`，避免再回落到旧 `production_lowered_hir` contract。
  - `crates/scoopc/src/llvm/emit.rs` 的 refactor 入口 `build_refactor_main_module_from_stage_output(...)` / `emit_refactor_main_*_from_stage_output(...)` 只从 stage handoff 读取 `materialized_pass_view()` 与 `LateLoweredProgram`；legacy `prepare_single_file_codegen_unit_*` / `emit_minimal_main_*_from_production_lowered_hir*` 仍仅属于 legacy 单文件/production API，不再是 refactor CLI 路径的数据源。
  - `crates/scoop/src/commands/build.rs` 中 `Executable` / `LlvmIr` / `Obj` / `Asm` 四条 LLVM 产物路径全部共用 `effect_refactor_pipeline::emit_production_llvm_artifact_to_file(...)`；`crates/scoop/src/commands/run.rs` 继续复用 `build::run(...)`，因此 refactor `run` 与 `build` 共享同一 LLVM stage 入口，没有额外旧 backend 旁路。
  - 额外文本搜索确认：允许命中只出现在 legacy API、测试、注释、导出层或 dispatcher 选择逻辑中；`crates/scoopc/src/llvm/codegen/effect_refactor/**` 仍保持独立目录边界，未把 refactor 主逻辑重新塞回 `crates/scoopc/src/llvm/codegen/effect/**`。同时，`crates/scoopc/src/llvm/emit.rs` 中的 `ensure_refactor_effect_lowering_is_supported(...)` 继续把任何仍需 legacy handler-stack / `EffectOutcome` lowering 的 effectful callable 在 stage 边界显式拒绝，因此 old effect backend 已不再是 refactor correctness path。
- 已重新运行验证：
  - `cargo test -p scoopc refactor_llvm_codegen_stage`
  - `cargo run -p scoop -- --effect-pipeline legacy build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p6_legacy_emit.ll`
  - `cargo run -p scoop -- --effect-pipeline refactor build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p6_refactor_emit.ll`
    - `cargo run -p scoop -- --effect-pipeline refactor build --emit-obj tests/fixtures/run-pass/minimal_main.scoop -o /tmp/p6_refactor_emit.o`
    - `cargo run -p scoop -- --effect-pipeline refactor build --emit-asm tests/fixtures/run-pass/minimal_main.scoop -o /tmp/p6_refactor_emit.s`
    - `cargo run -p scoop -- --effect-pipeline refactor run tests/fixtures/run-pass/minimal_main.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor build --emit-llvm tests/fixtures/effect_facts/handle_perform.scoop -o /tmp/p6_refactor_effect_fail.ll`（预期失败，并输出显式 `RefactorEffectLoweringUnsupported` 诊断）
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## [DONE] P6-T01b：扩展 refactor build/LLVM handoff 的 ABI 可见性，保证 P6-T02 build fixtures 能在不触发 legacy lowering 的前提下观察 effectful `Step` / continuation 形状

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.9, §5.2, §5.3.2-§5.3.6, §8
  - `crates/scoop/src/commands/build.rs`
  - `crates/scoopc/src/effect_refactor_pipeline/llvm_codegen_stage.rs`
  - `crates/scoopc/src/llvm/emit.rs`
- 背景：
  - 2026-05-03 在执行 `P6-T02` 时发现：refactor build/frontend 目前仍以 `EntryMain` request-root 作为 production MIR/materialized pass-view 的 authoritative root；因此 LLVM stage 在 `build --emit-llvm` 主路径上只能稳定看到 entry `main` 与其真实可达 body。
  - 对 `P6-T02` 设计的 build fixtures 而言，这带来两个直接问题：
    1. 把 effectful ABI carrier helper 仅作为“不可达 top-level helper”放在源文件里，不会进入 build 主路径 handoff，因此新的 `Step_F` / resume-interface / continuation ABI 形状不会出现在 `.ll` 产物中；
    2. 若为了让 helper 可达而在 `Pure main` 中引入 self-contained `handle { ... }` / 其它 reachable effect boundary，则当前主路径又会重新落回 legacy `scoop.effect.frame.*` lowering，违背 `P6-T01a` 的 fail-fast 边界与 `P6-T02` 的“不得靠 legacy lowering 产出 ABI 断言”约束。
  - 因此，`P6-T02` 的 build-fixture 验证当前被 production handoff 可见性问题阻塞；在修复之前，继续通过改 fixture 形状规避只会把验证建立在错误路径上。
- 目标：
  - 让 refactor build 主路径在不让 `main` 变成 effectful、也不重新进入 legacy effect body lowering 的前提下，能够为 `P6-T02` 所需的 build fixtures 暴露 canonical `Step_F` / continuation object / resume-interface / dynamic invoke ABI 形状；
  - 同时保持 P6-T01a 的 fail-fast 承诺：任何真正需要 legacy `handler-stack` / `EffectOutcome` body lowering 的 reachable effectful body 仍必须显式拒绝。

- 必须实现的内容：
  1. 为 refactor build/LLVM stage 增加一条 authoritative 的“ABI shell 可见性”路径。
     - 允许方案：在 build frontend handoff 中显式携带 request-source 范围的 ABI-only callable/schema shell；或其它等价的 compiler-owned stage 输入；
     - 但禁止：在 fixture runner / CLI 中偷偷走 dump path、测试私有 helper、或第二套不共享 production stage 的临时 lowering。
  2. 明确区分“ABI shell 可见性”与“reachable body lowering”。
     - 前者应允许 `P6-T02` 的 build fixtures 观察到 effectful `Step_F` / continuation / resume-interface 形状；
     - 后者在 `P6-T03` 真正落地前，仍必须继续受 `P6-T01a` 的 fail-fast 保护。
  3. 修正当前 reachable helper/handle 形状下可能重新落入 legacy `scoop.effect.frame.*` lowering 的入口边界。
     - 若 reachable shape 仍会进入 old effect backend，则 refactor build 必须显式拒绝；
     - 禁止让 `P6-T02` build fixtures 通过 legacy step/dispatch IR 断言新的 ABI contract。
  4. 调整 `P6-T02` 的 build fixtures，使其在真实 refactor build 主路径上可稳定通过。

- 必须遵从的约束：
  - 禁止把“不可达 helper + build path看不到它”当作 `P6-T02` build fixture 的可接受现状。
  - 禁止通过 effectful `main`、隐藏 selector、或让 reachable helper 静默进入 legacy backend 来制造 `.ll` 断言样本。
  - 禁止新增只在测试中存在、与 `scoop build --effect-pipeline refactor --emit-llvm` 不同源的 ABI 物化旁路。

- 验证：
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_step_enum_single_case.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_invoke_unit_payload.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_continuation_interface_full_methods.scoop`
  - 额外要求：任何为了让 fixture helper“变成可达”而出现的 reachable handle/call boundary，若仍会进入 legacy effect body lowering，则必须显式失败，而不是悄悄生成 `scoop.effect.frame.*` / old dispatch IR。

- 完成条件：
  - `P6-T02` 的三类 build fixtures 能在真实 refactor build 主路径上稳定观察到新的 ABI shell；
  - 同时，任何仍需 legacy effect body lowering 的 reachable effectful body 继续 fail fast；
  - `P6-T02` 可以在此基础上继续完成剩余 build-fixture / lint / 文档收口。
- 依赖：P6-T01R
- 完成记录：
  - 2026-05-03：`crates/scoop/src/commands/build.rs` 新增了按 request-root 策略切换的 build lowering helper；refactor build 现在在保留原有 `EntryMain` rooted production lowering 的同时，额外构造一份 `RequestSources` rooted 的 `abi_visibility_lowered_hir`，专门用于 request-source 范围 ABI shell 可见性。
  - `crates/scoopc/src/effect_refactor_pipeline/{mod.rs,llvm_codegen_stage.rs}` 已把这份附加 handoff 接入 refactor LLVM stage：primary `effect_lowered_stage_output` 继续作为 reachable body lowering / fail-fast 的 authoritative 输入；新增的 `abi_visibility_effect_lowered_stage_output` 只负责发布 `Step_F` / continuation / resume-interface / dynamic invoke ABI shell。
  - `crates/scoopc/src/llvm/{emit.rs,mod.rs}` 已把 refactor emit handoff 收口到 `RefactorStageEmitInput`；构建 module 时现在显式区分“reachable body lowering program”与“ABI shell visibility program”，并在进入 legacy effect-frame / handler-stack body lowering 之前返回结构化 `RefactorEffectLoweringUnsupported`，不再允许 build fixture 靠旧 `scoop.effect.frame.*` IR 断言新 ABI。
  - `crates/scoop/src/fixtures/mod.rs` 的 build fixture runner 现已透传 `session.options()` 给内部 `scoop build`，修复了 `scoop test --fixtures ...` 在 `--effect-pipeline refactor` 下仍默认走 legacy build session 的问题。
  - `tests/fixtures/build/{effect_refactor_step_enum_single_case,effect_refactor_dynamic_invoke_unit_payload,effect_refactor_continuation_interface_full_methods}.scoop` 已改为“pure main + 不可达 effectful helper”形状，直接通过真实 refactor build 主路径观察 ABI shell，不再借 reachable handle 制造 legacy lowering 样本。
  - 已新增定向回归：
    - `crates/scoop/src/commands/build.rs`：验证不可达 effectful helper ABI shell 会出现在 refactor build IR 中，以及 reachable self-contained effect lowering 会显式拒绝；
    - `crates/scoop/src/fixtures/mod.rs`：验证 build fixtures 会把 refactor session 选项传给内部 `scoop build`。
  - 已运行验证：
    - `cargo test -p scoopc refactor_llvm_codegen_stage`
    - `cargo test -p scoopc refactor_llvm_step_layout`
    - `cargo test -p scoopc refactor_llvm_frame_layout`
    - `cargo test -p scoopc refactor_llvm_continuation_layout`
    - `cargo test -p scoopc refactor_llvm_unit_abi`
    - `cargo test -p scoop refactor_build_publishes_request_source_abi_shells_for_unreachable_effectful_helpers`
    - `cargo test -p scoop refactor_build_rejects_reachable_self_contained_legacy_effect_body_lowering`
    - `cargo test -p scoop build_fixtures_propagate_refactor_session_options_to_build_command`
    - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_step_enum_single_case.scoop`
    - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_invoke_unit_payload.scoop`
    - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_continuation_interface_full_methods.scoop`
    - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## [DONE] P6-T02：把 P5 的 `Step` / frame / continuation / resume-interface 合同下沉到 LLVM type/layout lowering

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.9, §5.2, §5.3.2-§5.3.6, §5.3（930-999 行关于 `Unit` 的 codegen 约束）, §8
  - 当前实现参考：
    - `crates/scoopc/src/llvm/codegen/types.rs`
    - `crates/scoopc/src/llvm/codegen/layout.rs`
    - `crates/scoopc/src/llvm/codegen/enum_lowering.rs`
    - `crates/scoopc/src/llvm/codegen/runtime_abi.rs`
    - `crates/scoopc/src/llvm/codegen/runtime_symbols.rs`
    - `crates/scoopc/src/llvm/codegen/effect/contract.rs`（只能作 legacy 对照）
    - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`（只能作 legacy 对照）
- 目标：
  - 把 P5 late-lowered representation 中的 `Step_F`、frame schema、continuation object、internal resume interfaces、dynamic invoke / direct invoke signatures，统一下沉到新的 LLVM type/layout materialization 层；
  - 让后续 body emitter 只消费结构化的 LLVM 布局与函数签名，不再回 P5/P4/HIR 现场拼装 ABI。

- 必须实现的内容：
  1. 在 refactor LLVM backend 中建立独立的 type/layout materialization 子层。
     - 推荐模块：
       - `crates/scoopc/src/llvm/codegen/effect_refactor/types.rs`
       - `crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs`
       - 若需要，也可增加 `interfaces.rs` / `symbols.rs` / `verify.rs`
     - 它必须只消费 P5 representation 与通用 LLVM type/layout helper；
     - 明确禁止：让 type/layout 物化依赖 `hir::HandleExpr`、legacy `EffectSignal`/`EffectOutcome`、或 `Span` 驱动的 effect contract。
  2. 为每个 `StepSchemaId` 物化 canonical `Step_F` LLVM 类型。
     - 必须满足：
       - 与 `StepSchema` 一一对应；
       - `Complete` variant 与每个 case variant 的身份稳定；
       - variant/tag 顺序与 `CaseTag` 对齐；
       - `payload_tuple_ty == ()` 与 `complete_ty == ()` 时允许零载荷物理表示；
       - `SingleCase(case_tag)` 不能生成第二套“窄版 Step 类型”，只能复用同一 canonical `Step_F`。
  3. 为 frame schema 物化 LLVM 布局。
     - 最低要求必须显式承载：
       - object header（若当前 GC object 模型要求）
       - state tag
       - resume payload carrier
       - cleanup flag
       - one-shot flag
       - completion tag
       - P5 user slots / system slots
     - frame field 顺序必须以 P5 schema 为 authoritative 输入；
     - 若 backend 需要额外 ABI-only 字段，必须显式加入稳定 mapping，且不能破坏 `FrameSlotId -> field index` 查询面。
  4. 为 continuation object 物化具体 LLVM 布局。
     - continuation object 的物理类型必须固定到 callable version；
     - 它必须显式承载：
       - 捕获的 frame/context 引用
       - 必要的 system fields
       - one-shot 状态
       - resume dispatch 所需的 method/vtable/interface identity
     - 允许复用现有通用 object/interface lowering helper；
     - 但不得借此回落到 legacy effect handler stack contract。
  5. 为 internal resume interfaces 物化 LLVM-level 方法签名与 dispatch 表示。
     - 每个 interface method 必须满足：
       - 参数类型 = 对应 case 的 `resume_tuple_ty`
       - 返回类型 = 同一 `Step_F<T>`
       - identity 稳定、可被 build fixtures / LLVM IR 断言引用
     - method 集必须完整；
     - 不可能合法调用到的方法允许 lowering 成 `unreachable` body，但不能从接口/vtable 形状上删掉。
  6. 为 canonical dynamic callable surface 物化 LLVM-level entry 签名。
     - 必须直接体现 `invoke(args_tuple) -> Step_F`；
     - direct/static path 允许调用已知 concrete entry；
     - 但它必须与 canonical dynamic surface 属于同一语义合同，不得形成第二套 effect-special ABI。
  7. 对 `Unit` / `()` 做 codegen 级退化处理。
     - `f()` 与 `f(())`、`k.resume()` 与 `k.resume(())` 必须共享无额外 `Unit` 载荷的实现路径；
     - `Unit` 参数、局部、返回值在 codegen 层不应被强制 materialize 成真实运行时值；
     - `Step_F` 零载荷 case / `Complete(())` 等情况必须按同一规则退化，而不是偷偷多保留一个“Unit payload object”。
  8. 对 refactor type/layout 层建立稳定查询 API。
     - 后续 body emitter 至少要能稳定查询：
       - 某个 callable version 的 `Step_F` LLVM 类型
       - 某个 `FrameSlotId` / system field 对应的 LLVM field index
       - 某个 continuation object / resume interface 的签名与 vtable/method identity
       - 某个 dynamic invoke entry / direct entry 的 LLVM function signature
     - 明确禁止：让下游 emitter 重新从 P5 schema 手工二次拼装这些布局信息。

- 必须遵从的约束：
  - 禁止把 refactor LLVM ABI 重新建模成 erased `Signal { tag, payload }` / `EffectOutcome` 结构。
  - 禁止把 `SingleCase` 理解为“换一个更小的 `Step` 类型”；它只能影响可达分支与 dispatch 复杂度。
  - 禁止在 type/layout materialization 中使用裸字符串 effect 名/FQN、`Any`、`Todo(...)` 作为最终 ABI identity。
  - 禁止为 `Unit` 人为保留物理载荷，只因为 surface 上写成了 `()`。
  - 禁止跨不同 `allowed_row` 家族错误共享 callable version / `Step_F` / continuation object LLVM identity。

- 验证：
  1. 新增/更新单元测试，推荐命名：
     - `refactor_llvm_step_layout_*`
     - `refactor_llvm_frame_layout_*`
     - `refactor_llvm_continuation_layout_*`
     - `refactor_llvm_unit_abi_*`
  2. 新增/更新 build fixtures，推荐至少包括：
     - `tests/fixtures/build/effect_refactor_step_enum_single_case.scoop`
       - 目标：锁定 canonical `Step_F` 仍保留完整 case/tag 身份，而非生成第二套窄类型
     - `tests/fixtures/build/effect_refactor_dynamic_invoke_unit_payload.scoop`
       - 目标：锁定 `invoke(args_tuple)` / `resume()` 的 `Unit` 零载荷 ABI
     - `tests/fixtures/build/effect_refactor_continuation_interface_full_methods.scoop`
       - 目标：锁定 continuation object 完整 method 集
     - 若已有合适的 run-pass/effect-lowered 源文件，允许复制源码并添加 `// ARGS: --emit-llvm` 与 LLVM 子串断言
  3. 运行：
     - `cargo test -p scoopc refactor_llvm_step_layout`
     - `cargo test -p scoopc refactor_llvm_frame_layout`
     - `cargo test -p scoopc refactor_llvm_continuation_layout`
     - `cargo test -p scoopc refactor_llvm_unit_abi`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_step_enum_single_case.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_invoke_unit_payload.scoop`
  4. 额外抽样 legacy 不受影响：
     - `cargo run -p scoop -- --effect-pipeline legacy test --fixtures tests/fixtures/build/effect_no_perform_no_handler_symbols_basic.scoop`

- 完成条件：
  - `Step_F` / frame / continuation object / resume interface / dynamic invoke 的 LLVM type/layout 合同已闭合；
  - 后续 P6-T03 可以只消费这层 LLVM-level query API 做 body lowering；
  - 新路径不再需要依赖 legacy `EffectSignal`/`EffectOutcome` 合同表达 effect ABI。
- 依赖：P6-T01b
- 完成记录：
  - 2026-05-03：已在 `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs` 建立独立的 refactor LLVM type/layout materialization 与稳定 query API，并把 refactor `build --emit-llvm` 的 module 构建路径接到该 ABI shell 物化层。
  - 已新增 `refactor_llvm_step_layout_*`、`refactor_llvm_frame_layout_*`、`refactor_llvm_continuation_layout_*`、`refactor_llvm_unit_abi_*` 定向单测，覆盖 canonical `Step_F` tag identity、frame/system slot field index、resume-interface 完整 method 集，以及 `Unit` 零载荷 ABI。
  - `crates/scoopc/src/llvm/emit.rs` 现已通过 `RefactorStageEmitInput` 把 authoritative reachable-body program 与 ABI-visibility program 显式分离：前者继续作为 fail-fast / 后续 body lowering 的 authoritative handoff；后者只负责发布 request-source 范围的 canonical `Step_F` / continuation / resume-interface / dynamic invoke ABI shell，避免 backend 回到 legacy `EffectSignal` / `EffectOutcome` 模型。
  - `P6-T01b` 已修复此前的 build-fixture blocker：`crates/scoop/src/commands/build.rs` 会为 refactor build 额外构造 `RequestSources` rooted 的 ABI visibility handoff，`crates/scoop/src/fixtures/mod.rs` 也会把 refactor session 选项透传给内部 build，因此三个 build fixtures 现都能在真实 refactor build 主路径上稳定观察 ABI shell，而不必通过 reachable handle 重新进入 legacy lowering。
- 已运行验证：
  - `cargo test -p scoopc refactor_llvm_`
  - `cargo test -p scoop refactor_build_`
  - `cargo test -p scoop build_fixtures_propagate_refactor_session_options_to_build_command`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_step_enum_single_case.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_invoke_unit_payload.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_continuation_interface_full_methods.scoop`
  - `cargo run -p scoop -- --effect-pipeline legacy test --fixtures tests/fixtures/build/effect_no_perform_no_handler_symbols_basic.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## [DONE] P6-T02a：让 refactor LLVM ABI materializer 严格消费 P5 发布的 resume-interface contract，禁止在 P6 现场补造 interface identity

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.2, §5.3.2-§5.3.4
  - `crates/scoopc/src/effect_lowered/ir.rs`
  - `crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs`
- 背景：
  - `P6-T02R` 审阅发现：`crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 当前虽会先读取 `LateLoweredProgram.resume_interfaces()`，但 `derive_resume_interface_specs()` 仍会按 `(step_schema, effect_family)` 重新补齐/合成缺失的 `ResumeInterfaceId`；这会把缺失的 interface 发布静默掩盖成 P6 现场重建逻辑。
  - 同一文件里的 `materialize_continuation_object_layout(...)` / `materialize_callable_layout(...)` 目前也没有把 `LateLoweredContinuationObject.implemented_interfaces()` / `LateLoweredCallable.resume_interfaces()` 当作 authoritative interface 集合与顺序来源，而是继续消费按 step-schema 汇总的派生列表。
  - 结果是：refactor LLVM ABI query 仍可能与 P5 late-lowered handoff 的真实 interface identity 漂移；后续 P6-T03 若直接消费这层 query，要么被迫再做 remap/重建，要么静默接受漂移后的 identity，二者都违背 P5 -> P6 handoff contract。
- 目标：
  - 让 refactor LLVM ABI materializer 严格消费 `LateLoweredProgram.resume_interfaces()`、`LateLoweredCallable.resume_interfaces()`、`LateLoweredContinuationObject.implemented_interfaces()` 作为 authoritative identity/order；
  - 对缺失、错配、或不完整的 interface 发布 fail fast，而不是在 P6 现场补造新的 interface identity。

- 必须实现的内容：
  1. 移除/收口 ABI materialization 中对缺失 `ResumeInterfaceId` 的现场合成逻辑。
     - `crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 不得再在缺失 `LateLoweredResumeInterface` 时发明新的 `ResumeInterfaceId`；
     - 若 P5 handoff 少了某个 interface family、method set、或 identity 映射，必须返回结构化错误。
  2. 让 callable / continuation object layout 严格消费 authoritative interface 列表。
     - callable layout 必须以 `LateLoweredCallable.resume_interfaces()` 为 interface identity 与顺序来源；
     - continuation object layout 必须以 `LateLoweredContinuationObject.implemented_interfaces()` 为 interface field 发布来源；
     - 明确禁止继续用按 `step_schema` 汇总的派生列表替代这两份 authoritative handoff。
  3. 让 resume interface layout 严格对齐 `LateLoweredResumeInterface.methods()`。
     - method identity、case tag、`resume_tuple_ty`、返回 `Step_F` schema 必须优先消费 late-lowered interface/method shell；
     - 若需要回看 `StepSchema.cases()`，也只能用于校验“method 集是否完整且与 effect family/case tag 一致”，不能在缺失时静默重建。
  4. 补充定向测试与回归。
     - 至少覆盖：ABI query 对 callable/object 发布的 interface id 保真；
     - 以及一个“故意删掉/错配 published resume interface”的构造路径会被 materializer 显式拒绝，而不是继续产出漂移后的 query。

- 必须遵从的约束：
  - 禁止把 `(step_schema, effect_family)` 派生出的临时键空间当作最终 interface identity。
  - 禁止让 P6 ABI materializer 越权修补 P5 handoff 缺口；该层只能消费并验证 authoritative contract。
  - 禁止为了让测试继续通过而绕开 `LateLoweredCallable.resume_interfaces()` / `LateLoweredContinuationObject.implemented_interfaces()`。

- 验证：
  - `cargo test -p scoopc refactor_llvm_continuation_layout`
  - `cargo test -p scoopc refactor_llvm_step_layout`
  - `cargo test -p scoopc refactor_llvm_unit_abi`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_step_enum_single_case.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_invoke_unit_payload.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_continuation_interface_full_methods.scoop`

- 完成条件：
  - refactor LLVM ABI query 不再在 P6 现场补造 resume-interface identity；
  - callable / continuation / resume-interface 三层 layout 已与 P5 late-lowered handoff 的 authoritative identity/order 对齐；
  - `P6-T02R` 可以据此继续审阅“ABI contract 已固定且后续 body emitter 不会再 remap/reconstruct interface identity”。
- 依赖：P6-T02
- 完成记录：
  - 2026-05-03：`crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 已删除按 `(step_schema, effect_family)` 在 P6 现场补造 `ResumeInterfaceId` 的逻辑；resume-interface layout 现直接消费 `LateLoweredProgram.resume_interfaces()` 与 `LateLoweredResumeInterface.methods()`，callable/continuation layout 也分别严格按 `LateLoweredCallable.resume_interfaces()` / `LateLoweredContinuationObject.implemented_interfaces()` 取 authoritative identity 与顺序。
  - 同一处 materializer 已补上结构化 fail-fast 校验：缺失 published interface、重复 interface id、callable/object identity 漂移、return-step 不匹配、以及 method contract 与 step shell 漂移都会直接返回前端错误，不再被 P6 现场重建逻辑掩盖。
  - 在验证过程中发现 `P6-T01b` 的 ABI-visibility handoff 还隐藏着一个 blocker：`crates/scoopc/src/effect_lowered/opt.rs` / `crates/scoopc/src/effect_refactor_pipeline/effect_lowering_stage.rs` 原先会把 authoritative reachable-body 用的后处理裁剪同样施加到 ABI-visibility program 上，导致 unreachable helper 所需的 published resume interface/method shell 被裁掉；现已为 ABI-visibility handoff 增加“保留 published resume shells”的 late-opt 模式，并只在 `crates/scoopc/src/effect_refactor_pipeline/llvm_codegen_stage.rs` 构造 `abi_visibility_effect_lowered_stage_output` 时启用，确保 build fixture 看到的 resume-interface / continuation ABI 与 P5 authoritative shell 一致，同时不改变 authoritative reachable-body program 的原有后处理收缩。
  - 已补充/更新 `refactor_llvm_continuation_layout_*` / `refactor_llvm_unit_abi_*` 定向单测，覆盖 authoritative interface 顺序、authoritative method 顺序、缺失 published interface 的 fail-fast，以及 `Unit` ABI 场景。
- 已运行验证：
  - `cargo test -p scoopc refactor_llvm_step_layout`
  - `cargo test -p scoopc refactor_llvm_continuation_layout`
  - `cargo test -p scoopc refactor_llvm_unit_abi`
  - `cargo test -p scoop refactor_build_publishes_request_source_abi_shells_for_unreachable_effectful_helpers`
  - `cargo test -p scoop refactor_build_rejects_reachable_self_contained_legacy_effect_body_lowering`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_step_enum_single_case.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_invoke_unit_payload.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_continuation_interface_full_methods.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## [DONE] P6-T02b：让 refactor LLVM ABI materializer 对 authoritative resume-interface method completeness fail fast，禁止接受缺失 method 的 published shell

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.2, §5.3.2-§5.3.4
  - `crates/scoopc/src/effect_lowered/materialize.rs`
  - `crates/scoopc/src/effect_lowered/ir.rs`
  - `crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs`
- 背景：
  - `P6-T02R` 复审发现：`crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 中 `materialize_resume_interface_layout(...)` 当前会校验每个已发布 method 的 case tag、effect family、`concrete_op_key`、continuation contract 与 `out_step_schema` 是否和 step shell 对齐；
  - 但它只把 `published_case_tags` 用于检查“重复发布”，没有把这组 case tag 与同一 `effect_family` 在 authoritative `LateLoweredStepType.cases()` 中应有的完整 case 集做最终比对；
  - 结果是：如果 `LateLoweredResumeInterface.methods()` 少发了某个 authoritative case，P6 仍会静默接受并物化缩水的 vtable / method 布局，而不是 fail fast；
  - 这违背了 `P6-T02` 对“resume interface method 集必须完整”的要求，也违背了 `P6-T02a` 对“P6 只能消费并验证 authoritative handoff，不能掩盖缺口”的要求。
- 目标：
  - 让 refactor LLVM ABI materializer 在 authoritative resume-interface shell 缺失 method 时返回结构化错误；
  - 同时保持“method 顺序仍由 `LateLoweredResumeInterface.methods()` authoritative 发布顺序决定”，只做校验，不做补造或重排。

- 必须实现的内容：
  1. 在 `materialize_resume_interface_layout(...)` 中加入 authoritative method completeness 校验。
     - 以当前 interface 的 `effect_family` + `return_step_schema` 为键，找出同一 authoritative `LateLoweredStepType` 中应由该 interface 覆盖的全部 case；
     - 将该期望 case 集与 `LateLoweredResumeInterface.methods()` 实际发布的 case 集做对比；
     - 若缺失任一 authoritative case，必须返回结构化错误并指出缺失的 case tag / interface id / step schema。
  2. 明确禁止在校验失败时现场补造 method shell。
     - 可以借助 `StepType` 回查期望 case 集做验证；
     - 但不能像旧的 interface-id 漂移问题那样，在 P6 现场把缺失 method 自动补回去。
  3. 保持 authoritative method 顺序不变。
     - 对合法输入，vtable index 仍必须严格跟随 `LateLoweredResumeInterface.methods()` 的发布顺序；
     - 校验逻辑不能把 method 集重新排序后再写回布局层。
  4. 补充定向测试。
     - 至少新增一个“故意从 authoritative resume interface 中删掉某个 method”的构造路径；
     - 断言 ABI materializer 会显式拒绝，而不是继续产出缺失 method 的 interface layout。

- 必须遵从的约束：
  - 禁止把“缺失 method 时继续接受缩小后的 vtable”当作合法 ABI 变体；
  - 禁止通过按 case tag 排序重写 `LateLoweredResumeInterface.methods()` 来掩盖缺口；
  - 禁止把缺失 method 的修补下放到后续 P6-T03 body emitter。

- 验证：
  - `cargo test -p scoopc refactor_llvm_continuation_layout`
  - `cargo test -p scoopc refactor_resume_interface_completeness_groups_methods_by_effect_family`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_continuation_interface_full_methods.scoop`

- 完成条件：
  - authoritative resume-interface shell 缺失 method 时，refactor LLVM ABI materializer 会 fail fast；
  - 对合法输入，resume-interface method 顺序与 vtable index 仍严格跟随 authoritative published order；
  - `P6-T02R` 可以据此继续确认 LLVM type/layout 合同已真正闭合。
- 依赖：P6-T02a
- 完成记录：
  - 2026-05-03：`crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 中的 `materialize_resume_interface_layout(...)` 现已在逐个校验已发布 method 之外，额外从 authoritative `LateLoweredStepType.cases()` 中按 `(return_step_schema, effect_family)` 收集应覆盖的完整 case 集，并与 `LateLoweredResumeInterface.methods()` 的已发布 case tag 做最终比对；若缺失任一 authoritative case，会以结构化前端错误显式报出 interface id、step schema、effect family 与缺失 case tag，而不再静默接受缩水的 vtable/layout。
  - 同一实现保持了 authoritative method 顺序不变：vtable index 仍严格按 `LateLoweredResumeInterface.methods()` 的发布顺序分配，新增逻辑只做 completeness 校验，不补造、不重排 method shell。
  - 已新增定向单测 `refactor_llvm_continuation_layout_rejects_missing_authoritative_method`，覆盖“故意删掉 authoritative resume method 时必须 fail fast”；同时更新 `refactor_llvm_continuation_layout_preserves_authoritative_interface_order` 的构造输入，先补齐完整 Ping method 集，再只验证 interface 发布顺序，避免旧测试继续依赖不完整 shell。
- 已运行验证：
  - `cargo test -p scoopc refactor_llvm_continuation_layout`
  - `cargo test -p scoopc refactor_resume_interface_completeness_groups_methods_by_effect_family`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_continuation_interface_full_methods.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## [DONE] P6-T02R：Review LLVM type/layout 合同，确认 canonical `Step_F`、frame、continuation ABI 已固定且不再依赖 legacy signal/outcome 模型

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.9, §5.2, §5.3.2-§5.3.6
  - [`PLAN.md`](./PLAN.md) §2/P6
- 重点：
  - `Step_F` LLVM 形状是否继续由 `StepSchema` 唯一决定；
  - `SingleCase` 是否仍保持 canonical `Step_F` 类型与 tag；
  - `Unit` ABI 是否已零载荷退化；
  - continuation object / resume interfaces 是否已成为显式可查询的 LLVM contract；
  - 新实现是否已经摆脱 legacy `EffectSignal` / `EffectOutcome` / `LegacyEffectBoundary` 作为 ABI 载体。
- 必须检查的文件/位置：
  - 新增的 `crates/scoopc/src/llvm/codegen/effect_refactor/types.rs`
  - 新增的 `crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs`
  - `crates/scoopc/src/llvm/codegen/types.rs`
  - `crates/scoopc/src/llvm/codegen/layout.rs`
  - `crates/scoopc/src/llvm/codegen/effect/contract.rs`

- 验证：
  - 重新运行 P6-T02 的全部测试与命令；
  - 额外搜索：
    - `rg "EffectSignal|EffectOutcome|LegacyEffectBoundary|Unit value|Signal \{|Todo\(" crates/scoopc/src/llvm/codegen/effect_refactor crates/scoopc/src/effect_refactor_pipeline crates/scoopc/src/llvm/codegen/effect`
  - 要求：
    - 允许命中：legacy 模块、测试、注释；
    - 不允许命中：refactor LLVM ABI 主实现继续以这些 legacy contract 作为最终模型。

- 完成条件：
  - review 能明确说明：refactor LLVM type/layout 合同已经固定，后续 body emitter 不会再回旧 contract 或 HIR 现场补 ABI；
  - 可进入 P6-T03。
- 依赖：P6-T02b
- 完成记录：
  - 2026-05-03：审阅发现 blocker。`crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 当前仍会在 `derive_resume_interface_specs()` 中按 `(step_schema, effect_family)` 补齐/合成缺失的 `ResumeInterfaceId`，并在 `materialize_continuation_object_layout(...)` / `materialize_callable_layout(...)` 中继续消费按 step-schema 汇总的派生 interface 列表，而不是严格使用 `LateLoweredContinuationObject.implemented_interfaces()` / `LateLoweredCallable.resume_interfaces()`。
  - 这意味着 refactor LLVM ABI query 仍可能掩盖 P5 handoff 漏发/错配的 resume-interface identity，后续 P6-T03 body emitter 若直接依赖该 query，仍会被迫 remap 或现场重建 interface contract，违背“P6 只消费 P5 authoritative handoff”的审阅目标。
  - 因此新增前置任务 `P6-T02a`，先收紧 ABI materializer 对 authoritative resume-interface contract 的消费边界；待该问题修复后再继续本 review。
  - 2026-05-03：重新运行 `cargo test -p scoopc refactor_llvm_`、`cargo test -p scoopc refactor_resume_interface_completeness_groups_methods_by_effect_family`、三个 refactor build fixtures，以及一个 legacy build fixture 抽样；现有矩阵通过，且 `crates/scoopc/src/llvm/codegen/effect_refactor/**` 中未发现 `EffectSignal` / `EffectOutcome` / `LegacyEffectBoundary` 等 legacy ABI 载体残留。
  - 同次复审发现新的 blocker：`materialize_resume_interface_layout(...)` 当前只用 `published_case_tags` 检查重复发布，却没有把 `LateLoweredResumeInterface.methods()` 与同一 `effect_family` 在 authoritative `LateLoweredStepType` 中应有的完整 case 集做比对；这意味着少发某个 authoritative method 时，P6 仍会静默接受缩水的 vtable / method 布局，违背“resume interface method 集必须完整且缺口必须 fail fast”的合同。
  - 因此新增前置任务 `P6-T02b`，先补齐 authoritative resume-interface method completeness 校验，再继续本 review。
  - 2026-05-03：`P6-T02b` 落地后重新执行 review，发现 `layout.rs` 里的 ABI 单测夹具仍把 authoritative reachable-body program 直接送入 ABI materializer，导致新增的 method completeness 校验把已裁剪 published shells 的默认测试路径判为失败；现已把测试夹具改为额外构造一份“保留 published resume shells”的 `abi_visibility_program`，让默认 ABI materialization 与真实 refactor LLVM stage 的 `abi_visibility_effect_lowered_stage_output` 保持一致，同时继续保留 authoritative handoff 供 fail-fast 负例测试使用。
  - 2026-05-03：复审确认 `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs` 已满足当前阶段合同：
    - `Step_F` 物理形状继续只由 `StepSchemaId` 决定，`SingleCase` 仅影响 `impl_plan`/可达分支，不会生成第二套窄 ABI；
    - frame/continuation/callable query API 已固定到 P5 发布的 schema、slot、resume-interface identity 与顺序；
    - `Unit` 参数、`Complete(())` 与 `resume(())` 继续以零载荷 ABI 退化；
    - refactor ABI materializer 不再依赖 `EffectSignal` / `EffectOutcome` / `LegacyEffectBoundary` 作为最终 ABI 载体。
  - 已运行验证：
    - `cargo test -p scoopc refactor_llvm_`
    - `cargo test -p scoopc refactor_resume_interface_completeness_groups_methods_by_effect_family`
    - `cargo test -p scoop refactor_build_`
    - `cargo test -p scoop build_fixtures_propagate_refactor_session_options_to_build_command`
    - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_step_enum_single_case.scoop`
    - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_invoke_unit_payload.scoop`
    - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_continuation_interface_full_methods.scoop`
  - `cargo run -p scoop -- --effect-pipeline legacy test --fixtures tests/fixtures/build/effect_no_perform_no_handler_symbols_basic.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
  - `rg "EffectSignal|EffectOutcome|LegacyEffectBoundary|Unit value|Signal \{|Todo\(" crates/scoopc/src/llvm/codegen/effect_refactor crates/scoopc/src/effect_refactor_pipeline crates/scoopc/src/llvm/codegen/effect`

## [DONE] P6-T02c：发布 continuation surface-resume ABI/query contract，禁止 P6-T03 在 backend 现场猜测 `resume(...)` 入口

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.2, §5.3.2-§5.3.6, §5.5
  - `crates/scoopc/src/effect_facts/facts.rs`
  - `crates/scoopc/src/effect_lowered/ir.rs`
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs`
- 背景：
  - 2026-05-03 开始执行 `P6-T03` 时发现：`ResumeSiteEffectFacts` 当前只发布了 `continuation_schema`、`resume_tuple_ty`、`answer_ty`、`out_step_schema`、`resolved_cases` 等语义信息；
  - 但 `P6-T02` 现有的 refactor LLVM ABI query 只物化了 `Step_F`、frame、continuation object、internal resume interfaces、以及 direct/dynamic callable entry，没有发布 `Continuation.resume(...)` surface lowering 所需的稳定 LLVM call target / query 映射；
  - `RefactorAbiQuery` 也没有提供 `ContinuationSchemaId`（或等价 authoritative surface key）到具体 resume lowering layout 的查询面；
  - 这意味着 `P6-T03` 若直接继续实现 `CallKind::Resume`，就会被迫在 backend 现场重新推导/猜测 `resume` 入口，或者偷走 legacy resume lowering 的合同，违背 P5 -> P6 handoff 边界。

- 目标：
  - 在进入 `P6-T03` 之前，先把 continuation surface `resume(args_tuple) -> Step_F` 的 authoritative ABI/query contract 明确发布到 refactor LLVM handoff 中；
  - 让后续 body emitter 能仅凭 P5/P6 发布的结构化 contract，把 `ResumeSiteEffectFacts` 解析到具体 LLVM-level lowering，而不再发明第二套 resume ABI。

- 必须实现的内容：
  1. 为 continuation surface `resume(...)` 建立显式的 refactor LLVM ABI/query 物化层。
     - 可新增 `RefactorContinuationResumeLayout` 或等价结构；
     - authoritative key 必须来自 `ContinuationSchemaId` 或等价的 compiler-owned stable identity；
     - 明确禁止让 `P6-T03` 通过成员名 `resume`、HIR 名字、或 legacy resume helper 反推 ABI。
  2. 明确发布 surface `resume(args_tuple) -> Step_F` 的 LLVM 函数签名 / call target contract。
     - 必须与 `ContinuationSchema.resume_tuple_ty` / `answer_ty` / `out_step_schema` 对齐；
     - `Unit` / `()` 的零载荷退化规则必须与 `P6-T02` 已固定的 ABI 规则保持一致；
     - 禁止把 surface `resume` 静默折叠成“随便挑一个 internal resume-interface method”而不显式记录 contract。
  3. 为 continuation object 发布 authoritative 的 surface-resume 映射。
     - body emitter 必须能从 `ResumeSiteEffectFacts` 的 authoritative contract 直接解析到：
       - 该 site 应调用的 surface-resume layout / symbol identity；
       - 与之对应的返回 `Step_F` schema；
       - 若需要继续进入 internal dispatch，所需的 compiler-owned mapping。
     - 明确禁止在 `P6-T03` 现场靠扫描 raw late-lowered 列表临时拼一个未发布的选择规则。
  4. 对缺失/错配的 surface-resume contract fail fast。
     - 当 `ResumeSiteEffectFacts` 引用的 continuation schema 没有已发布的 refactor LLVM surface-resume layout，或与 continuation object / step schema 漂移时，必须返回结构化错误。
  5. 补充定向测试与回归。
     - 至少覆盖：
       - refactor ABI query 能从 authoritative continuation schema 解析到 surface-resume layout；
       - 一个包含 `k.resume(...)` 的 fixture（如 `tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`）不会再要求 backend 现场猜测 resume 入口；
       - 缺失 published surface-resume contract 时会显式拒绝。

- 必须遵从的约束：
  - 禁止把 `Continuation.resume(...)` lowering 继续藏在 legacy `call/resume.rs` / handler-stack contract 中；
  - 禁止新增只在测试里可见的 surface-resume helper；
  - 禁止让 `P6-T03` 通过 HIR/名字启发式把 `ResumeSiteEffectFacts` 临时映射到某个 LLVM call target。

- 验证：
  - `cargo test -p scoopc refactor_llvm_continuation_layout`
  - `cargo test -p scoopc refactor_llvm_surface_resume_layout`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`

- 完成条件：
  - continuation surface `resume(...)` 已拥有 authoritative 的 refactor LLVM ABI/query contract；
  - `P6-T03` 可以仅消费已发布 handoff 来 lower `CallKind::Resume`，不再需要 backend 现场猜测或重建 resume 入口；
  - 若 contract 漏发或漂移，refactor LLVM path 会在 stage/codegen 边界显式拒绝。
- 依赖：P6-T02R
- 完成记录：
  - 2026-05-03：`crates/scoopc/src/llvm/codegen/effect_refactor/types.rs` 已新增 `RefactorContinuationSurfaceResumeLayout`，并在 `RefactorAbiQuery` 上发布 `ContinuationSchemaId -> surface-resume layout` 查询面；该 layout 显式固定了 source-visible `Continuation.resume(args_tuple) -> Step_F` 的 symbol identity、函数签名、`resume_tuple_ty` / `answer_ty` / `out_step_schema`，不再要求后续 body emitter 通过成员名 `resume` 或 legacy helper 反推 ABI。
  - 同一轮实现中，`crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 现会同时消费两类 authoritative P5 handoff 来注册 surface-resume contract：
    - continuation object 已发布的 `surface_resumes()`；
    - owner callable `Resume` boundary lowering 里的 `ResumeSiteEffectFacts`。
    这样就能覆盖像 `tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop` 这类“resume site 的 `continuation_schema/out_step_schema` 比 owner callable outward `StepSchema` 更窄”的真实路径，避免 `P6-T03` 再在 backend 现场猜测 `k.resume(...)` 的 lowering 入口。
  - continuation object layout 现已额外发布 per-object 的 surface-resume schema 绑定；该绑定既覆盖 owner step-case continuation schema，也覆盖 owner callable 内部真实出现的 `ResumeSiteEffectFacts` continuation schema。若对象少发 schema、resume site 引用了未发布 layout、或多处 authoritative source 对同一 `ContinuationSchemaId` 给出了漂移的 `resume_tuple_ty` / `answer_ty` / `out_step_schema`，ABI materializer 会在 stage 边界 fail fast。
  - 已新增 `refactor_llvm_surface_resume_layout_*` 单测，覆盖：
    - ABI query 可从真实 `k.resume(...)` fixture 的 authoritative `ContinuationSchemaId` 解析到 surface-resume layout；
    - continuation object 可同步发布该 schema 的 object-level binding；
    - 缺失 published surface-resume contract 时显式拒绝；
    - `Unit` payload 的 source-visible `resume()` ABI 与 P6-T02 已固定的零载荷规则保持一致。
- 已运行验证：
  - `cargo test -p scoopc refactor_llvm_surface_resume_layout`
  - `cargo test -p scoopc refactor_llvm_continuation_layout`
  - `cargo test -p scoopc refactor_llvm_unit_abi`
  - `cargo test -p scoopc refactor_llvm_`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## [DONE] P6-T02d：发布 canonical dynamic-invoke callable-object ABI/query contract，禁止 P6-T03 在 backend 现场猜测 indirect call 入口

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.9-§4.13, §5.5.1-§5.5.4, §8
  - `crates/scoopc/src/effect_lowered/ir.rs`
  - `crates/scoopc/src/effect_lowered/materialize.rs`
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs`
  - `crates/scoopc/src/llvm/codegen/{mir_body.rs,closure/mod.rs,call/dispatch.rs}`（只能作现有 runtime callable / closure / dispatch 形状对照，不能直接借壳 legacy effect ABI）
- 背景：
  - `P5-T05/P5-T07` 已把 call boundary 的语言级 contract 固定到 `LateLoweredCallBoundaryLowering`：site facts 会发布 `CallSiteTarget::{KnownInstance, CandidateSet, DynamicFallback}`、`invoke_args_tuple_ty`、callee `StepSchema`、以及 boundary dispatch plan。
  - `EFFECT_REFACTOR.md` 已明确要求：dynamic boundary 不是第二套 effect-special transport，而是 compiler-owned callable object / closure-like carrier 上的 canonical `invoke(args_tuple) -> Step_F` ordinary indirect call。
  - 但当前 `crates/scoopc/src/llvm/codegen/effect_refactor/types.rs` / `layout.rs` 只会按 callable version 发布 static `dynamic_entry` / `direct_entry` symbol 与签名；`RefactorAbiQuery` 也只支持按 `StepSchemaId` / root FQN 查询这些 entry。
  - 这意味着 `P6-T03` 一旦开始 lower `CallTargetMode::CandidateSet` / `DynamicFallback`，就没有 authoritative LLVM-level query 可以把“runtime callee value / callable object”映射到 canonical dynamic `invoke` surface。若继续实现，backend 只能：
    - 回 `CallKind::{Closure, FunValue, Virtual, Interface}` / HIR / 旧 callable wrapper 现场重建 ABI；或
    - 错误地把任务范围缩窄成只支持 `KnownInstance` direct call。
  - 两种做法都违背了 P5 -> P6 handoff contract 与本阶段“不得在 backend 现场猜语义/ABI”的约束，因此必须先补齐这层 query contract。

- 目标：
  - 为 refactor LLVM body emitter 发布一层 authoritative 的 canonical dynamic-invoke callable-object ABI/query contract；
  - 让 `P6-T03` 可以只消费已发布 handoff 来 lower effectful `Call` boundary（含 `KnownInstance`、`CandidateSet`、`DynamicFallback`），而不回 legacy callable/effect dispatch 现场补造动态入口协议。

- 必须实现的内容：
  1. 扩展 refactor LLVM ABI handoff，显式发布 runtime callable value 的 canonical dynamic-invoke contract。
     - 至少要覆盖：
       - dynamic/candidate-set call target 需要的 callable carrier 形状（closure-like env/callee identity、或等价稳定 representation）；
       - `invoke(args_tuple) -> Step_F` 的 LLVM-level call signature；
       - backend 如何从 authoritative handoff 上取得该 surface，而不是再回旧 call wrapper / dispatch helper 猜测。
     - 若现有 `LateLoweredProgram` 仍缺少发布这层 contract 所必需的稳定字段，允许最小化扩展 P5/P6 handoff；但扩展后的 contract 必须成为 authoritative 输入，不能只在 P6 局部缓存一份临时规则。
  2. 把 `CallSiteTarget` / `CallTargetMode` 与新的 ABI query 接通。
     - `KnownInstance`：继续允许 lower 到已发布 callable concrete entry；
     - `CandidateSet` / `DynamicFallback`：必须通过新的 canonical dynamic-invoke query 取得 ordinary icall/interface-call 所需合同；
     - 明确禁止让 `P6-T03` 通过 HIR 名字、旧 closure wrapper、或 legacy effect call ABI 反推出动态入口。
  3. 对缺失/漂移 contract fail fast。
     - 若某个 call boundary 需要 dynamic invoke contract，但 handoff 没有发布对应 callable carrier / invoke query；
     - 或 query 与 `invoke_args_tuple_ty` / callee `StepSchema` / target mode 漂移；
     - 必须返回结构化错误，不能静默回落到 legacy call/effect lowering。
  4. 补充定向测试与 build fixture。
     - 至少覆盖：
       - ABI query 可从 authoritative handoff 查询到 dynamic callable invoke contract；
       - `CandidateSet` / `DynamicFallback` 缺少 contract 时显式拒绝；
       - `Unit` payload 的 dynamic invoke ABI 仍遵守 `invoke(args_tuple)` 零载荷退化规则。

- 必须遵从的约束：
  - 禁止把 dynamic invoke 再建模回 legacy hidden resume token / `EffectOutcome` / handler-stack call ABI。
  - 禁止把 `CallTargetMode::CandidateSet` / `DynamicFallback` 缩窄成“暂只支持 `KnownInstance`”。
  - 禁止在 body emitter 中重新根据 `CallKind` / HIR / `Span` / wrapper 名字发明 callable-object layout。

- 验证：
  1. 新增/更新单元测试，推荐命名：
     - `refactor_llvm_dynamic_invoke_query_*`
     - `refactor_llvm_callable_carrier_layout_*`
  2. 新增/更新 build fixtures，推荐至少包括：
     - `tests/fixtures/build/effect_refactor_dynamic_invoke_candidate_set_emit_llvm.scoop`
       - 目标：锁定 candidate-set / dynamic invoke ABI query 已发布
     - `tests/fixtures/build/effect_refactor_dynamic_invoke_unit_payload.scoop`
       - 目标：锁定 dynamic invoke `Unit` 零载荷 ABI
  3. 运行：
     - `cargo test -p scoopc refactor_llvm_dynamic_invoke_query`
     - `cargo test -p scoopc refactor_llvm_callable_carrier_layout`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_invoke_unit_payload.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`

- 完成条件：
  - refactor LLVM handoff 已显式发布 canonical dynamic callable invoke contract；
  - `P6-T03` 可以只消费 query 就 lower effectful indirect/candidate-set call boundary；
  - backend 不再需要借壳 legacy callable/effect ABI 或把范围缩到 direct known call。
- 依赖：P6-T02c, P5-T07a
- 完成记录：
  - 2026-05-03：`crates/scoopc/src/llvm/codegen/effect_refactor/types.rs` 新增了按 call boundary 发布的 dynamic invoke query 面：
    - `RefactorDynamicInvokeLayout` 记录 `invoke(receiver, args_tuple) -> Step_F` 的 LLVM surface；
    - `RefactorDynamicInvokeCarrierLayout` 区分 closure carrier 与 virtual/interface receiver-dispatch carrier；
    - `RefactorCallTargetQuery` 把 `KnownInstance` / `CandidateSet` / `DynamicFallback` 收口到同一 ABI query API；
    - 现有 `RefactorCallableEntryLayout` 也开始记录 `invoke_args_tuple_ty`，便于 query 对 drift 做 fail-fast 校验。
  - `crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 现已基于 authoritative handoff 发布 dynamic invoke contract：
    - 直接消费 `LateLoweredBoundaryLowering::Call` 的 `CallSiteEffectFacts`；
    - 通过 ABI visibility 对应的 canonical MIR `CallKind` 发布 closure / virtual receiver / interface receiver 的 carrier 形状；
    - 对缺失 call-site metadata、缺失 CandidateSet published shell、以及 `target_mode` / `invoke_args_tuple_ty` / `callee_step_schema` 漂移一律结构化拒绝。
  - `crates/scoopc/src/llvm/emit.rs` 已把 ABI visibility `MaterializedMirPassView` 一起接到 ABI materializer，修复了 pure `main` build fixture 下“ABI visibility program 已发布 helper shell，但 ABI query 仍错误读取 reachable-body pass-view”导致 helper dynamic-invoke contract 丢失的问题。
  - 已新增 build fixture `tests/fixtures/build/effect_refactor_dynamic_invoke_candidate_set_emit_llvm.scoop`，用 pure `main` + unreachable effectful virtual helper 锁定 CandidateSet dynamic invoke shell 的真实 refactor build 可见性；现有 `effect_refactor_dynamic_invoke_unit_payload.scoop` 继续覆盖 `Unit` 零载荷退化。
  - 已新增/更新 `refactor_llvm_call_target_query_*` / `refactor_llvm_dynamic_invoke_query_*` / `refactor_llvm_callable_carrier_layout_*` 单测，覆盖：
    - `KnownInstance` 继续回查 published callable entry；
    - `DynamicFallback` fun-value 调用的 closure carrier 与 `Unit` 零载荷 ABI；
    - `CandidateSet` virtual dispatch 的 receiver carrier contract；
    - 缺失 authoritative dynamic-invoke contract 时的显式拒绝。
- 已运行验证：
  - `cargo test -p scoopc refactor_llvm_call_target_query`
  - `cargo test -p scoopc refactor_llvm_dynamic_invoke_query`
  - `cargo test -p scoopc refactor_llvm_callable_carrier_layout`
  - `cargo test -p scoopc refactor_llvm_unit_abi`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_invoke_candidate_set_emit_llvm.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_invoke_unit_payload.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## [DONE] P6-T02e：发布 pure caller call boundary 本地消费 compiler-generated runtime-error case 的 lowering contract，禁止 P6-T03 在 backend 现场发明传播路径

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.9, §5.5.3-§5.5.7, §8
  - [`TODO-P5.md`](./TODO-P5.md) 中 `P5-T07a`
  - `crates/scoopc/src/effect_lowered/{ir,materialize,dump,opt}.rs`
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs`
- 背景：
  - `P5-T07a` 为 pure caller 新增了 `LateLoweredCallBoundaryLowering::consumed_runtime_error_case`，用来表达“callee 的 compiler-generated one-shot `Raise<RuntimeError>` case 不应被强行投影回 caller outward `StepSchema`”；
  - 但当前 authoritative handoff 只发布了 `input_case_tag` / `concrete_op_key` / `payload_tuple_ty`，并没有发布 backend 可直接 lower 的控制流/边界合同：
    - caller `state_graph` 里没有对应的 synthetic state / boundary；
    - `boundary_map` / `resume_state_map` 没有说明该 case 在 caller 内部应该如何继续传播；
    - `RefactorAbiQuery` 也没有为这条“本地消费的 ordinary runtime-error”提供可执行 query。
  - 因此若直接继续 `P6-T03`，backend 只能自行猜测“是要重新构造本地 runtime-error boundary、改写为 hidden trap、还是临时扩 surface row”；这违背了本阶段“只翻译已发布 handoff，禁止现场发明第二套传播路径”的约束。

- 目标：
  - 为 pure caller call boundary 补齐一个 authoritative、可 lower 的 local-runtime-error consumption contract；
  - 让 `P6-T03` 可以仅消费这份合同，就正确 lower `consumed_runtime_error_case`，而不必在 backend 现场发明新的控制流或隐藏通道。

- 必须实现的内容：
  1. 为 `consumed_runtime_error_case` 发布显式 lowering contract。
     - 允许方案：
       - 在 late-lowered state graph / boundary map 中新增 dedicated synthetic boundary 或 state；
       - 或新增等价的结构化 lowering 节点，明确给出该 case 的 caller-local 传播方式、目标控制流与 payload/step contract。
     - 但禁止继续只发布“是哪条 case”而把实际行为留给 P6 backend 自己补想象。
  2. 明确 pure caller 的 local consume 语义边界。
     - 必须同时满足：
       - 不把 compiler-generated runtime-error case 反写回 caller `surface_ty` / outward `StepSchema`；
       - 不把该路径降级成 hidden trap / outcome side channel；
       - backend 能根据 handoff 明确知道：这条 ordinary runtime-error 在 caller 内部如何继续传播/结束。
  3. 把新 contract 接入稳定 dump/query 面。
     - `dump-effect-lowered` 必须能公开该 local consume contract；
     - 若 contract 缺失、漂移、或 body/state graph 没有为其发布必要 lowering 信息，必须在 P5/P6 边界 fail fast。
  4. 补充定向回归。
     - 至少覆盖：
       - `tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop` 中 pure caller `main -> run(...)` 的 call boundary 不再只是“记住 case id”，而是拥有 backend 可执行的 local runtime-error contract；
       - 缺失该 contract 时会显式拒绝，而不是把责任留给 `P6-T03` 现场猜测。

- 必须遵从的约束：
  - 禁止把 `consumed_runtime_error_case` 当成“LLVM backend 自己知道怎么处理”的非正式约定。
  - 禁止为 pure caller 新增隐藏 trap/abort 通道来掩盖缺失 handoff。
  - 禁止借机扩大 caller 的源码层 callable surface 或 residual row。

- 验证：
  - `cargo test -p scoopc refactor_boundary_lowering`
  - `cargo test -p scoopc refactor_effect_lowered_stage`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

- 完成条件：
  - pure caller call boundary 对 compiler-generated ordinary runtime-error case 的本地消费路径已经成为 authoritative、可 lower 的 handoff；
  - `P6-T03` 可以仅消费已发布 contract 来实现 call boundary lowering，而不再需要 backend 现场发明 pure caller 的 runtime-error 传播路径。
- 依赖：`P6-T02d`, `P5-T07a`
- 完成记录：
  - 2026-05-03：完成 `P6-T02e`。在 `crates/scoopc/src/effect_lowered/{ir,builder,materialize,dump,mod,opt}.rs` 中把 `LateLoweredCallBoundaryLowering::consumed_runtime_error_case` 从“只记录输入 case 身份”升级为结构化 lowering contract：该 contract 现在显式携带 `target_state`，并由 materializer 为每个 pure caller call boundary 追加 dedicated synthetic `LocalRuntimeError` terminal state；对应 owner `Suspend` state 也会把该 target 记入新的 `local_runtime_error_states` 后继集合，避免 P6-T03 继续在 backend 现场补想象 caller-local 控制流。
  - 2026-05-03：`dump-effect-lowered` 已同步公开这条 contract：pure caller `main` 的 call boundary 现在不再只打印 `consumed_runtime_error_case` 的 case id，而会同时公开 `target=st*`；state graph 里也新增显式的 `LocalRuntimeError(payload_tuple_ty=...)` synthetic state，说明这条 ordinary runtime-error 在 caller 内部的结束路径已经成为 authoritative handoff，而不是 hidden trap 约定。
  - 2026-05-03：在 `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs` 中新增 refactor LLVM ABI query 的 local runtime-error contract 发布面。backend 后续可通过 owner step schema + call site 稳定回查 payload ABI / input case / target state；若 target state 缺失、terminator 不是 `LocalRuntimeError`、或 payload 漂移，ABI materializer 会显式 fail fast，防止 `P6-T03` 在 body lowering 现场重新发明 pure caller runtime-error 传播路径。
  - 2026-05-03：已补充定向测试与 dump/query 断言：
    - `refactor_boundary_lowering_keeps_local_runtime_error_contract_for_pure_caller_calls` 现断言 target state 与 owner `Suspend` successor 同时存在；
    - `refactor_effect_lowered_stage_dump_exposes_local_runtime_error_call_contract` 锁定 stable dump 文本；
    - `refactor_llvm_local_runtime_error_contract_resolves_pure_call_boundary_targets` / `...rejects_missing_target_state` 锁定 ABI query 发布与 fail-fast 行为。
  - 2026-05-03：已运行验证：`cargo test -p scoopc refactor_boundary_lowering`、`cargo test -p scoopc refactor_effect_lowered_stage`、`cargo test -p scoopc refactor_llvm_local_runtime_error_contract`、`cargo run -q -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`、`cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`。`PLAN.md` 无需改动。

## [DONE] P6-T02f：发布 straight-line source-slice 非 boundary dynamic call 的 callable-object ABI/query contract，禁止 P6-T03 在 body emitter 现场回落旧 callable wrapper

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.9-§4.13, §5.5.1-§5.5.7, §8
  - `crates/scoopc/src/effect_lowered/ir.rs`
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs`
  - `crates/scoopc/src/llvm/codegen/mir_body.rs`
  - `crates/scoopc/src/mir/pass_view.rs`
- 背景：
  - `P6-T02d` 已为 call boundary 发布 authoritative 的 canonical dynamic-invoke contract，但当前实现范围仍只覆盖 `boundary_map` 里的 `LateLoweredBoundaryLowering::Call`。
  - 具体来说，`crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 当前的 `materialize_dynamic_invoke_layouts(...)` 只扫描 `callable.boundary_map().entries()`；`RefactorAbiQuery::call_target_layout(...)` 也只消费这批已发布 query。
  - 但 `P6-T03` 不是只 lower boundary terminator；它必须 lower 整个 state graph 的 straight-line `source_slices`。这些 slices 里仍可能出现**非 boundary** 的 `Rvalue::Call`，且其 `CallKind` 可能是 `Closure` / `FunValue` / `Virtual` / `Interface`。
  - 若不先补齐这层 authoritative handoff，`P6-T03` 遇到这类 source-slice call 只能：
    - 回落到 legacy closure/wrapper/dispatch ABI；或
    - 在 backend 现场临时发明第二套 callable-object invoke 规则；或
    - 错误地把 whole-body lowering 缩窄成“只支持 boundary call 上的 dynamic invoke”。
  - 这三种做法都违背当前阶段 contract-first 约束，因此必须先把 non-boundary source-slice dynamic call 也纳入已发布 LLVM query。

- 目标：
  - 为 refactor LLVM body emitter 发布一层 authoritative 的“whole-body source-slice dynamic call” ABI/query contract；
  - 让 `P6-T03` 在 lower `LateLoweredState.source_slices()` 时，可以只消费 handoff 就完成 non-boundary `Closure` / `FunValue` / `Virtual` / `Interface` call lowering，而不回 legacy callable wrapper 或现场补造 ABI。

- 必须实现的内容：
  1. 扩展 refactor LLVM ABI handoff，覆盖 non-boundary source-slice dynamic call site。
     - 至少要能为 canonical MIR 中仍留在 straight-line state slice 里的 `CallKind::{Closure, FunValue, Virtual, Interface}` 发布 stable query；
     - 这层 query 可以继续按 `owner step schema + site_id` 稳定索引，或使用等价但同样 authoritative 的 key；
     - 但禁止只在 P6 body emitter 里临时缓存一份未发布规则。
  2. 明确 non-boundary dynamic call 的 callable-object carrier / invoke surface。
     - 必须继续对齐 canonical `invoke(args_tuple) -> Step_F` contract；
     - 允许复用 `P6-T02d` 已有的 carrier 形状与 drift 校验逻辑；
     - 但必须把发布范围从“boundary-only”扩到 `P6-T03` whole-body source-slice 真正可见的动态调用点。
  3. 对缺失或漂移 contract fail fast。
     - 若某个 source-slice dynamic call 仍无 authoritative query；
     - 或 query 与 `CallKind` / carrier shape / `invoke_args_tuple_ty` / return `StepSchema` 漂移；
     - 必须在 P5/P6 边界显式拒绝，而不是把责任留给 `P6-T03` 现场猜测。
  4. 补充定向测试与 fixture。
     - 至少覆盖一个 `NoOutward` 或 straight-line state slice 中包含 non-boundary dynamic call 的样例；
     - 证明 ABI query 可以在不依赖 `boundary_map` Call entries 的前提下，仍稳定发布对应 callable-object invoke contract。

- 必须遵从的约束：
  - 禁止让 `P6-T03` 继续借壳 legacy closure object fn-ptr ABI、旧 dispatch helper、或 wrapper 名字约定来 lower non-boundary dynamic call。
  - 禁止把“先只支持 boundary dynamic invoke，straight-line dynamic call 后面再说”当作当前任务的完成标准。
  - 禁止把 site contract 重新退回到 HIR/`Span`/源码名字启发式。

- 验证：
  - `cargo test -p scoopc refactor_llvm_dynamic_invoke_query`
  - `cargo test -p scoopc refactor_llvm_callable_carrier_layout`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/build/effect_refactor_non_boundary_dynamic_call_emit_llvm.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

- 完成条件：
  - refactor LLVM handoff 已为 straight-line source-slice non-boundary dynamic call 发布 authoritative callable-object invoke contract；
  - `P6-T03` 可以在 whole-body lowering 中只消费已发布 query，而不再需要现场发明 dynamic call ABI 或回落 legacy wrapper。
- 依赖：`P6-T02e`
- 完成记录：
  - 2026-05-03：在 `crates/scoopc/src/llvm/emit.rs` 给 refactor ABI materializer 补齐了 ABI visibility `MaterializedEffectFacts` handoff；P6 的 ABI query 现在不只拿到 late-lowered program / types / canonical MIR pass-view，还能对 source-slice call site 回查 authoritative `BodyEffectFacts`。
  - 2026-05-03：在 `crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 中把 `materialize_dynamic_invoke_layouts(...)` 从“只扫描 `boundary_map` 的 call boundary”扩展为“双通道发布”：
    - 继续为 boundary call site 发布已有 dynamic invoke contract；
    - 额外扫描 `LateLoweredState.source_slices()` 指向的 canonical MIR statement slice，找出未进入 `boundary_map`、但 `CallKind::{Closure, FunValue, Virtual, Interface}` 且 `target_mode != KnownInstance` 的 non-boundary dynamic call；对这些 site 结合 published `BodyEffectFacts` 与 canonical MIR call kind 发布同样的 `(owner_step_schema, site_id)` query。
  - 2026-05-03：为 whole-body source-slice handoff 新增 fail-fast 校验：
    - 若 source slice 指向越界 block/statement range；
    - 若 dynamic call site 缺少 published `BodyEffectFacts`；
    - 若 dynamic call 的 effect facts kind 与 canonical MIR `CallKind` 漂移；
    - 若 non-boundary source-slice dynamic call 仍暴露 outward cases 却没有对应 call boundary；
    都会在 ABI materialization 阶段显式拒绝，而不会把责任留给 `P6-T03` body emitter 现场猜测。
  - 2026-05-03：新增 `tests/fixtures/build/effect_refactor_non_boundary_dynamic_call_emit_llvm.scoop`，用 pure `helper(base: Base)` 中的 `base.ping()` 锁定“source-slice non-boundary virtual CandidateSet call 也必须发布 dynamic invoke contract”。
  - 2026-05-03：新增/更新定向测试：
    - `refactor_llvm_dynamic_invoke_query_resolves_non_boundary_virtual_contract`
    - `refactor_llvm_callable_carrier_layout_resolves_non_boundary_virtual_contracts`
    它们共同验证：non-boundary source-slice dynamic call 不经过 `boundary_map` 也能稳定回查 query，且 carrier 仍是 canonical virtual receiver dispatch layout，而不是 legacy wrapper。
- 已运行验证：
  - `cargo test -p scoopc refactor_llvm_dynamic_invoke_query`
  - `cargo test -p scoopc refactor_llvm_callable_carrier_layout`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/build/effect_refactor_non_boundary_dynamic_call_emit_llvm.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_non_boundary_dynamic_call_emit_llvm.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## [DONE] P6-T02g：发布 callable carrier -> canonical dynamic entry 的 refactor contract，确保 closure/vtable/itable 不再指向 legacy 调用 ABI

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.9, §4.16, §5.2, §5.3.2-§5.3.6, §5.5.1-§5.5.7, §8
  - `crates/scoopc/src/llvm/codegen/{closure/mod.rs,mir_body.rs,gc.rs}`
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs`
  - `crates/scoopc/src/llvm/codegen/call/dispatch.rs`（只能作 legacy 对照）
- 背景：
  - `P6-T02d` / `P6-T02f` 已经为 boundary 与 source-slice dynamic call 发布了 authoritative 的 callable-object ABI/query：body emitter 现在可以按 `(owner_step_schema, site_id)` 回查 carrier 形状、`invoke_args_tuple_ty`、以及 canonical `invoke(args_tuple) -> Step_F` 返回合同。
  - 但当前 runtime carrier 本身还没有切到这套 contract：
    - `crates/scoopc/src/llvm/codegen/mir_body.rs` 与 `crates/scoopc/src/llvm/codegen/closure/mod.rs` 仍把 closure object 的 `fn_ptr` 直接写成普通 lambda/top-level LLVM function 指针；
    - `crates/scoopc/src/llvm/codegen/gc.rs` 仍用 `declare_top_level_fun(...)` 的普通函数符号去填 class vtable / interface itable method 槽位；
    - 仓库中还没有任何地方把这些 carrier 稳定绑定到 `RefactorCallableLayout.dynamic_entry()` 或等价的已发布 refactor dynamic entry。
  - 这意味着 `P6-T03` 即使拿到了 query，也仍无法只靠 runtime carrier + published contract 去 lower `CallKind::{Closure, FunValue, Virtual, Interface}`：
    - 要么被迫把 legacy 普通函数指针重新解释成 refactor `invoke(args_tuple) -> Step_F` target；
    - 要么在 backend 现场按名字/符号关系把普通 ABI remap 到 refactor entry；
    - 要么继续借壳旧 closure wrapper / vtable / itable dispatch 主线。
  - 这三条路都违背 `P6-T02d` / `P6-T02f` 的 contract-first 目标，因此在继续 `P6-T03` 之前，必须先把“runtime callable carrier 究竟指向哪个 canonical refactor entry”明确发布并真正接入 carrier materialization。

- 目标：
  - 为 refactor LLVM 新路径补齐 authoritative 的 callable carrier target-entry contract；
  - 让 closure object、top-level function value、class vtable、interface itable 在 refactor 路径上都稳定指向 canonical published dynamic entry（或等价且同样已发布的 refactor invoke target），使 `P6-T03` 可以只消费 handoff lower dynamic call，而不再借壳 legacy callable ABI。

- 必须实现的内容：
  1. 为 runtime callable carrier 发布 authoritative 的 target-entry 绑定。
     - 至少覆盖：
       - MIR/source-slice `MakeClosure` 生成的 closure object；
       - HIR closure value / top-level function value 对应的 closure-like callable object；
       - class vtable method 槽位；
       - interface itable method 槽位。
     - 这些 carrier 必须稳定指向 canonical `invoke(args_tuple) -> Step_F` target；
     - 允许用显式发布的 wrapper symbol 作为桥接；
     - 但该 symbol 必须本身就是 refactor contract 的一部分，不能是 backend 现场猜出来的名字约定。
  2. 对齐 carrier target 与 `RefactorAbiQuery`。
     - `RefactorCallableLayout.dynamic_entry()` / 等价 published shell 必须能被 carrier materialization authoritative 地消费；
     - 若某个 callable 已发布 dynamic entry，但 closure/vtable/itable 仍无法把它接入 carrier；
     - 必须在 P6 边界显式 fail fast，而不是把 remap 责任留给 `P6-T03` body emitter。
  3. 对齐 `Unit` / zero-payload 与 receiver 形状。
     - `invoke_args_tuple_ty = ()` 时，carrier target 仍必须指向零载荷 canonical dynamic entry；
     - `Closure` / `FunValue` / `Virtual` / `Interface` 的 receiver/callee carrier ABI 不得偷偷退回 legacy 普通参数列表。
  4. 补充定向测试与 fixture。
     - 至少覆盖：
       - build fixture 能证明 refactor class vtable / interface itable / closure callable object 发布的 target 已切到 canonical dynamic entry；
       - 若 carrier 仍指向普通 ABI 或缺失 published target，编译会显式拒绝。

- 必须遵从的约束：
  - 禁止把 legacy 普通 top-level/lambda 函数指针当作“默认也等价于 refactor dynamic entry”。
  - 禁止在 `P6-T03` body emitter 中按符号名、wrapper 名字约定、`Span`、HIR 成员名、或旧 dispatch helper 反推 callable target。
  - 禁止把“query 已发布，但 runtime carrier 仍沿用旧函数指针 ABI”视为当前阶段已完成。

- 验证：
  - `cargo test -p scoopc refactor_llvm_dynamic_invoke_query`
  - `cargo test -p scoopc refactor_llvm_callable_carrier_layout`
  - 新增/更新以 carrier target 发布为重点的定向单元测试（推荐命名：`refactor_llvm_dynamic_entry_publication_*`）
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_invoke_candidate_set_emit_llvm.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_non_boundary_dynamic_call_emit_llvm.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

- 完成条件：
  - refactor 路径上的 closure/vtable/itable carrier 已 authoritative 地指向 canonical published dynamic entry；
  - `P6-T03` 可以只消费 runtime carrier + published query lower dynamic call，而不再借壳 legacy callable wrapper / dispatch ABI。
- 依赖：`P6-T02f`
- 完成记录：
  - 2026-05-03：完成 `P6-T02g`。在 `crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 中新增 callable carrier target 发布层：refactor ABI materializer 现在会为 closure carrier、class vtable slot、interface itable slot 预发布对应的 dynamic-entry shell，并把 `(carrier kind, callable fqn) -> published target symbol` 注册到编译单元共享 cache；若 refactor contract 已启用但 carrier 缺少 published target，后续 carrier materialization 会显式 fail fast，而不是静默回退到普通 ABI。
  - 2026-05-03：在 `crates/scoopc/src/llvm/codegen/{closure/mod.rs,mir_body.rs,gc.rs,mod.rs}` 中把 closure object、pass MIR `MakeClosure`、class vtable、interface itable 的 target 写入统一改成消费上述 authoritative mapping。legacy 路径在未启用 refactor carrier contract 时仍保持原行为；refactor 路径下则会把 closure `fn_ptr`、vtable slot、itable method array 改写为 `__scoop_refactor_{closure,vtable,itable}_dynamic_entry__*` published shell。
  - 2026-05-03：新增 `tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`，并在 `layout.rs` 中补充 `refactor_llvm_dynamic_entry_publication_*` 单元测试，覆盖 carrier target shell 发布与缺失 published target 的 fail-fast 行为；fixture 现在会直接断言 emitted LLVM IR 中的 class vtable / interface itable / closure object target 已切到 refactor dynamic entry，而不再指向普通 ABI 符号。
  - 已运行验证：
    - `cargo test -p scoopc refactor_llvm_dynamic_invoke_query`
    - `cargo test -p scoopc refactor_llvm_callable_carrier_layout`
    - `cargo test -p scoopc refactor_llvm_dynamic_entry_publication`
    - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_invoke_candidate_set_emit_llvm.scoop`
    - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_non_boundary_dynamic_call_emit_llvm.scoop`
    - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`
    - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## [DONE] P6-T02h：发布 `LocalRuntimeError` synthetic terminal state 的 authoritative lowering contract，禁止 P6-T03 在 backend 现场发明 pure caller runtime-error 的结束路径

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.9, §5.5.3-§5.5.7, §8
  - [`TODO-P6.md`](./TODO-P6.md) 中 `P6-T02e`
  - `crates/scoopc/src/effect_lowered/{ir,materialize,dump,opt}.rs`
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout,body}.rs`
- 背景：
  - `P6-T02e` 已经为 pure caller call boundary 发布了 `consumed_runtime_error_case` 的 `target_state`，并在 late-lowered state graph 中追加 `LocalRuntimeError(payload_tuple_ty=...)` synthetic terminal state；
  - 但这条 synthetic state 目前仍只公开了 payload type / target-state 身份，并没有发布“backend 到底要把它 lower 成什么终止语义”的 authoritative contract：
    - 是回到某个显式 published boundary / state；
    - 还是走某个已发布的 runtime fatal surface；
    - 还是进入某条已发布的 local handler / catch 结束路径；
  - `RefactorAbiQuery` 当前也只发布了 `input_case_tag` / `payload_abi` / `target_state`，没有发布 terminal action；
  - 因此若直接继续 `P6-T03`，backend 将不得不为 `LocalRuntimeError` 现场发明 pure caller runtime-error 的结束路径，违背本阶段的 contract-first / no-workaround 约束。

- 目标：
  - 为 `LocalRuntimeError` synthetic terminal state 补齐 authoritative、可 lower 的 terminal contract；
  - 让 `P6-T03` 只消费已发布 handoff，就能正确处理 pure caller 本地消费的 ordinary runtime-error，而不必在 LLVM backend 现场补想象终止语义。

- 必须实现的内容：
  1. 为 `LocalRuntimeError` synthetic state 发布显式 terminal lowering contract。
     - 允许方案：
       - 给 `LateLoweredStateTerminator::LocalRuntimeError` 增补结构化 terminal action；
       - 或用等价的结构化 lowering 节点替代当前只带 `payload_tuple_ty` 的壳；
     - 但无论采用哪种表示，都必须至少 authoritative 地发布：
       - 输入 runtime-error case 身份；
       - payload materialization contract；
       - terminal action（例如显式 outward emission、显式 local catch/handler path、或显式 runtime fatal surface）；
       - 所需 target state / boundary / symbol / runtime entry。
  2. 明确 pure caller local runtime-error 的语义边界。
     - 必须同时满足：
       - 不把 compiler-generated runtime-error case 反写回 caller `surface_ty` / outward `StepSchema`；
       - 不把该路径降级成 backend 私有 hidden trap / outcome side channel；
       - 若最终语义是 fatal/abort，也必须作为已发布 runtime contract 明确暴露，而不是 `P6-T03` 自己临时决定。
  3. 把新 contract 接入稳定 dump/query 面。
     - `dump-effect-lowered` 必须能公开 `LocalRuntimeError` 的 terminal action；
     - `RefactorAbiQuery` 或等价查询面必须能让 backend 稳定回查这条 terminal contract；
     - 若 terminal action 缺失、漂移、或与 state graph / payload contract 不一致，必须在 P5/P6 边界 fail fast。
  4. 补充定向回归。
     - 至少覆盖：
       - `tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop` 中 pure caller `main -> run(...)` 的 local runtime-error synthetic state 已拥有 backend 可执行的 terminal contract；
       - 缺失该 terminal contract 时会显式拒绝，而不是把责任留给 `P6-T03` 现场猜测。

- 必须遵从的约束：
  - 禁止把 `LocalRuntimeError(payload_tuple_ty=...)` 当成“backend 自己知道怎么收尾”的非正式约定。
  - 禁止用未发布的 panic/abort 路径掩盖 handoff 缺失。
  - 禁止借机扩大 pure caller 的 outward row / callable surface。

- 验证：
  - `cargo test -p scoopc refactor_boundary_lowering`
  - `cargo test -p scoopc refactor_effect_lowered_stage`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

- 完成条件：
  - `LocalRuntimeError` synthetic state 已拥有 authoritative、可 lower 的 terminal contract；
  - `P6-T03` 可以只消费已发布 contract lower pure caller runtime-error 结束路径，而不再需要 backend 现场发明语义。
- 依赖：`P6-T02e`
- 完成记录：
  - 2026-05-03：完成 `P6-T02h`。在 `crates/scoopc/src/effect_lowered/{ir,materialize,dump}.rs` 中把 pure caller `LocalRuntimeError` synthetic state 升级为结构化 terminal contract：`LateLoweredConsumedRuntimeErrorCase` 与 `LateLoweredStateTerminator::LocalRuntimeError` 现在都会 authoritative 地携带 `RuntimeFatal(runtime_entry=scoop_runtime_error_fatal)` terminal action，而不再只暴露 `payload_tuple_ty` 的非正式约定。`dump-effect-lowered` 也会同步公开 boundary/state 两侧的 terminal action。
  - 2026-05-03：在 `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs` 中为 `RefactorAbiQuery` 新增 LLVM 级 local runtime-error terminal contract：query 现在不仅发布 `input_case_tag / payload_abi / target_state`，还会发布对应的 runtime-fatal entry symbol/type，并在 P5/P6 边界对 `target_state`、payload、terminal action 一致性执行 fail-fast 校验，禁止 backend 在 `P6-T03` 现场自行决定 pure caller runtime-error 的结束路径。
  - 2026-05-03：在 `crates/scoopc/src/llvm/codegen/{runtime_symbols,runtime_abi}.rs` 与 `runtime/c/scoop_runtime.c` 中新增已发布 runtime entry `scoop_runtime_error_fatal(void* runtime_error)`。当前实现仍是立即终止，但该语义现在被显式固定在 runtime contract 中，而不是藏在 backend 私有 trap 约定里。
  - 2026-05-03：新增/更新定向回归：`refactor_boundary_lowering_keeps_local_runtime_error_contract_for_pure_caller_calls`、`refactor_effect_lowered_stage_dump_exposes_local_runtime_error_call_contract`、`refactor_llvm_local_runtime_error_contract_*`，覆盖 published terminal action、module symbol 声明，以及缺失 target state / 缺失 LocalRuntimeError terminator 时的 fail-fast。
- 已运行验证：
  - `cargo test -p scoopc refactor_boundary_lowering`
  - `cargo test -p scoopc refactor_effect_lowered_stage`
  - `cargo test -p scoopc refactor_llvm_local_runtime_error_contract`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## [DONE] P6-T02i：发布 synthetic invoke-carrier / source-type ABI value lowering contract，禁止 P6-T03 把 refactor handoff 类型回塞 legacy codegen `TypeStore`

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.2, §5.3.9, §5.5.1-§5.5.7, §8
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout,body}.rs`
  - `crates/scoopc/src/llvm/codegen/mir_body.rs`
  - `crates/scoopc/src/effect_lowered/{ir,materialize}.rs`
- 背景：
  - `P6-T03` 需要直接消费 P5/P6 handoff 中的 `invoke_args_tuple_ty`、synthetic `ResumeSurface::*` / `CallSurface::*` step schema，以及这些 schema 上的 payload / answer carrier；
  - 这些 source type 并不总是存在于 legacy codegen 使用的 `hir::LoweredHir.types` / `CgTy` 键空间中；
  - 当前 LLVM codegen 里的 `cg_ty_of`、`build_tuple_cg_value_from_values`、`cg_value_from_loaded`、以及基于 `equivalent_codegen_type_id(...)` 的 helper 都默认“要 lower 的类型已经在 `lowered.types` 中有等价项”；
  - 2026-05-03 开始实现 `P6-T03` 时已实际复现该缺口：一旦 direct entry / surface wrapper 尝试绑定 `invoke_args_tuple_ty` 为 synthetic carrier 的 callable（例如 closure / member invoke carrier），backend 就会被迫：
    - 要么把 late-lowered synthetic type 现场回塞到 legacy `TypeStore` / `CgTy` 体系；
    - 要么按源码/经验猜 carrier 形状并手写一套未发布的 load/store/build 规则；
    - 二者都违背 P5 -> P6 handoff 的 contract-first 边界。

- 目标：
  - 为 refactor LLVM backend 补齐一层 authoritative 的“source-type -> ABI value” lowering contract；
  - 让 `P6-T03` 可以在不依赖 legacy `lowered.types` 等价映射的前提下，稳定构造、装载、拆解和传递 synthetic invoke / resume carrier。

- 必须实现的内容：
  1. 建立 source-type ABI value lowering helper/query。
     - 至少要能 authoritative 地处理：
       - `Unit` / 单值 carrier；
       - tuple carrier；
       - synthetic `ResumeSurface::*` / `CallSurface::*` step schema 上的 complete / case payload field；
       - direct entry / dynamic entry / surface resume / internal resume method 的参数与返回值装载。
     - 这层 helper 必须直接消费 P5/P6 handoff 的 `TypeStore` / schema 信息，而不是偷偷回 HIR `lowered.types` 猜类型。
  2. 为 source-type ABI value lowering 建立稳定边界。
     - 推荐位置：`crates/scoopc/src/llvm/codegen/effect_refactor/{types,body}.rs`；
     - 允许新增独立 helper / query type；
     - 但禁止把 synthetic source type 临时重新 intern 到 legacy codegen `TypeStore` 里当 workaround。
  3. 为缺失映射建立 fail-fast verifier。
     - 若某个 synthetic carrier / payload / answer type 仍无法 lower，必须在 P5/P6 边界显式拒绝；
     - 禁止把责任留给 `P6-T03` 在 body emitter 现场 panic、猜 tuple 形状、或按旧 `CgTy` 分支特判。
  4. 建立定向回归。
     - 至少覆盖：
       - pure no-outward callable / lambda / member invoke 的 synthetic `invoke_args_tuple_ty` 能被 direct entry 稳定绑定；
       - synthetic `ResumeSurface::*` / `CallSurface::*` step schema 的 payload / complete carrier 能被 authoritative 地 lower；
       - 缺失这层 helper 时会显式 fail fast，而不是在 `P6-T03` 现场崩溃。

- 必须遵从的约束：
  - 禁止通过修改/污染 legacy `hir::LoweredHir.types` 来“补齐” refactor synthetic type。
  - 禁止让 `P6-T03` 继续依赖 `equivalent_codegen_type_id(...)` 是否恰好能在旧键空间找到匹配结果。
  - 禁止为 closure/member/resume 各写一套互不一致的私有 carrier lowering 规则。

- 验证：
  - `cargo test -p scoopc refactor_llvm_layout`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

- 完成条件：
  - refactor LLVM backend 已能 authoritative 地 lower synthetic invoke/resume carrier 与 step payload source type；
  - `P6-T03` 不再需要把 refactor handoff 类型回塞 legacy `TypeStore`，也不再需要 backend 现场猜 shape。
- 依赖：P6-T02h
- 完成记录：
  - 2026-05-03：在 `crates/scoopc/src/llvm/codegen/effect_refactor/types.rs` 中新增 `RefactorSourceAbiLayoutKind`、`RefactorSourceAbiFieldLayout`、`RefactorSourceAbiLayout`，并把 `source_value_layouts: BTreeMap<TypeId, RefactorSourceAbiLayout>` 挂到 `RefactorAbiQuery`。P6 后续 body emitter 现在可以只靠这层 query 回查 late-lowered source type 的 LLVM ABI value contract，而不必再把 synthetic type 回塞 legacy `TypeStore`。
  - 2026-05-03：在 `crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 中为 ABI materializer 增加 authoritative `source_value_layout(...)` 发布路径，并把它接到当前任务要求的全部 handoff 面：callable direct/dynamic entry `invoke_args_tuple_ty`、surface resume `resume_tuple_ty/answer_ty`、internal resume method payload、`Step` complete/case payload，以及 pure caller `LocalRuntimeError` payload。缺失或不可 lowering 的 source type 现在会在 ABI materialization 阶段以 `source-type ABI value lowering` 诊断显式 fail fast，而不会把问题留到 `P6-T03` body emitter。
  - 2026-05-03：已为 `RefactorStepVariantLayout` 补充 `payload_source_ty` 元数据，使 `CallSurface::*` / `ResumeSurface::*` 对应的 step payload field 能通过统一 query 回查 source layout；`P6-T03` 后续只需消费已发布 contract，不必再靠 `equivalent_codegen_type_id(...)`、legacy `CgTy` 分支或手写 tuple 猜测来拆装 payload。
  - 2026-05-03：已新增 `refactor_llvm_layout_*` 定向测试，覆盖 pure direct-entry invoke carrier（同时观察 single-value 与 tuple carrier）、unit case payload、tuple resume payload/answer，以及 synthetic invoke args 不可 lowering 时的 fail-fast。
- 已运行验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc refactor_llvm_layout`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## [DONE] P6-T02j：发布 `HandleDispatch` / completion-state lowering contract，禁止 P6-T03 在 backend 现场发明 handle body/arm/finally 的内部返回协议

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.16, §5.5.4-§5.5.7, §8
  - `crates/scoopc/src/effect_lowered/{ir,segment,frame,materialize,dump}.rs`
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{layout,types}.rs`
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`（只能作 legacy 对照，禁止把其中的 magic state/completion tag 直接当成 refactor contract 复用）
- 背景：
  - 2026-05-03 开始真正实现 `P6-T03` 的 whole-body emitter 时发现：`LateLoweredStateTerminator::HandleDispatch` 当前虽然已经发布了 `body_state`、`arm_states`、`finally_state`、`exit_state`、`boundary_ids`，且 `frame_schema` 也已显式保留 `StateTag` / `CompletionTag` 系统槽位；
  - 但现有 P5 -> P6 handoff 并没有进一步 authoritative 地说明：同一 callable 内部 handle body 子图完成后，backend 应如何区分“body 完成 / handled arm 命中 / finally 续点 / 向外继续传播 / dropped continuation abandon”，以及这些内部状态之间究竟通过什么已发布的 completion/state carrier 交接；
  - 当前仓库里唯一可见的具体协议仍藏在 legacy `state_machine_emitter.rs` 的 backend-private 常量和约定中（例如 `STATE_TAG_HANDLE_RETURNED` / `STATE_TAG_FUNCTION_RETURNED` 一类 magic tags，以及对应 completion slot 的消费路径）。若直接继续 `P6-T03`，refactor backend 将不得不：
    - 借壳 legacy emitter 的隐藏 magic values；或
    - 在 P6 现场重新发明一套 body/arm/finally/exit completion-state 协议；或
    - 错误地把 `HandleDispatch` 缩成“只跳 `body_state` 的普通 CFG 边”，从而无法实现 handled perform / finally / nested-handle 的正确语义。
  - 以上都违背本阶段的 contract-first / no-workaround 约束，因此必须先把这层 lowering contract 显式发布出来，再继续 `P6-T03`。

- 目标：
  - 为 refactor LLVM backend 补齐 authoritative 的 intra-callable `HandleDispatch` lowering contract；
  - 让 `P6-T03` 可以只消费已发布 handoff，就把 handle body / arm / finally / exit / outward-propagation 翻译成 LLVM CFG，而不再回 legacy state-machine emitter 借 private tags 或现场重造 completion 协议。

- 必须实现的内容：
  1. 在 P5/P6 handoff 中显式发布 `HandleDispatch` 的 internal completion/state contract。
     - 若 `StateTag` / `CompletionTag` 确实是 authoritative carrier，必须把它们对 `HandleDispatch` 的语义显式写成结构化 contract，而不是只保留系统槽位名字；
     - 若需要额外的 compiler-owned selector / case-dispatch / exit-state token，也必须作为已发布字段或 query 暴露出来；
     - 明确禁止让 `P6-T03` 继续从 legacy `STATE_TAG_*` 常量、HIR/Span、或 hidden runtime side channel 反推这层协议。
  2. 显式发布 handle body completion 到 arm/finally/exit 的 authoritative 映射。
     - 至少需要覆盖：
       - body 正常完成 -> `exit_state`
       - handled case 命中 -> 对应 arm state
       - arm 完成 / finally 完成后的续点
       - body/arm/finally 向外继续传播时的 outward `Step_F` / boundary emission contract
       - dropped continuation / abandon 与 pending cleanup 的分流边界
     - 若这些映射依赖 `HandleSiteEffectFacts.arm_facts()`、`LateLoweredHandleBoundaryLowering.outward_emissions()`、`resume_state_map` 或其它现有 authoritative 数据，必须把消费关系显式固定下来。
  3. 为 refactor ABI/query 层提供 backend 可直接消费的查询面或等价结构。
     - `P6-T03` 必须能够从 owner callable + `HandleDispatch` state / handle site 稳定回查：
       - handled case -> arm state
       - body completion / finally completion -> exit target
       - 向外传播时应构造的 canonical `Step_F` / boundary emission
       - 若使用 completion/state carrier：其 LLVM 级 layout、tag identity、以及允许的 tag 集合
     - 明确禁止把这些信息只保留在 dump 文本或测试私有 helper 中。
  4. 在 stage 边界增加 fail-fast 校验。
     - 缺失 `HandleDispatch` completion-state contract、缺失 handled case -> arm mapping、或 body/arm/finally/exit 与已发布 contract 漂移时，必须在 ABI materialization / refactor lowering 准备阶段显式拒绝；
     - 禁止把问题留到 `P6-T03` body emitter 现场 panic，或静默回落 legacy handle lowering。
  5. 更新文档 / dump surface。
     - `dump-effect-lowered`（或等价 published dump）必须把新发布的 `HandleDispatch` / completion-state contract 公开出来，使后续 review 能确认 backend 消费的就是这层 contract。

- 必须遵从的约束：
  - 禁止把 legacy `state_machine_emitter.rs` 中的 magic state/completion tag 当作“默认正确值”直接照搬而不先发布为 refactor contract。
  - 禁止让 `P6-T03` 通过 `Span`、HIR handle 结构、旧 handler-stack runtime ABI、或 source-shape 特判来补足 handle dispatch 语义。
  - 禁止把问题通过“暂时只支持没有 handle 的 effectful callable”来规避；像 `effect_resume_if_else_branch_single_perform.scoop`、`effect_multi_escape_indirect_direct_while.scoop` 这类真实 handle 路径必须在 contract 层闭合。

- 验证：
  1. 新增/更新定向单测，推荐命名：
     - `refactor_handle_dispatch_contract_*`
     - `refactor_completion_state_contract_*`
  2. 至少覆盖：
     - `HandleDispatch` 能 authoritative 地回查 handled case -> arm state / exit state / outward emission；
     - 若缺失 completion-state contract、handled case 映射、或 tag identity 漂移，会显式 fail fast；
     - `dump-effect-lowered` 会公开新的 published contract。
  3. 运行：
     - `cargo test -p scoopc refactor_handle_dispatch_contract`
     - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`

- 完成条件：
  - `HandleDispatch` / completion-state contract 已在 P5/P6 handoff 中 authoritative 发布；
  - `P6-T03` 后续可以只消费这层 contract 来 lower handle body/arm/finally/exit，而不再借壳 legacy private tags 或现场发明新的内部返回协议。
- 依赖：P6-T02i
- 完成记录：
  - 2026-05-03：在 `crates/scoopc/src/effect_lowered/ir.rs` 为 `LateLoweredStateTerminator::HandleDispatch` 新增结构化 `LateLoweredHandleDispatchContract`，显式发布 carrier（`StateTag` / `CompletionTag` / `ResumePayloadCarrier`）、handled case -> arm state 映射、body/arm/finally completion target、body/finally/arm outward case 集、published outward emissions、pending completion token，以及 abandon target。
  - 2026-05-03：在 `crates/scoopc/src/effect_lowered/materialize.rs` 基于 `HandleSiteEffectFacts` + handle boundary lowering 物化该 contract，并对 handled-arm 数量、handle boundary source/kind、outward emission case 集等执行 fail-fast；`crates/scoopc/src/effect_lowered/opt.rs` 也已同步保持 state redirect 后的 contract 一致性。
  - 2026-05-03：在 `crates/scoopc/src/effect_lowered/dump.rs` 把 `handle_contract` 渲染到 stable dump / `dump-effect-lowered` surface，后续 review 可以直接看到 body/arm/finally completion 与 pending completion 发布结果。
  - 2026-05-03：在 `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs` 为 `RefactorAbiQuery` 新增 `HandleDispatch` published layout，发布 `StateTag` / `CompletionTag` / `ResumePayloadCarrier` field index 与 completion tag identity，并在 ABI materialization 阶段对 lowered contract、frame system slot、pending completion tag、handled-arm mapping 做 fail-fast 校验。
  - 2026-05-03：新增/更新定向测试 `refactor_handle_dispatch_contract_*` 与 `refactor_completion_state_contract_*`，覆盖 late-lowered 正例、dump 暴露、LLVM query 正例，以及缺失 handled-arm / completion-tag 槽位时的显式拒绝。
- 已运行验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc refactor_handle_dispatch_contract`
  - `cargo test -p scoopc refactor_completion_state_contract`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## [DONE] P6-T02k：发布 `HandleDispatch` arm payload binder / escape-continuation binder contract，禁止 P6-T03 在 body emitter 现场回 canonical MIR handle arm 恢复绑定形状

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.16, §5.3.2-§5.3.6, §5.5.4-§5.5.7, §8
  - `crates/scoopc/src/effect_lowered/{ir,frame,materialize,dump}.rs`
  - `crates/scoopc/src/effect_facts/{facts,dump}.rs`
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs`
  - `crates/scoopc/src/mir/mod.rs`（只能作当前缺口的对照，任务完成后禁止让 P6-T03 继续把这里当 authoritative source）
- 背景：
  - 2026-05-03 开始真正编写 `P6-T03` 的 handle-state emitter 时发现：当前 late-lowered / LLVM query handoff 已经 authoritative 发布了：
    - `HandleDispatch` 的 handled case -> arm state 映射；
    - `payload_carrier` / `state_tag` / `completion_tag` carrier；
    - `body/arm/finally/exit` completion target 与 outward emission contract；
  - 但它还没有进一步 authoritative 发布 arm 入口绑定本身：
    - 某个 handled case 进入对应 arm state 时，payload tuple 的各个字段应写到哪些 binder；
    - 该 arm 是否存在 escape continuation binder；
    - 若存在 continuation binder，它应绑定哪个已发布 continuation schema / object query；
    - 同一 handle site 下多 arm 的 binder 集合如何与 `handled_case` / `arm_state` 稳定对应。
  - 当前 `LateLoweredFrameSlotKind::HandleBinder { site_id, ordinal }` 只给出了“同一 handle site 下存在某些 payload binder slot”的弱信息：
    - 它没有把 binder slot authoritative 地挂到具体 `handled_case` / `arm_state`；
    - continuation binder 也没有等价的 published contract；
    - 因而一旦同一 handle site 有多个 arm，或 arm 同时绑定 payload + continuation，backend 仍无法只靠 handoff 还原 arm 入口绑定。
  - 若直接继续 `P6-T03`，refactor body emitter 将不得不回 canonical MIR `TerminatorKind::Handle { arms, binder_locals, continuation_local, .. }`（甚至回 HIR）现场恢复 arm entry shape；这正是 `P6-T03` 明确禁止的 `mir::Body`/shape fallback。
  - 因此必须先把这层 contract 作为新的 authoritative handoff 发布出来，再继续 `P6-T03`。

- 目标：
  - 为每个 `HandleDispatch` / handled arm authoritative 发布 arm-entry binding contract；
  - 让 `P6-T03` 能只消费 late-lowered + LLVM query handoff，就完成 handled payload 与 escape continuation binder 初始化，而不再回 canonical MIR handle arm 或 HIR 恢复绑定形状。

- 必须实现的内容：
  1. 在 late-lowered handoff 中显式发布 handle arm binding contract。
     - 至少需要 authoritative 暴露：
       - handled case / arm state / arm ordinal；
       - payload tuple ty 与 payload binder 列表（按 published ordinal 顺序）；
       - optional escape continuation binder；
       - continuation binder 关联的 continuation schema / continuation object / ABI query key（若适用）。
     - 若需要新增独立结构（例如 `LateLoweredHandleArmBinding`），允许新增；
     - 但禁止继续只靠 `HandleBinder { site_id, ordinal }` 这类 site-scoped 弱信息，让 backend 自己反推 arm 入口形状。
  2. 把 payload binder / continuation binder contract 与已发布 `HandleDispatch` contract 绑定起来。
     - backend 必须能够从 owner callable + handle site + handled case 稳定回查：
       - 进入哪个 arm state；
       - 需要初始化哪些 payload binder；
       - 是否需要发布 continuation binder，以及它消费哪条 continuation contract；
       - 若 arm 没有 binder / 没有 continuation binder，也必须显式可见，而不是靠“查不到就当没有”推断。
  3. 在 LLVM ABI/query 层为上述 contract 提供直接查询面。
     - `P6-T03` 不得再读取 canonical MIR `HandlerArm.binder_locals` / `continuation_local` 当 authoritative source；
     - 缺失 handled-case 映射、payload binder 发布不完整、或 continuation binder contract 漂移时，必须在 ABI materialization / refactor lowering 准备阶段 fail fast。
  4. 更新 dump / published surface。
     - `dump-effect-lowered`（或等价 surface）必须公开 handle arm binding contract，使 review 能直接看到：
       - handled case -> arm state；
       - payload binder ordinals；
       - optional continuation binder / continuation schema；
       - 与 arm outward cases 的关系。
  5. 若当前 frame schema 需要为 continuation binder 增加更稳定的 slot/query 分类，必须在本任务中一并发布。
     - 明确禁止把 continuation binder 保留成“只有 canonical MIR arm locals 才知道的隐式输入”。

- 必须遵从的约束：
  - 禁止把 canonical MIR `TerminatorKind::Handle.arms` / `binder_locals` / `continuation_local` 当成 `P6-T03` 的 semantic source of truth。
  - 禁止只发布 payload binder 数量而不发布具体 binder / continuation 绑定。
  - 禁止通过“当前只支持单 arm / 无 continuation binder 的 handle”来规避该 contract 缺口。
  - 禁止让 `P6-T03` 继续依赖 site-scoped heuristic（按 `site_id` + ordinal 扫 slot / local）去猜哪组 binder 属于哪条 handled case。

- 验证：
  1. 新增/更新定向单测，推荐命名：
     - `refactor_handle_arm_binding_contract_*`
     - `refactor_handle_arm_continuation_binding_*`
  2. 至少覆盖：
     - 单 arm + escape continuation 的 handle site，会 authoritative 发布 payload binder 与 continuation binder contract；
     - 多 arm + mixed payload/continuation binder 的 handle site，handled case -> arm/binder 绑定不会歧义；
     - 若缺失 continuation binder 发布、payload binder 次序漂移、或 handle site contract 不完整，会显式 fail fast；
     - `dump-effect-lowered` 会公开新的 arm binding contract。
  3. 运行：
     - `cargo test -p scoopc refactor_handle_arm_binding_contract`
     - `cargo test -p scoopc refactor_handle_arm_continuation_binding`
     - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`

- 完成条件：
  - `HandleDispatch` arm payload binder / escape continuation binder contract 已在 late-lowered + LLVM query handoff 中 authoritative 发布；
  - `P6-T03` 后续可以不再回 canonical MIR handle arm / HIR 恢复 binder 形状，而只消费已发布 contract 完成 arm 入口初始化。
- 依赖：P6-T02j
- 完成记录：
  - 2026-05-03：已在 `crates/scoopc/src/effect_lowered/ir.rs` 扩展 `LateLoweredHandleDispatchContract` / `LateLoweredHandleArmDispatch`，新增 per-arm payload binder 与 optional continuation binder published contract：每个 handled arm 现在都会 authoritative 发布 `handled_case -> arm_state -> arm_ordinal -> payload_tuple_ty -> payload_binders(local + optional frame_slot) -> optional continuation_binder(local + optional frame_slot + continuation_schema + continuation_object)`。
  - 2026-05-03：已在 `crates/scoopc/src/effect_lowered/materialize.rs` 让 `build_handle_dispatch_contract(...)` 显式回读 canonical MIR `TerminatorKind::Handle` 仅用于 P5 authoritative 发布阶段，把 `binder_locals` / `continuation_local` 与 `HandleSiteEffectFacts.arm_facts()` 的 `payload_tuple_ty` / `continuation_schema` 接起来；`P6-T03` 后续不再需要自己回 MIR arm 恢复绑定形状。
  - 2026-05-03：已在 `crates/scoopc/src/effect_lowered/dump.rs` 暴露新的 arm binding surface；`dump-effect-lowered` 现在会直接显示 `payload_binders:` 与 `continuation_binder:`，包括 binder ordinal、local、optional frame slot、continuation schema 与 continuation object。
  - 2026-05-03：已在 `crates/scoopc/src/llvm/codegen/effect_refactor/types.rs` / `layout.rs` 增加对应的 per-arm LLVM query layout，并在 ABI materialization 阶段对以下漂移 fail fast：
    - arm ordinal / arm state 与 state graph 不一致；
    - payload tuple type、payload binder 次序或 local 漂移；
    - 缺失/多余 continuation binder；
    - continuation binder 指向的 continuation object / surface-resume binding 未发布。
  - 2026-05-03：作为本任务验证中的直接相关修复，`materialize_handle_dispatch_layouts(...)` 现只对实际含 `HandleDispatch` 的 callable 要求 handle system slots，不再错误拒绝无 handle 的 pure helper / `main` callable；该问题会直接阻断本任务的 multi-callable ABI query 验证，因此一并纳入 `P6-T02k` 修复。
  - 2026-05-03：新增/更新定向测试 `refactor_handle_arm_binding_contract_*` 与 `refactor_handle_arm_continuation_binding_*`，覆盖：单 arm payload+continuation binder 发布、多 arm mixed binder 发布、`dump-effect-lowered` 暴露、payload binder 次序漂移 fail-fast，以及缺失 published continuation binder fail-fast。
- 已运行验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc refactor_handle_arm_binding_contract`
  - `cargo test -p scoopc refactor_handle_arm_continuation_binding`
  - `cargo test -p scoopc refactor_handle_arm_`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## [DONE] P6-T02kR：Review `HandleDispatch` arm binder / continuation binder contract，确认 P6-T03 不再需要回 canonical MIR handle arm 恢复绑定形状

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.16, §5.3.2-§5.3.6, §5.5.4-§5.5.7, §8
  - `crates/scoopc/src/effect_lowered/{ir,frame,materialize,dump}.rs`
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs`
- 重点：
  - handled case -> arm state / payload binder / continuation binder 是否已经 authoritative 发布；
  - continuation binder 是否有稳定 query，而不是继续藏在 canonical MIR arm locals 中；
  - `P6-T03` 是否已经可以只消费 published contract 完成 arm 入口初始化。
- 必须检查的文件/位置：
  - 新增的 handle arm binding contract 定义位置
  - `dump-effect-lowered` 对应渲染位置
  - LLVM ABI/query 发布位置

- 验证：
  - 重新运行 P6-T02k 的全部测试与命令；
  - 额外搜索：
    - `rg "binder_locals|continuation_local|TerminatorKind::Handle" crates/scoopc/src/llvm/codegen/effect_refactor crates/scoopc/src/effect_refactor_pipeline`
  - 要求：
    - 允许命中：测试、注释、late-lowered contract 发布代码；
    - 不允许命中：refactor LLVM 主实现仍把 canonical MIR handle arm 当 semantic source of truth。

- 完成条件：
  - review 能明确说明：P6-T03 不再需要回 canonical MIR handle arm / HIR 恢复 payload 与 continuation binder 绑定；
  - 可重新进入 P6-T03。
- 依赖：P6-T02k
- 完成记录：
  - 2026-05-03：复核 `crates/scoopc/src/effect_lowered/{ir,materialize,dump}.rs` 后确认，handled case -> arm state / payload binders / optional continuation binder 已在 `LateLoweredHandleDispatchContract` 中 authoritative 发布；`dump-effect-lowered` 也会稳定公开 `payload_binders:` 与 `continuation_binder:` surface，因此 arm 入口初始化所需的 published contract 已经齐全。
  - 2026-05-03：复核 `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs` 后确认，`RefactorHandleDispatchLayout` / `RefactorHandleArmLayout` 已直接发布 `payload_binders()` / `continuation_binder()` / `handled_arm(...)` 查询面；后续 `P6-T03` 可以只消费 LLVM query 完成 arm 入口初始化，而不必回 canonical MIR handle arm 恢复绑定形状。
  - 2026-05-03：额外搜索 `binder_locals|continuation_local|TerminatorKind::Handle` 显示，`crates/scoopc/src/effect_refactor_pipeline` 中的命中仅位于 `mir_stage.rs`（P3/P5 contract 生成与测试侧）；`crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 中的少量命中仅用于 ABI materialization fail-fast 交叉校验 published contract 与 canonical MIR 是否漂移，发布到 query 的 binder/continuation 数据仍以 late-lowered contract 为 authoritative source，而不是由 LLVM backend 重新从 MIR arm locals 推导。
  - 2026-05-03：重新运行 `cargo test -p scoopc refactor_handle_arm_binding_contract`、`cargo test -p scoopc refactor_handle_arm_continuation_binding`、`cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`、`cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`、`cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`，均通过；本 review 未发现需要新增的 blocker 或前置任务，可重新进入 `P6-T03`。

## [DONE] P6-T02l：发布 `HandleDispatch` state-region / boundary-consumption contract，禁止 P6-T03 在 backend 现场重建 body/arm/finally 子图归属

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.5.3-§5.5.7, §8
  - [`TODO-P6.md`](./TODO-P6.md) 中 `P6-T02j`、`P6-T02k`
  - `crates/scoopc/src/effect_lowered/{ir,materialize,dump,opt}.rs`
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout,body}.rs`
- 背景：
  - 2026-05-03 继续实现 `P6-T03` 时发现：当前 handoff 已经 authoritative 发布了 `HandleDispatch` 的 `body_state` / `arm_states` / `finally_state` / `exit_state`、handled case -> arm 映射、pending completion、以及 arm payload/continuation binder；
  - 但它还没有 authoritative 发布“哪些 late-lowered states / boundaries 属于该 handle 的 body / arm / finally region，以及某个 boundary/outward case 是否应被当前 handle 本地消费、经 finally pending，还是直接向外传播”的稳定查询面；
  - 以 `tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop` 为例，`run` 的 perform boundary `bd2` 需要在 body region 内命中 handled case `c0` 后跳到 arm `st3`，同时把 continuation 恢复点固定到 `st9`；当前 published contract 分别公开了这些点状事实，但没有把 `bd2/st6/st9` 与 handle site `site0` 的消费路由 authoritative 地连成可直接消费的 lowering contract。
  - 若直接继续 `P6-T03`，backend 将不得不在 P6 现场通过 state-graph 遍历或回 canonical MIR 重新恢复 body/arm/finally 子图归属与 boundary 消费路由；这已经超出“只翻译已发布 handoff”的职责边界，等价于在 backend 再做一轮高层控制流分析。

- 目标：
  - 把 `HandleDispatch` 子图归属与 boundary 消费路由显式发布为 authoritative handoff；
  - 让 `P6-T03` 可以仅凭 P5/P6 已发布查询面，判断某个 state/boundary 当前处在 handle 的 body / arm / finally 哪个 region，以及该 outward case 应被 arm 消费、转成 pending completion、还是直接 outward emission，而不再在 backend 现场重建这层语义。

- 必须实现的内容：
  1. 为 `HandleDispatch` 发布 authoritative 的 state-region / boundary-consumption contract。
     - 允许表示方式：
       - 在 `LateLoweredHandleDispatchContract` 中新增显式 region membership / boundary routing 字段；
       - 或发布等价的 compiler-owned query 容器；
     - 但无论使用哪种表示，都必须至少稳定回答：
       - 给定 handle site / `StateId`，该 state 属于 body / 哪个 arm / finally / exit 之外的哪类 region；
       - 给定 handle site / `BoundaryId`（或其 owner state），该 boundary 的 outward case 是被当前 handle 本地消费、经 finally pending，还是直接 outward emission；
       - handled case 命中后应接到哪个 arm state，以及 continuation 应保留哪个已发布 `resume_state`。
  2. 这层合同必须在 P5 authoritative 发布阶段生成，而不是留给 backend 现场重建。
     - 可以消费 late-lowered state graph、boundary map、resume-state map 与现有 `HandleDispatch` contract；
     - 但禁止把 region 归属恢复推迟到 LLVM emitter 再通过 DFS/回 MIR 推断。
  3. 把新合同接到稳定 dump / LLVM query 面。
     - `dump-effect-lowered` 必须能稳定公开每个 handle site 的 region / routing 信息；
     - `RefactorAbiQuery` 或等价查询面必须能让 backend 只靠 owner step schema + handle site + state/boundary 完成回查；
     - 若 region membership、boundary routing、handled-case arm target、或 resume target 与 state graph / boundary map 漂移，必须在 P5/P6 边界显式 fail fast。
  4. 补充定向回归。
     - 至少覆盖：
       - `tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop` 中 handle body 内 perform boundary 的 handled routing；
       - `tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop` 中 handle body/arm tail 与 resume-state routing；
       - 一个带 `finally` 的 handle fixture，证明 pending completion / finally routing 也拥有同样的 published contract。

- 必须遵从的约束：
  - 禁止把“从 `body_state` / `arm_states` 出发再遍历一次 CFG 就能算出来”当作 backend 合法职责；若 `P6-T03` 需要这层语义，就必须先 authoritative 发布。
  - 禁止让 `P6-T03` 继续回 canonical MIR `Handle` terminator、HIR handle 结构、`Span`、或 legacy `STATE_TAG_*` magic tags 恢复 body/arm/finally 边界。
  - 禁止把 handled routing 写成只在测试里存在的私有 helper；CLI 与 Rust 测试必须共享同一 published contract。

- 验证：
  - `cargo test -p scoopc refactor_handle_dispatch_region_contract`
  - `cargo test -p scoopc refactor_handle_dispatch_region_routing`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

- 完成条件：
  - `HandleDispatch` 的 body/arm/finally region 与 boundary 消费路由已作为 authoritative handoff 发布；
  - `P6-T03` 可以只消费 published contract lower handle 子图，不再需要 backend 现场重建子图归属或 handled routing。
- 依赖：`P6-T02kR`
- 完成记录：
  - 2026-05-03：已在 `crates/scoopc/src/effect_lowered/ir.rs` 扩展 `LateLoweredHandleDispatchContract`，新增 authoritative `state_regions` / `boundary_routings` published surface，以及 `LateLoweredHandleStateRegion*` / `LateLoweredHandleBoundary*Routing*` 结构。每个 handle site 现在都会显式发布 body / arm / finally / dispatch / exit state membership，以及 boundary 上每条 outward case 的 `consume_to_arm` / `pending completion` / `emit_outward` 路由动作。
  - 2026-05-03：已在 `crates/scoopc/src/effect_lowered/materialize.rs` 让 P5 authoritative 发布阶段基于 state graph + boundary map 构造这层 contract，而不是把 region/routing 恢复留给 backend。实现中显式覆盖了：body perform handled routing、multi-boundary resume-state routing、pending completion、以及 body 区域 nested-handle finally-origin outward case 的直接 outward emission。
  - 2026-05-03：已在 `crates/scoopc/src/effect_lowered/{dump,opt}.rs` 接上这层 surface。`dump-effect-lowered` 现在会稳定公开 `state_regions:` / `boundary_routings:` / `case_routings:`；post-opt state redirect 也会同步重写新 contract 中的 `StateId`，避免优化后 routing 漂移。
  - 2026-05-03：已在 `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs` 为 `RefactorHandleDispatchLayout` 增加直接 query API（state region / boundary routing lookup），并在 ABI materialization 阶段对 published region membership、boundary routing、handled-arm target、以及 continuation resume-state 与 state graph / boundary map 的一致性执行 fail-fast 校验，禁止 `P6-T03` 再在 backend 现场重建 handle 子图归属。
  - 2026-05-03：已新增定向测试 `refactor_handle_dispatch_region_contract_*` 与 `refactor_handle_dispatch_region_routing_*`，覆盖：`effect_resume_if_else_branch_single_perform.scoop` 的 body perform handled routing、`effect_multi_escape_indirect_direct_while.scoop` 的多 boundary resume-state routing、带 finally 的 pending/direct outward routing、以及 published routing 漂移时的 LLVM ABI fail-fast。
- 已运行验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc refactor_handle_dispatch_region_contract`
  - `cargo test -p scoopc refactor_handle_dispatch_region_routing`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## P6-T02m：发布 continuation surface-resume -> owner dispatch contract，禁止 P6-T03 在 backend 现场扫描 continuation object 或猜 owner callable

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.2-§5.3.6, §5.5
  - `crates/scoopc/src/effect_facts/builder.rs`
  - `crates/scoopc/src/effect_lowered/ir.rs`
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs`
- 背景：
  - `P6-T02c` 已发布 `ContinuationSchemaId -> surface-resume layout/symbol` 查询面，也让 continuation object layout 额外发布了 object-level schema binding；
  - 但当前 `RefactorContinuationSurfaceResumeLayout` 只固定了 `__scoop_refactor_surface_resume__k*` 的 symbol/signature/`out_step_schema`，`RefactorContinuationSurfaceResumeBinding` 只记录 `continuation_schema` 与 `return_step_schema`；
  - 与此同时，`ContinuationSchemaId` 在 `crates/scoopc/src/effect_facts/builder.rs` 中按 `(resume_tuple_ty, answer_ty, out_step_schema, surface_ty)` canonical 去重，而不是按 owner callable / continuation object 唯一化；
  - 这意味着同一个 surface-resume schema 允许被多个 continuation object / owner callable 复用，但当前 handoff 没有发布“这个 shared surface symbol 如何 authoritative 地到达 owner-specific resume implementation”的 contract；
  - 若直接继续 `P6-T03`，backend 只能：
    - 扫描 raw late-lowered continuation object 列表，临时拼出 schema -> owner/method 选择规则；
    - 或按 runtime type desc / symbol 名字 / object layout 假定去猜该走哪条 owner body；
    - 或在 surface resume shell 里偷偷发明一套未发布的 second-stage dispatch。
  - 以上都违背 `P6-T02c` 已明确禁止的“不得靠扫描 raw late-lowered 列表或未发布规则补足 surface-resume dispatch”边界。

- 目标：
  - 在进入 `P6-T03` 之前，先把 continuation surface-resume 从 shared schema symbol 到 owner-specific resume implementation 的 authoritative dispatch contract 显式发布出来；
  - 让后续 body emitter 能仅凭 published handoff 定义并调用 `__scoop_refactor_surface_resume__k*`，而不需要现场枚举 continuation object、比较 runtime type、或猜 owner callable。

- 必须实现的内容：
  1. 为 surface-resume implementation target 发布 compiler-owned dispatch contract。
     - authoritative key 必须仍以 `ContinuationSchemaId` 或等价 stable identity 为主；
     - contract 必须显式说明 shared surface-resume symbol 如何到达 owner-specific implementation；
     - 可行形态包括但不限于：
       - `ContinuationSchemaId -> (ResumeInterfaceId, CaseTag, object-side lookup contract)`；
       - `ContinuationSchemaId -> owner-specific trampoline set + authoritative runtime selector`；
       - 或其它等价且已发布的 compiler-owned dispatch plan；
     - 明确禁止让 `P6-T03` 在 body emitter / surface-resume body 现场再扫描 raw continuation object / late-lowered method 列表临时恢复这层规则。
  2. 为 continuation object / LLVM query 发布 surface-resume body 所需的 object-side lookup contract。
     - 若 surface-resume 需要经由 object field / interface vtable / method slot / owner trampoline 继续分派，相关 identity 与 lookup path 必须作为 published contract 暴露；
     - 若同一 `ContinuationSchemaId` 被多个 object 复用，必须显式说明 shared symbol 采用哪条 authoritative dispatch 路径，而不是让 backend 自己比较 runtime type 或 header；
     - 若 contract 设计要求“同一 schema 只能对应唯一 owner/method identity”，则必须在 P5/P6 边界对多重发布显式 fail fast。
  3. 对缺失、歧义或漂移的 surface-resume dispatch contract fail fast。
     - 至少包括：
       - schema 已发布 surface symbol，但缺少 owner dispatch target；
       - 同一 schema 对应多个互不兼容的 owner/method/vtable lookup；
       - continuation object layout 缺少 surface-resume body 继续分派所需的 published lookup；
       - `P6-T03` 若只消费 ABI query / late-lowered contract 仍无法唯一决定 surface-resume body。
  4. 补充定向测试与回归。
     - 至少覆盖：
       - ABI query / late-lowered handoff 可从真实 `ContinuationSchemaId` authoritative 地解析 surface-resume implementation target；
       - 多个 continuation object 共享同一 schema 时，不需要 backend 现场扫描 raw object 列表；
       - 缺失或歧义 contract 时显式拒绝；
       - `tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop` 这类真实 `k.resume(...)` 路径不再把责任留给 `P6-T03` 现场猜 owner dispatch。

- 必须遵从的约束：
  - 禁止在 `P6-T03` 的 surface-resume body 中按 symbol 名、runtime type desc、header 指针、`ResumeStateTag` 偶然分布、或 raw late-lowered object 顺序发明 dispatch 规则；
  - 禁止把“扫描 continuation object 列表再找一个 continuation_schema 相等的 method”当成 backend 合法职责；若需要这层关系，必须先 authoritative 发布；
  - 禁止把 surface-resume shared symbol 悄悄退化成 owner-private symbol，除非这也是显式发布并能由 `ContinuationSchemaId` authoritative 查询到的 contract。

- 验证：
  - `cargo test -p scoopc refactor_llvm_surface_resume_layout`
  - `cargo test -p scoopc refactor_llvm_continuation_layout`
  - `cargo test -p scoopc refactor_llvm_`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`

- 完成条件：
  - surface-resume shared schema symbol 到 owner-specific implementation 的 dispatch contract 已成为 published handoff；
  - `P6-T03` 可以只消费 authoritative contract 定义/调用 surface-resume body，而不再现场扫描 continuation object 或猜 owner callable；
  - 缺失、歧义或漂移时会在 P5/P6 边界显式拒绝。
- 依赖：P6-T02c
- 完成记录：
  - （执行时填写）

## P6-T03：按 P5 state graph / boundary contract 完成 refactor LLVM body lowering，停止在 backend 重做 state-machine transformation

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.16, §5.2, §5.3.9, §5.5.1-§5.5.7, §8
  - 当前实现参考：
    - `crates/scoopc/src/llvm/codegen/mir_body.rs`
    - `crates/scoopc/src/llvm/codegen/call/dispatch.rs`
    - `crates/scoopc/src/llvm/codegen/call/resume.rs`
    - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`（只能作 legacy 对照）
    - `crates/scoopc/src/llvm/codegen/effect/state_machine_bridge.rs`（只能作 legacy 对照）
    - P5 late-lowered representation 定义位置
- 目标：
  - 用 P5 已经完成的 state graph / frame schema / boundary map / `impl_plan`，直接生成 refactor LLVM body；
  - 保证 backend 只是“翻译既有 state graph”，而不是在 LLVM 阶段再次识别 boundary、再切 CFG、或按 code shape 重新做局部状态机。

- 必须实现的内容：
  1. 建立 refactor LLVM body emitter。
     - 推荐位置：`crates/scoopc/src/llvm/codegen/effect_refactor/body.rs`；
     - 输入必须至少包含：
       - callable version
       - state graph
       - frame layout 查询面
       - `Step_F` / continuation / invoke / resume-interface LLVM signatures
       - P5 boundary/resume mapping 与 `impl_plan`
     - 明确禁止：让 body emitter 回 `hir::HandleExpr`、`mir::Body`、`Span`、或旧 `effect_op_call_sites` 猜边界语义。
  2. 把 `StateId` / `BoundaryId` / `resume_state` 显式映射成 LLVM CFG。
     - 必须显式产出：
       - callable version entry block
       - state dispatch / switch / branch
       - 各 state segment 的 straight-line 代码
       - completion / cleanup / drop / resume 后缀路径
     - 不能让 P6 依赖 direct-style CFG 原样保留。
  3. 统一 lower 所有 boundary。
     - `Perform`：
       - 直接构造 outward `Step_F` case；
       - 生成/捕获当前 continuation object；
       - 跳出当前 callable version。
     - effectful `Call` / `invoke`：
       - 调用 callee 的 canonical entry（直接已知 target 可 direct call；动态 target 走 canonical `invoke(args_tuple)`）；
       - 对返回的 `Step_F` 做显式 dispatch：
         - `Complete(answer)` -> 写 result slot，跳到当前 boundary 的 `resume_state`
         - outward case -> 构造对当前 caller 的 outward `Step_F`，并捕获 caller continuation object
     - `Resume`：
       - 调用 continuation object 的 resume interface method；
       - 返回值必须继续走与 call boundary 同一套 `Step_F` dispatch 逻辑；
       - one-shot 非法再次恢复必须作为 ordinary runtime error outward 进入普通 case 分支，而不是 trap-only 路径。
     - ordinary runtime error outward：
       - 必须 lower 成普通 outward case；
       - 不得绕开 `Step_F` 直接走隐藏异常边。
     - outward nested-handle boundary：
       - 必须作为真正 boundary 进入同一 outward `Step_F` 模型；
       - `SelfContained` nested handle 不得再向外层扩散切分。
  4. 把 P5 的显式控制流合同翻译到 LLVM CFG。
     - `return`
     - `break` / `continue`
     - `finally` / cleanup
     - handler arm 结束后的续点
     - dropped continuation completion path
     都必须来自 P5 state graph 的显式路径，而不能在 backend 再根据源码形状重建。
  5. 按 `impl_plan` 生成不同复杂度但同一合同的 LLVM lowering。
     - `NoOutward`：
       - 只允许生成 `Complete` 路径；
       - 不得因为它简单就绕开统一 state/body emitter。
     - `SingleCase(case_tag)`：
       - 允许内部减少 case dispatch 分支；
       - 但 outward 返回类型、tag 编号、dynamic surface 仍是 canonical `Step_F`。
     - `CanonicalFull`：
       - 保持完整 case dispatch。
  6. 若 legacy `state_machine_emitter` 中有完全中立的 CFG/IR helper，可抽共享；
     - 但前提是：
       - 不依赖 `UnifiedHandleLoweringContract`
       - 不依赖 HIR/Span
       - 不内置 tail-resume / statement-only / 单 handle 专用 fast path
     - 若任一条件不满足，则必须在 refactor 新路径中重建 emitter。
  7. 建立 refactor LLVM body verifier 或等价断言层。
     - 至少要能在 codegen 开始前/过程中显式拒绝：
       - 缺失 `StateId` 映射
       - 缺失 `BoundaryId -> owner/resume` 映射
       - frame slot 查询失败
       - `Step_F` / continuation signature 与当前 callable version 不匹配
       - backend 仍试图从 source shape 推断 control/effect contract
     - 明确禁止：这些场景静默 fallback 到 legacy effect emitter。

- 必须遵从的约束：
  - 禁止在 LLVM backend 再做第二次 segmentation 或 frame lifting。
  - 禁止在 refactor body emitter 中继续调用 legacy `build_unified_lowering_contract`、`effect_analysis_ctx`、`current_call_site(span)`、`effect_op_call_sites` 等旧入口作为主事实来源。
  - 禁止为 `single perform` / tail-`resume` / statement-only / 线性函数保留独立 effect lowering 入口。
  - 禁止把 runtime error 重新变成隐藏 trap/outcome channel。

- 验证：
  1. 新增/更新单元测试，推荐命名：
     - `refactor_llvm_state_graph_lowering_*`
     - `refactor_llvm_boundary_codegen_*`
     - `refactor_llvm_impl_plan_codegen_*`
     - `refactor_llvm_runtime_error_case_*`
  2. 新增/更新 build fixtures，推荐至少包括：
     - `tests/fixtures/build/effect_refactor_no_outward_complete_only.scoop`
       - 目标：锁定 `NoOutward` 只产出 `Complete` 路径
     - `tests/fixtures/build/effect_refactor_single_case_codegen.scoop`
       - 目标：锁定 `SingleCase` 仅缩小 dispatch，而不改变 canonical `Step_F`
     - `tests/fixtures/build/effect_refactor_boundary_inside_expr_emit_llvm.scoop`
       - 目标：锁定 boundary-in-expression 的 owner/resume lowering 已进入 LLVM CFG
  3. 运行：
     - `cargo test -p scoopc refactor_llvm_state_graph_lowering`
     - `cargo test -p scoopc refactor_llvm_boundary_codegen`
     - `cargo test -p scoopc refactor_llvm_impl_plan_codegen`
     - `cargo test -p scoopc refactor_llvm_runtime_error_case`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_no_outward_complete_only.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_single_case_codegen.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop`

- 完成条件：
  - refactor LLVM body emitter 已只消费 P5 state graph / boundary contract；
  - 所有 boundary 与显式控制流都已在 LLVM CFG 中闭合；
  - backend 不再承担第二套高层 effect lowering 语义工作。
- 依赖：P6-T02R，P6-T02c，P6-T02d，P6-T02e，P6-T02f，P6-T02g，P6-T02h，P6-T02i，P6-T02j，P6-T02kR，P6-T02l，P6-T02m，P5-T07a
- 完成记录：
  - 2026-05-03：开始实现时发现 blocker。当前 `ResumeSiteEffectFacts` 只发布 continuation schema / out-step 语义，但 `P6-T02` 现有 ABI query 还没有把 `Continuation.resume(...)` surface lowering contract 显式发布成 LLVM-level call target / query 映射；若直接继续 `P6-T03`，backend 将不得不在现场猜测 `resume` 入口或绕回 legacy resume lowering。
  - 因此新增前置任务 `P6-T02c`，先补齐 continuation surface-resume ABI/query handoff，再继续本任务。
  - 2026-05-03：继续实现时发现第二个 blocker。当前 `LateLoweredCallBoundaryLowering` 虽发布了 `CallSiteTarget` / `CallTargetMode` / `invoke_args_tuple_ty` / callee `StepSchema`，但 refactor LLVM ABI query 仍只按 callable version 发布 static `dynamic_entry/direct_entry` 签名，没有 runtime callable value -> canonical dynamic `invoke(args_tuple) -> Step_F` 的 authoritative LLVM query。若直接继续 `P6-T03`，backend 将不得不回 `CallKind::{Closure, FunValue, Virtual, Interface}` / legacy callable wrapper 现场重建 ABI，或把范围错误缩窄成只支持 `KnownInstance`。
  - 因此新增前置任务 `P6-T02d`，先补齐 canonical dynamic-invoke callable-object ABI/query contract，再继续本任务。
  - 2026-05-03：继续实现时发现第三个 blocker。`P5-T07a` 虽为 pure caller call boundary 新增了 `consumed_runtime_error_case`，但当前 handoff 只告诉 P6“callee 返回了哪条 compiler-generated ordinary runtime-error case”，并没有发布 caller-local 的 lowerable 控制流合同：state graph / boundary map / resume-state map 中都没有 dedicated synthetic path，ABI/query 层也没有说明 backend 应如何在不扩大 caller surface、也不发明 hidden trap 的前提下处理该 case。若直接继续 `P6-T03`，backend 只能现场猜测 pure caller 的 runtime-error 传播路径，违背本阶段 contract-first 约束。
  - 因此新增前置任务 `P6-T02e`，先把 pure caller call boundary 本地消费 compiler-generated runtime-error case 的 lowering contract 显式发布出来，再继续本任务。
  - 2026-05-03：继续进入 whole-body emitter 设计时发现第四个 blocker。当前 `crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 的 `materialize_dynamic_invoke_layouts(...)` 仍只扫描 `boundary_map` 里的 call boundary，因此 `RefactorAbiQuery` 只能为 effectful call boundary 发布 dynamic invoke query；但 `P6-T03` 必须 lower 整个 `LateLoweredState.source_slices()`，straight-line state slice 里仍可能出现 non-boundary 的 `CallKind::{Closure, FunValue, Virtual, Interface}`。若直接继续本任务，backend 仍会被迫在 source-slice lowering 现场回 legacy callable wrapper / dispatch ABI，或临时发明一套未发布的 invoke 规则，违背本阶段 contract-first 边界。
  - 因此新增前置任务 `P6-T02f`，先把 straight-line source-slice non-boundary dynamic call 的 callable-object ABI/query contract 显式发布出来，再继续本任务。
  - 2026-05-03：继续核对 actual dynamic call lowering 时发现第五个 blocker。虽然 `P6-T02d` / `P6-T02f` 已经发布了 dynamic invoke query，但 runtime callable carrier 仍没有 authoritative 地指向这套 refactor target：`crates/scoopc/src/llvm/codegen/{mir_body.rs,closure/mod.rs}` 仍把 closure object `fn_ptr` 写成普通 lambda/top-level LLVM function 指针，`crates/scoopc/src/llvm/codegen/gc.rs` 仍用 `declare_top_level_fun(...)` 的普通符号去填 class vtable / interface itable method 槽位。若直接继续 `P6-T03`，backend 仍会被迫在现场把 legacy 普通 ABI remap 到 refactor dynamic entry，或借壳旧 closure/vtable/itable dispatch helper，违背 `P6-T02d` / `P6-T02f` 明确禁止的 contract-first 边界。
  - 因此新增前置任务 `P6-T02g`，先把 callable carrier -> canonical dynamic entry 的 published contract 接到 closure/vtable/itable materialization，再继续本任务。
  - 2026-05-03：继续设计 `LocalRuntimeError` synthetic state lowering 时发现第六个 blocker。当前 late-lowered / ABI handoff 只为 pure caller call boundary 发布了 `input_case_tag` / `payload_abi` / `target_state`，但 `LateLoweredStateTerminator::LocalRuntimeError` 本身仍只携带 `payload_tuple_ty`，没有 authoritative terminal action 来说明 backend 应把这条 ordinary runtime-error 结束到哪条已发布语义路径。若直接继续 `P6-T03`，backend 将不得不现场决定是走 local catch、显式 outward emission，还是 runtime fatal path，违背 `P6-T02e` 本应建立的 contract-first 边界。
  - 因此新增前置任务 `P6-T02h`，先把 `LocalRuntimeError` synthetic terminal state 的 authoritative lowering/runtime contract 显式发布出来，再继续本任务。
  - 2026-05-03：继续真正接入 refactor body emitter 时发现新的 blocker。`P6-T03` 需要直接消费 P5/P6 handoff 中的 synthetic `invoke_args_tuple_ty` 与 `ResumeSurface::*` / `CallSurface::*` source type；但当前 LLVM codegen 的 value-lowering helper 仍默认这些类型已经存在于 legacy `hir::LoweredHir.types` / `CgTy` 键空间中。实际试接 direct entry / surface wrapper 时，synthetic carrier 会触发“无法映射到 codegen `TypeStore`”或错误地按旧 `CgTy::Ref`/pointer 规则解码，等价于要求 backend 把 refactor handoff 类型临时回塞 legacy 类型层或现场猜 carrier shape。
  - 因此新增前置任务 `P6-T02i`，先发布并实现 authoritative 的 synthetic invoke-carrier / source-type ABI value lowering contract，再继续本任务。
  - 2026-05-03：继续推进 handled path 时发现新的 blocker。`LateLoweredStateTerminator::HandleDispatch` 当前只显式发布了 `body_state` / `arm_states` / `finally_state` / `exit_state` 与系统槽位名字（`StateTag` / `CompletionTag`），但并没有 authoritative 地发布“body 子图完成后如何通过 internal completion/state carrier 进入 arm/finally/exit”的 lowering contract。当前唯一可见的具体协议仍藏在 legacy `state_machine_emitter.rs` 的 backend-private magic tags（例如 `STATE_TAG_HANDLE_RETURNED` / `STATE_TAG_FUNCTION_RETURNED`）与对应 completion slot 约定中。若直接继续 `P6-T03`，refactor backend 将不得不借壳这些隐藏常量，或在现场重新发明一套 handle completion-state 协议，违背本阶段 contract-first / no-workaround 约束。
  - 因此新增前置任务 `P6-T02j`，先把 `HandleDispatch` / completion-state lowering contract 显式发布到 late-lowered + LLVM query handoff 中，再继续本任务。
  - 2026-05-03：继续真正落地 `HandleDispatch` arm entry lowering 时发现新的 blocker。当前 handoff 虽已发布 handled case -> arm state / completion-state / payload carrier 等 contract，但还没有 authoritative 发布 arm payload binder / escape continuation binder 绑定：`HandleBinder { site_id, ordinal }` 仍无法区分同一 handle site 的不同 arm，continuation binder 也没有 published query。若直接继续 `P6-T03`，backend 将不得不回 canonical MIR `TerminatorKind::Handle { binder_locals, continuation_local, .. }` 或 HIR 现场恢复 arm entry shape，直接违反本任务“不得回 `mir::Body`/shape source 猜边界语义”的约束。
  - 因此新增前置任务 `P6-T02k` / `P6-T02kR`，先把 `HandleDispatch` arm payload binder / continuation binder contract authoritative 发布到 late-lowered + LLVM query handoff 中，再继续本任务。
  - 2026-05-03：继续真正设计 handle 子图 lowering 时发现新的 blocker。当前 `HandleDispatch` handoff 虽已发布 `body_state` / `arm_states` / `finally_state` / `exit_state`、handled case -> arm、pending completion、以及 arm binder，但还没有 authoritative 发布“哪些 states / boundaries 属于该 handle 的 body / arm / finally region，以及某个 boundary/outward case 是否应被当前 handle 本地消费”的稳定 routing query。以 `effect_resume_if_else_branch_single_perform.scoop` 为例，body 内 perform boundary `bd2` 需要命中 handled case `c0` 后转入 arm `st3`，并把 continuation 恢复点固定到 `st9`；若没有这层 published routing，backend 只能在 P6 现场重新遍历 state graph 或回 MIR 恢复 handle 子图归属，违背本阶段“只翻译已发布 handoff”的边界。
  - 因此新增前置任务 `P6-T02l`，先发布 `HandleDispatch` state-region / boundary-consumption contract，再继续本任务。
  - 2026-05-03：继续把 `Resume` boundary 与 surface `k.resume(...)` 真正接到 body emitter 时发现新的 blocker。当前 handoff 已发布 `ContinuationSchemaId -> surface-resume symbol/signature`，但还没有 authoritative 发布“shared surface-resume schema symbol 如何到达 owner-specific resume implementation”的 dispatch contract；`ContinuationSchemaId` 仍按 `(resume_tuple_ty, answer_ty, out_step_schema, surface_ty)` canonical 去重，允许被多个 continuation object / owner callable 复用。若直接继续 `P6-T03`，backend 将不得不在 surface-resume body 或 resume boundary lowering 现场扫描 raw continuation object / method 列表，或按 runtime type/header/符号名临时发明 dispatch 规则，直接违反 `P6-T02c` 已禁止的边界。
  - 因此新增前置任务 `P6-T02m`，先发布 continuation surface-resume -> owner dispatch contract，再继续本任务。

## P6-T03R：Review LLVM body lowering，确认 backend 只翻译 state graph，而不再重做 segmentation / frame lifting / shape 推断

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.16, §5.5, §8
  - [`PLAN.md`](./PLAN.md) §2/P6
- 重点：
  - LLVM body emitter 是否只消费 P5 late-lowered representation；
  - owner-state / resume-state / `impl_plan` 是否直接来自 P5，而非 backend 现场重做；
  - runtime error 是否仍以普通 `Step_F` case 传播；
  - 是否仍然避免了 statement-only / tail-resume / 单 handle 等形状特判主线。
- 必须检查的文件/位置：
  - 新增的 `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs`
  - 新增的 `crates/scoopc/src/llvm/codegen/effect_refactor/calls.rs`
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_bridge.rs`
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
  - P5 late-lowered representation 定义位置

- 验证：
  - 重新运行 P6-T03 的全部测试与命令；
  - 额外搜索：
    - `rg "build_unified_lowering_contract|effect_analysis_ctx|current_call_site\(|effect_op_call_sites|hir::HandleExpr|Span|tail-resume|single perform" crates/scoopc/src/llvm/codegen/effect_refactor crates/scoopc/src/effect_refactor_pipeline`
  - 要求：
    - 允许命中：legacy 模块、测试、注释；
    - 不允许命中：refactor LLVM 主实现仍依赖这些旧入口作为 semantic source of truth。

- 完成条件：
  - review 能明确说明：LLVM backend 已只承担“把 P5 合同翻译成 LLVM”的职责；
  - 可进入 P6-T04。
- 依赖：P6-T03
- 完成记录：
  - （执行时填写）

## P6-T04：接通 GC roots / stackmaps / runtime 语义，并锁定 dropped continuation、runtime error 与 Managed ABI 边界

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.7, §5.3.8, §5.3.9, §5.5.6, §8
  - 当前实现参考：
    - `crates/scoopc/src/llvm/pipeline.rs`
    - `crates/scoopc/src/llvm/stackmap.rs`
    - `crates/scoopc/src/llvm/codegen/gc.rs`
    - `crates/scoopc/src/llvm/codegen/runtime_symbols.rs`
    - `crates/scoopc/src/llvm/codegen/runtime_abi.rs`
    - `crates/scoopc/src/llvm/codegen/effect/contract.rs`（只能作 legacy 对照）
    - `crates/scoopc/src/llvm/codegen/effect/state_machine_bridge.rs`（只能作 legacy 对照）
- 目标：
  - 让 refactor LLVM 新路径在 GC/runtime 层面对 P5 合同端到端闭合；
  - 保证 frame/continuation/`Step` payload 在 moving GC 与 stackmap 环境下可正确追踪；
  - 同时把 dropped continuation、runtime error ordinary effect、Managed ABI/extern 边界的语义锁死到 backend 行为中。

- 必须实现的内容：
  1. 把 refactor 生成的 frame object / continuation object / `Step_F` payload 接入现有 GC object/runtime 约定。
     - 至少要处理：
       - object header / RTTI / trace/scan 所需元信息
       - allocation path
       - 需要被 GC 看见的 ref fields
       - 需要跨 safepoint/statepoint 更新的 roots
     - continuation 捕获到的引用必须遵守普通 GC 可达性规则；
     - 明确禁止：引入 continuation 专用生命周期 hack 或“掉引用时自动继续执行剩余计算”的语义。
  2. 让 refactor 路径生成的 effectful/state-machine 函数进入现有 LLVM pass pipeline / stackmap 路径。
     - 至少要确保：
       - 相关函数拥有现有 pipeline 需要的属性/前提；
       - frame slots、continuation captures、resume carriers、`Step_F` payload 中的 GC ref 在当前支持的根模型下可追踪；
       - 若当前 moving GC 只支持可写回 spill-slot roots，则 refactor path 必须生成与该假设兼容的根形态，或显式扩展支持，而不是静默留 bug。
  3. 把 ordinary runtime error 继续 lower 成普通 outward effect 分支。
     - `ContinuationAlreadyResumed` 等路径必须通过普通 `Step_F` / case dispatch 暴露，而非 trap-only 或 hidden outcome channel；
     - 如果为了性能引入 fast path，必须保持与普通 outward case 语义等价，并在代码注释/测试中可解释。
  4. 锁定 dropped continuation 语义。
     - dropped continuation 只能表示“剩余语言级计算被放弃”；
     - 任何尚未执行到的 pending `finally` / cleanup 都不得因为对象析构、GC、或 runtime helper 而再次执行；
     - `cleanup hook` 只能继续作为 runtime/GC 内部机制存在，不能被 refactor effect backend 编织成 continuation 语义的一部分。
  5. 锁定 Managed ABI / extern 边界。
     - refactor LLVM 路径不得让 `Step_F` / continuation object / resume interfaces 穿过 Managed ABI 或 extern callback；
     - 若当前这类边界只支持 pure callback / 普通显式错误码模型，则必须在新路径保持一致；
     - 若 refactor 路径碰到 effectful extern 场景，应维持与当前仓库一致的明确拒绝/诊断，而非偷偷 fallback 到 legacy contract。
  6. 禁止 refactor 新路径继续依赖 legacy handler-stack / outcome runtime contract。
      - 新路径生成的 LLVM IR 不应以这些 runtime 调用作为 correctness 前提：
        - `scoop_effect_handler_stack_top`
        - `scoop_effect_handler_stack_swap_top`
        - `scoop_effect_outcome_consume_current`
        - 等价 legacy effect-outcome / handler-stack runtime calls
     - 若某个 runtime helper 仅做完全中立的对象/分配/GC 事务，允许保留；
     - 但 effect propagation 本身必须由 `Step_F` / continuation / state graph 模型表达。
  7. 为 refactor GC/runtime integration 增加显式 LLVM IR 断言与运行时断言。
     - 至少要能验证：
       - 某些样本 emitted IR 中不再包含 legacy handler-stack calls；
       - moving GC / stackmap 环境下 effect/continuation 样本仍正确；
       - dropped continuation 不执行剩余 cleanup/finally；
       - one-shot / runtime error 行为仍正确。

- 必须遵从的约束：
  - 禁止让 refactor 新路径以 `LegacyEffectBoundary`、`EffectSignal`、`EffectOutcome`、handler-stack top 交换为 correctness 基础。
  - 禁止把 `cleanup hook` 当成 dropped continuation 语义的一部分。
  - 禁止因为当前阶段不是 full regression，就跳过 moving GC / stackmap / runtime 根正确性验证。
  - 禁止让 `Step_F` / continuation / effect context 穿过 Managed ABI / extern 边界。

- 验证：
  1. 新增/更新单元测试，推荐命名：
     - `refactor_llvm_gc_roots_*`
     - `refactor_llvm_stackmap_*`
     - `refactor_llvm_dropped_continuation_*`
     - `refactor_llvm_managed_abi_boundary_*`
  2. 新增/更新 build fixtures，推荐至少包括：
     - `tests/fixtures/build/effect_refactor_no_legacy_handler_stack_calls.scoop`
       - 目标：锁定 refactor emitted IR 不再依赖 legacy handler-stack / outcome calls
     - `tests/fixtures/build/effect_refactor_runtime_error_is_step_case.scoop`
       - 目标：锁定 runtime error 仍表现为普通 `Step_F` case
  3. 运行：
     - `cargo test -p scoopc refactor_llvm_gc_roots`
     - `cargo test -p scoopc refactor_llvm_stackmap`
     - `cargo test -p scoopc refactor_llvm_dropped_continuation`
     - `cargo test -p scoopc refactor_llvm_managed_abi_boundary`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_no_legacy_handler_stack_calls.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_runtime_error_is_step_case.scoop`
     - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/gc_continuation_cross_thread_resume_with_objects.scoop`
     - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/gc_continuation_multi_thread_concurrent_alloc_resume.scoop`
     - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/runtime_gc/effect_outer_mutable_state_body_writeback_basic.scoop`

- 完成条件：
  - refactor LLVM 新路径已在 GC/runtime/stackmap 层面对 `Step` / continuation / dropped continuation / runtime error / Managed ABI 边界端到端闭合；
  - 新路径 emitted IR 不再以 legacy handler-stack / outcome runtime contract 为 correctness 前提；
  - 后续 P6-T05 只需锁定定向验证矩阵并冻结 P6 -> P7 handoff。
- 依赖：P6-T03R
- 完成记录：
  - （执行时填写）

## P6-T04R：Review GC/runtime 集成，确认没有残留 legacy handler-stack 依赖，也没有错误的 dropped-continuation / FFI 语义

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.7, §5.3.8, §5.3.9, §8
  - [`PLAN.md`](./PLAN.md) §2/P6
- 重点：
  - refactor 路径是否已经把 GC root / stackmap / object reachability 正确接通；
  - dropped continuation 是否只表现为“剩余计算被放弃”；
  - runtime error 是否仍然是普通 effect 分支；
  - Managed ABI / extern 是否仍保持 pure-only 边界；
  - 是否已经摆脱 legacy handler-stack / outcome runtime contract。
- 必须检查的文件/位置：
  - 新增的 `crates/scoopc/src/llvm/codegen/effect_refactor/gc.rs`
  - 新增的 `crates/scoopc/src/llvm/codegen/effect_refactor/runtime.rs`
  - `crates/scoopc/src/llvm/pipeline.rs`
  - `crates/scoopc/src/llvm/stackmap.rs`
  - `crates/scoopc/src/llvm/codegen/runtime_symbols.rs`
  - `crates/scoopc/src/llvm/codegen/effect/contract.rs`

- 验证：
  - 重新运行 P6-T04 的全部测试与命令；
  - 额外搜索：
    - `rg "scoop_effect_handler_stack|scoop_effect_outcome|LegacyEffectBoundary|cleanup hook|Managed ABI|extern" crates/scoopc/src/llvm/codegen/effect_refactor crates/scoopc/src/effect_refactor_pipeline crates/scoopc/src/llvm/codegen/effect`
  - 要求：
    - 允许命中：legacy 模块、注释、测试；
    - 不允许命中：refactor LLVM 主实现仍以这些 legacy/TLS/cleanup-hook 语义作为 correctness 前提。

- 完成条件：
  - review 能明确说明：GC/runtime/FFI 语义边界已经按设计锁定；
  - 可进入 P6-T05。
- 依赖：P6-T04
- 完成记录：
  - （执行时填写）

## P6-T05：建立 refactor LLVM 定向 build/run-pass/runtime_gc 验证矩阵，并冻结 P6 -> P7 handoff contract

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6，§2/P7
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.9, §4.16, §5.2, §5.3.7-§5.3.9, §5.5, §8
  - 当前 CLI/fixture 入口参考：
    - `crates/scoop/src/commands/build.rs`
    - `crates/scoop/src/commands/run.rs`
    - `crates/scoop/src/fixtures/mod.rs`
    - `crates/scoop/src/fixtures/expectations.rs`
- 目标：
  - 用仓库现有的 build / run-pass / runtime_gc fixture 相位，为 refactor LLVM 新路径建立一套定向且可重复的验证矩阵；
  - 同时把“P7 只切主线并跑 full regression，不再重做 LLVM effect backend 设计”的 handoff contract 固化到代码与测试中。

- 必须实现的内容：
  1. 使用现有 build fixture phase 锁定 LLVM IR 形状。
     - 本任务优先复用 `tests/fixtures/build/**`；
     - 非必要禁止再新增新的 fixture phase；
     - 至少要覆盖：
       - `NoOutward`
       - `SingleCase`
       - `CanonicalFull`
       - direct `perform` / `handle` / `resume`
       - dynamic callable fallback
       - continuation one-shot / runtime error
       - `Unit` 零载荷 case
       - emitted IR 不含 legacy handler-stack / outcome calls
  2. 使用现有 run-pass fixture phase 锁定端到端执行语义。
     - 至少要覆盖：
       - direct/indirect effect call
       - continuation capture / resume
       - dynamic callable `invoke`
       - dropped continuation 语义
       - nested handle outward/self-contained 相关行为
     - 允许直接复用现有 effect 相关 run-pass fixtures；
     - 若现有样本不能精确覆盖某个 P6 关键形状，则新增最小样本并在完成记录中说明为何必须新增。
  3. 使用现有 runtime_gc fixture phase 锁定 moving-GC / verify-roots 语义。
     - 至少要覆盖：
       - frame / continuation capture roots
       - cross-thread or delayed resume 下的对象可达性
       - effect body writeback / mutable state
       - `Step_F` payload / resume payload 中的 ref roots
     - 允许复用现有 runtime_gc fixtures；
     - 若缺少 effect-specific moving-GC 覆盖，新增最小补充样本。
  4. 明确 P6 -> P7 handoff contract。
     - 必须在代码注释或等价文档实体中明确写出：
       - P6 已完成 refactor LLVM codegen 路径对接；
       - P7 的工作只包括：切换默认 selector、保留短期 legacy 参数、执行 full regression；
       - P7 不得重新设计 `Step` ABI、continuation ABI、LLVM state-graph lowering、GC root 模型、或 runtime error/dropped continuation 语义；
       - legacy 路径在 P7 之前仍继续保留，但不再是 refactor correctness 的隐式兜底。
  5. 确保所有 refactor build/run-pass/runtime_gc 验证共享同一 LLVM stage。
     - `build --emit-llvm`
     - `build` 产出可执行文件
     - `run`
     - `scoop test --fixtures tests/fixtures/build/...`
     - `scoop test --fixtures tests/fixtures/run-pass/...`
     - `scoop test --fixtures tests/fixtures/runtime_gc/...`
     必须都通过同一 refactor LLVM stage/helper，而不是各自拼不同 backend 入口。
  6. 若某些验证依赖 opt level 差异，至少锁定一组 `O0` 与一组较高优化级别样本，证明：
     - refactor LLVM 新路径在不同 opt level 下仍走同一 backend；
     - 差异只来自既有 `impl_plan` / P5 post-opt / LLVM pass pipeline，而不是切回 legacy 或第二条 lowering 通道。

- 必须遵从的约束：
  - 禁止为 P6 再新增“专供 refactor LLVM 用的隐藏测试入口”，而绕过现有 build/run/fixture 通道。
  - 禁止把 build fixtures 变成 legacy/refactor 双重输出拼接比较；refactor 验证应直接断言自己的 emitted IR / 执行结果。
  - 禁止在 P6-T05 才补高层 effect lowering 逻辑；本任务只负责验证矩阵与 handoff 收口。
  - 禁止把 P7 要做的 selector flip / full regression 提前在 P6 执行。

- 验证：
  1. 运行新增/更新的 build fixtures，至少包括：
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_step_enum_single_case.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_no_outward_complete_only.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_no_legacy_handler_stack_calls.scoop`
  2. 运行定向 run-pass fixtures，至少包括：
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/continuation_resume_surface_named_tuple_and_unit_basic.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_escape_continuation_resume_later_exit.scoop`
  3. 运行定向 runtime_gc / GC env 验证，至少包括：
     - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/runtime_gc/effect_outer_mutable_state_body_writeback_basic.scoop`
     - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/gc_continuation_cross_thread_resume_with_objects.scoop`
     - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/gc_continuation_multi_thread_concurrent_alloc_resume.scoop`
  4. 额外 CLI smoke：
     - `cargo run -p scoop -- --effect-pipeline refactor build --emit-llvm tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop -o /tmp/p6_refactor_resume.ll`
     - `cargo run -p scoop -- --effect-pipeline refactor run tests/fixtures/run-pass/continuation_resume_surface_named_tuple_and_unit_basic.scoop`
  5. 抽样 legacy 不受影响：
     - `cargo run -p scoop -- --effect-pipeline legacy test --fixtures tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop`

- 完成条件：
  - 仓库中已经有一套可重复运行的 refactor LLVM 定向验证矩阵；
  - `build` / `run` / build fixtures / run-pass / runtime_gc 在 refactor 模式下都共享同一 LLVM stage；
  - P6 -> P7 handoff contract 已通过代码与测试锁定。
- 依赖：P6-T04R
- 完成记录：
  - （执行时填写）

## P6-T05R：Review P6 阶段退出条件，确认 P7 只需切主线并执行 full regression

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6，§2/P7，§3
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.9, §4.16, §5.2, §5.3.7-§5.3.9, §5.5, §8
- 重点：
  - refactor LLVM codegen stage 是否已独立存在，并成为 build/run/fixture 的共同入口；
  - `Step_F` / frame / continuation object / resume interfaces 的 LLVM 合同是否已闭合；
  - body lowering 是否只翻译 P5 state graph，而不再重做高层 effect lowering；
  - GC/runtime/stackmap/dropped continuation/runtime error/Managed ABI 边界是否已锁定；
  - 定向 build/run-pass/runtime_gc 矩阵是否已建立；
  - P7 是否已经可以只做 selector flip + full regression，而不再新增 effect backend 设计工作。

- 验证：
  - 重新运行 P6-T01 ~ P6-T05 的全部定向测试与命令；
  - 不再额外执行 `cargo test -p scoop` / `cargo test -p scoopc` 全 crate 测试；这些 broad regression 留到 P7 统一执行。

- 完成条件：
  - review 能明确说明：P6 已完成“LLVM codegen 新路径对接（仍不切主线）”的阶段目标；
  - P7 可以在不重新讨论 LLVM effect backend 架构、ABI、或 GC/runtime 语义的前提下，直接进入默认主线切换与 full regression。
- 依赖：P6-T05
- 完成记录：
  - （执行时填写）
