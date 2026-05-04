# TODO（P6：LLVM codegen 新路径对接 Part 2 / 未完成任务）

> 生成时间：2026-05-02  
> 设计基线：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 前置条件：`TODO-P5.md` 已完整完成；refactor late-lowering stage、`dump-effect-lowered`、以及 P5 -> P6 handoff contract 已存在并稳定；P5 产出的 late-lowered representation 已成为 LLVM 前唯一允许消费的 effect/continuation 中层合同。  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 本阶段目标：把 P5 产出的 late-lowered representation 接到新的 LLVM codegen 路径；在不切默认主线的前提下，让 `--effect-pipeline refactor` 下的 `build` / `run` / `--emit-llvm` / `--emit-obj` / `--emit-asm` 能端到端生成正确 IR 和可运行程序，同时保持“backend 只翻译 P5 state graph / frame schema / boundary contract，而不再重新做高层 effect lowering 设计”的边界。
> 拆分说明：已完成任务（`P6-T01` ~ `P6-T02ma`）见 [`TODO-P6-part1.md`](./TODO-P6-part1.md)；当前文件保留未完成任务与继续推进所需的全局约束。

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
  - authoritative 的 resume 语义主键必须是 `ContinuationSchemaId` / `CaseTag` / `ConcreteOpKey` 或其等价 published contract；
  - 每个 method 的参数类型由 `ContinuationSchema.resume_tuple_ty` 决定；
  - 每个 method 的返回类型统一为同一个 `Step_F<T>`；
  - 若实现中保留按 effect family 分组的 internal resume interface / vtable，它只能作为 packing/object-side lookup 层；
  - 对不可能合法调用到的方法，允许 body 为 `unreachable`；
  - 若保留这层 packing，则不能在接口或对象定义中删掉这些方法。
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

## 已完成前置任务参考

- 已完成任务见 [`TODO-P6-part1.md`](./TODO-P6-part1.md)。
- 当前未完成链路按 `P6-T02n -> P6-T02o -> P6-T02p -> P6-T02qa -> P6-T02q -> P6-T02qb -> P6-T02qc -> P6-T03 -> P6-T03R -> P6-T04 -> P6-T04R -> P6-T05 -> P6-T05R` 推进。

## [DONE] P6-T02m：发布 continuation surface-resume -> owner dispatch contract，禁止 P6-T03 在 backend 现场扫描 continuation object 或猜 owner callable

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
- 依赖：P6-T02c，P6-T02ma
- 完成记录：
  - 2026-05-03：尝试把 `ContinuationSchemaId` 直接收口到单一 `ResumeInterfaceId/CaseTag/object lookup` 时发现 blocker。当前 handoff 还没有 authoritative 发布 schema 对应的 dispatch-source inventory：
    - `effect_refactor_step_enum_single_case.scoop` 中，同一 `k0` 同时对应 `ko1` 的 `c0/c1` 两个 surface case，但 only-one reachable method shell 被 `ImplPlan::SingleCase(c0)` 保留；
    - `effect_resume_if_else_branch_single_perform.scoop` 中，resume site 直接需要 `k3` 的 surface-resume symbol，而 `k3` 并不存在于任何 continuation object surface/method shell；同时 handle binder 仍要求 `k0` 具备 published surface-resume source。
  - 这说明 `P6-T02m` 不能只在 LLVM query 层补一个 schema -> method 选择器；必须先把 shared-schema / resume-site-only / handle-binder schema 的 authoritative dispatch-source inventory 显式发布出来。
  - 因此新增前置任务 `P6-T02ma`，先补齐 dispatch-source inventory，再继续本任务。
  - 2026-05-04：在 `crates/scoopc/src/llvm/codegen/effect_refactor/types.rs` / `layout.rs` 新增 `RefactorContinuationSurfaceResumeDispatchLayout` 查询层：
    - 每个非 `Unreachable` 的 `ContinuationSchemaId` 现在都会发布唯一 owner trampoline contract `__scoop_refactor_surface_resume_owner_dispatch__*`；
    - `ContinuationObjectMethod` schema 额外发布 `method_targets[]`，显式列出 object-side `interface_id + field_index + case_tag + vtable_index` lookup；
    - `ResumeBoundaryOnly` / `HandleContinuationBinderOnly` schema 则通过 owner trampoline 公开 resume-site 列表或 handle-binder route 列表。
  - 2026-05-04：根据真实 fixture 修正了 contract 形状，不再错误假设“同一 schema 只能对应唯一 reachable method”。`tests/fixtures/effect_facts/dynamic_fallback_widening.scoop` 中共享 schema `k0` 现在会 authoritative 地保留同一 owner/object 下的多 method lookup（`ri0::c0`、`ri1::c1`），同时仍收口到单一 owner trampoline；backend 不再需要扫描 raw continuation object 或猜 owner callable。
  - 2026-05-04：补上 fail-fast：若 `ContinuationObjectMethod` 缺失 reachable method target、若 object-side lookup / published method layout 漂移、或若多个 continuation object 共享同一 schema，则在 ABI materialization 阶段显式拒绝，而不是把歧义留给 `P6-T03`。
- 已运行验证：
  - `cargo test -p scoopc refactor_llvm_surface_resume_layout`
  - `cargo test -p scoopc refactor_llvm_surface_resume_dispatch_layout`
  - `cargo test -p scoopc refactor_llvm_continuation_layout`
  - `cargo test -p scoopc refactor_llvm_`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## [DONE] P6-T02n：清理 refactor LLVM ABI/query 的 resume 主键，降级 effect-level resume interface 为 packing 层

- 参考：
  - [`PLAN.md`](./PLAN.md) §0，§2/P5，§2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.2-§5.3.6, §5.5, §8
  - [`TODO-P5.md`](./TODO-P5.md) `P5-T07b`
  - [`TODO-P6-part1.md`](./TODO-P6-part1.md) `P6-T02c`
  - 当前实现参考：
    - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs`
    - `crates/scoopc/src/effect_lowered/ir.rs`
- 背景：
  - 当前 P6 已经有一批 resume 相关 ABI/query 明显是按 per-op/per-schema authoritative contract 在走：`RefactorContinuationSurfaceResumeLayout` 与 `RefactorContinuationSurfaceResumeDispatchLayout` 都按 `ContinuationSchemaId` 发布，surface-resume shared symbol 的 owner dispatch 也已经从 `ContinuationSchemaId` 起步；
  - 但与此同时，LLVM query 里仍保留了较强的 effect-level interface 外壳：`RefactorResumeInterfaceLayout` 按 effect family 组织 method，`RefactorContinuationSurfaceResumeMethodLookup` 仍显式携带 `ResumeInterfaceId + field_index + case_tag + vtable_index`，continuation object layout 也仍把 interface field/vtable 当作主叙事之一；
  - 若不在进入 `P6-T03` 前先清掉这层主次混淆，body emitter 很容易继续把 `ResumeInterfaceId` / effect family 当成恢复 resume 语义的起点，而不是把它当作 object-side packing 细节，从而把我们刚确认的问题延续到 LLVM body lowering 阶段。

- 目标：
  - 在 P6 ABI/query 层先明确分开 authoritative lookup 与 packing lookup；
  - 让 `P6-T03` 以后只把 `ContinuationSchemaId` / `CaseTag` / `ConcreteOpKey` / 已发布 owner dispatch contract 当作 resume 语义入口；
  - 若仓库继续保留 `ResumeInterfaceId` / effect-level interface layout，它只能服务 continuation object field/vtable packing 与 object-side method lookup，而不能再充当 backend 的 semantic source of truth。

- 必须实现的内容：
  1. 清理 `RefactorAbiQuery` 的 resume 主键叙事。
     - surface-resume、resume boundary、continuation object method target、以及任何会被 `P6-T03` body emitter 直接消费的 query，都必须明确说明：authoritative key 是 `ContinuationSchemaId` / `CaseTag` / `ConcreteOpKey` 或其等价 published contract；
     - `ResumeInterfaceId` 若继续出现，必须被标注为 object-side packing lookup，而不是 resume 语义入口。
  2. 为 body lowering 准备 direct authoritative lookup。
     - `P6-T03` 所需的 resume 相关 query，必须能在不先扫描 effect family 分组或 interface 列表的前提下，直接从 published contract 找到：
       - surface-resume symbol/signature；
       - owner trampoline / dispatch target；
       - object-side method lookup（若仍保留 interface/vtable packing）。
  3. 限缩 effect-level interface 的职责边界。
     - `RefactorResumeInterfaceLayout` 若保留，只能继续承担：
       - vtable 物理布局；
       - object field index / method slot index；
       - 与 authoritative case 集对齐的完整性校验；
     - 明确禁止：让 `P6-T03` 通过 effect family 分组或 `ResumeInterfaceId` 反向恢复 `ContinuationSchemaId` / per-op contract。
  4. 为主次漂移补齐 fail-fast 与测试。
     - 若某个 body-prep/helper/query 只有在先拿到 `ResumeInterfaceId` / effect family 后才能恢复语义，则必须显式 fail fast，而不是把这种倒推默许成 backend 合法职责；
     - 定向测试至少要覆盖 shared `ContinuationSchemaId`、同 owner 下多 method target、以及 `ResumeBoundaryOnly` / `HandleContinuationBinderOnly` 路径不依赖 effect-level interface 分组恢复语义。

- 必须遵从的约束：
  - 禁止仅因为当前 object layout 仍含 vtable field，就把 `ResumeInterfaceId` 继续当成 P6 authoritative 主键。
  - 禁止让 `P6-T03` 的 body emitter、surface-resume body 或其它 lowering helper 扫描 raw interface 列表、比较 effect family 名字、或依赖 `ri*::c*` 的偶然分布来补语义。
  - 允许保留 effect-level interface layout，但只能把它当作 packing 层，而不是 contract-first 边界中的语义本体。

- 验证：
  1. `cargo test -p scoopc refactor_llvm_surface_resume_layout`
  2. `cargo test -p scoopc refactor_llvm_surface_resume_dispatch_layout`
  3. `cargo test -p scoopc refactor_llvm_continuation_layout`
  4. `cargo test -p scoopc refactor_llvm_`
  5. `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  6. `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`

- 完成条件：
  - `P6-T03` 已可直接把 `ContinuationSchemaId` / `CaseTag` / published owner dispatch contract 当作 resume 语义入口；
  - `ResumeInterfaceId` / effect family 在 P6 ABI/query 层已被明确降级为 packing/object-lookup 细节；
  - backend 不再需要通过 effect-level interface 分组倒推 per-op/per-schema contract。
- 依赖：P6-T02m，P6-T02ma，P5-T07b
- 完成记录：
  - 2026-05-04：完成 `P6-T02n`。`crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs` 现已把 LLVM ABI/query 的公开叙事改为 `resume packing`：`RefactorCallableLayout` / `RefactorAbiQuery` / `RefactorContinuationObjectLayout` 分别改用 `resume_packings()`、`resume_packing_layout(...)`、`field_index_for_packing(...)`，`RefactorContinuationSurfaceResumeMethodLookup` / `RefactorResumeMethodLayout` 也改用 `packing_interface_id` / `packing_field_index` 命名，明确 `ResumeInterfaceId` 只承担 object-side packing/vtable lookup，而不再充当 resume 语义主键。
  - 2026-05-04：新增 `RefactorAbiQuery::surface_resume_method_layout(...)`，让后续 `P6-T03` body lowering 能直接从已发布的 `ContinuationSchemaId -> owner dispatch -> method lookup` contract 回查 surface-resume method layout，并在 query 层对 packing lookup 漂移显式 fail fast；backend 不再需要先扫描 effect family 分组或 interface 列表来恢复 per-op/per-schema contract。
  - 2026-05-04：同步更新 continuation/surface-resume 相关测试与断言文案，覆盖 shared `ContinuationSchemaId`、同 owner 多 method target、`ResumeBoundaryOnly` / `HandleContinuationBinderOnly` 路径，以及 unit-payload ABI，不再把 effect-level interface 当成查询入口。`PLAN.md` 无需改动。
- 已运行验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc refactor_llvm_surface_resume_layout`
  - `cargo test -p scoopc refactor_llvm_surface_resume_dispatch_layout`
  - `cargo test -p scoopc refactor_llvm_continuation_layout`
  - `cargo test -p scoopc refactor_llvm_`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## [DONE] P6-T02o：发布 statement/terminator anchored boundary operand contract，禁止 P6-T03 在 body emitter 现场回 raw MIR statement/terminator 恢复 `Call / Perform / Resume` 输入

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.5.2-§5.5.6
  - `crates/scoopc/src/effect_lowered/{segment,materialize,ir}.rs`
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs`
- 背景：
  - `P6-T02d` / `P6-T02e` / `P6-T02f` / `P6-T02j` / `P6-T02m` / `P6-T02n` 已经把 dynamic invoke、local runtime error、handle dispatch、surface-resume owner dispatch 与 resume packing 主键等 LLVM ABI/query contract 发布到 refactor handoff；
  - 但当前 `LateLoweredCallBoundaryLowering` / `LateLoweredPerformBoundaryLowering` / `LateLoweredResumeBoundaryLowering` 只发布了 boundary 语义、dispatch 计划与 emitted-step/runtime-error pairing，没有 authoritative 发布 boundary 本身的 lowering 输入：
    - call/resume 的 ordered args source contract；
    - dynamic call 的 carrier source；
    - resume 的 continuation source；
    - perform payload source；
    - 以及 statement-anchored boundary 在 source slice 中究竟消费到哪一条语句的 contract；
  - 与此同时，`LateLoweredStateSlice` 只保留了 `block + stmts[start..end] (+ 可选 terminator)`；若直接继续 `P6-T03`，body emitter 只能回 raw `mir::Body` / `mir::Rvalue::Call` / `mir::TerminatorKind::Perform` / `mir::CallKind::Resume` 现场恢复 boundary 输入与 anchor 位置，等价于把 boundary lowering 所需的 authoritative 事实重新留给 backend 自己猜。

- 目标：
  - 在进入 `P6-T03` 前，先把 statement/terminator anchored boundary 的 lowering 输入 contract 显式发布到 late-lowered / LLVM query handoff；
  - 让后续 refactor body emitter 可以只消费 published contract + straight-line source slices，就完成 boundary lowering，而不需要再把 raw MIR boundary statement/terminator 当成语义事实来源。

- 必须实现的内容：
  1. 为 `Call` boundary 发布 authoritative operand/source contract。
     - 至少要覆盖：
       - direct known-instance call 的 ordered args source；
       - closure / fun-value / virtual / interface dynamic call 的 carrier source 与 ordered args source；
       - 与已发布 `invoke_args_tuple_ty` / `CallTargetMode` / callable layout 的一致性校验；
     - 明确禁止：让 `P6-T03` 通过 raw `mir::Rvalue::Call` 重新决定 callee kind、receiver source、或实参次序。
  2. 为 `Perform` boundary 发布 authoritative payload/source contract。
     - payload 为 `()` / 零载荷时也必须显式发布，而不是让 backend 通过“args 恰好为空”临时推断；
     - 若 perform payload 需要从 source locals / temporaries 读取，相关 source contract 必须在 handoff 中显式可查。
  3. 为 `Resume` boundary 发布 authoritative continuation-source / ordered resume-arg contract。
     - 至少要覆盖：
       - continuation receiver source；
       - ordered resume args source；
       - 与已发布 `ContinuationSchemaId -> surface-resume layout` / owner dispatch contract 的一致性校验；
     - 明确禁止：让 `P6-T03` 通过 raw `mir::CallKind::Resume` 恢复 continuation local 或参数顺序。
  4. 为 statement/terminator anchored boundary 发布 source-slice consumption contract。
     - 至少要能让 `P6-T03` 明确知道：
       - 某个 boundary 是消费 statement anchor 还是 terminator anchor；
       - statement-anchored boundary 在所属 source slice 中是否占用最后一条语句，以及该语句必须由 boundary lowering 而不是 generic straight-line statement lowering 消费；
     - 明确禁止：让 backend 通过 raw MIR block 扫描 / “最后一条看起来像 call/perform/resume” 的 shape 规则临时恢复 anchor。
  5. 对缺失、歧义或漂移的 boundary operand contract fail fast。
     - 至少包括：
       - boundary 已发布 dispatch/emission，但缺少 operand source contract；
       - ordered args/payload source 与 published tuple ABI 漂移；
       - 同一 boundary source 被重复发布为多个不兼容的 anchor/operand contract；
       - `P6-T03` 若只消费 published contract 仍无法唯一决定 boundary lowering 输入。
  6. 补充定向测试与回归。
     - 推荐命名：
       - `refactor_effect_lowered_boundary_operand_contract_*`
       - `refactor_llvm_boundary_operand_contract_*`
     - 至少覆盖：
       - statement-anchored direct call boundary；
       - non-`KnownInstance` call boundary；
       - perform payload contract；
       - resume boundary continuation/arg contract；
       - 缺失/歧义 contract 时显式拒绝。

- 必须遵从的约束：
  - 禁止把 raw `mir::Body` / `mir::Rvalue::Call` / `mir::TerminatorKind::Perform` / `mir::CallKind::Resume` 当作 `P6-T03` 的 authoritative boundary 语义来源；
  - 允许 `P6-T03` 继续使用 canonical MIR/source slice 作为 straight-line code lowering 的载体；但 boundary lowering 所需的 callee/receiver/args/payload/anchor facts，必须先以 published contract 形式显式给出；
  - 禁止把“boundary 恰好是 source slice 最后一条语句/唯一语句”的当前样本形状当成默认规则，除非这本身就是显式发布并经过校验的 contract。

- 验证：
  - `cargo test -p scoopc refactor_effect_lowered_boundary_operand_contract`
  - `cargo test -p scoopc refactor_llvm_boundary_operand_contract`
  - `cargo test -p scoopc refactor_llvm_`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`

- 完成条件：
  - `Call / Perform / Resume` boundary 的 lowering 输入已成为 published handoff，而不是留给 backend 现场恢复；
  - statement/terminator anchored boundary 的 source-slice consumption contract 已显式可查；
  - `P6-T03` 可以在不把 raw MIR boundary statement/terminator 当作语义事实来源的前提下继续 body lowering。
- 依赖：P6-T02n
- 完成记录：
  - 2026-05-04：在 `crates/scoopc/src/effect_lowered/ir.rs` 新增 `LateLoweredOperandSource` / `LateLoweredBoundarySourceConsumption`，并把 `Call / Perform / Resume` boundary 的 operand contract 显式挂到各自 lowering 上；`effect_lowered/materialize.rs` 现在会在 P5/P6 handoff 处读取 canonical MIR，把 ordered args、dynamic carrier、resume continuation、perform payload 与 statement/terminator anchor 一次性物化进 late-lowered contract，同时对缺失 anchor、重复 anchor、result-local 漂移与 source-count 漂移 fail fast。
  - 2026-05-04：`effect_lowered/dump.rs` / `opt.rs` 已同步保留并渲染这些 contract，`dump-effect-lowered` 现在会直接显示 boundary 的 anchor、carrier、ordered args / payload sources；其中显式处理了 `Unit` zero-arg sugar 与“单一 tuple surface arg -> tuple carrier”两类 contract，不再把它们误判成“空 source”或“扁平多 source”。
  - 2026-05-04：在 `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs` 新增 `Refactor{Call,Perform,Resume}BoundaryOperandLayout` 查询层，并让 ABI materializer 在发布 query 时校验 source-slice anchor、source type ABI、dynamic carrier 存在性、resume surface contract、以及 ordered source 与 published carrier 的一致性；backend 后续可直接消费这批 published contract，而不必回 raw MIR statement/terminator 恢复输入。
- 已运行验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc refactor_effect_lowered_boundary_operand_contract`
  - `cargo test -p scoopc refactor_llvm_boundary_operand_contract`
  - `cargo test -p scoopc refactor_llvm_`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## [DONE] P6-T02p：发布 callable version 选择 contract，禁止 P6-T03 在 backend 现场按 `root_fqn` / 单壳层假定选择 late-lowered body

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.9-§4.13, §5.5, §8
  - `crates/scoopc/src/effect_lowered/ir.rs`
  - `crates/scoopc/src/effect_facts/facts.rs`
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs`
- 背景：
  - P5 authoritative handoff 已把 callable identity 固定到 `LateLoweredBodyVersionKey`，同一 `root_fqn` 在不同 `allowed_row` / `impl_plan` / `needs_reentry` 下允许对应多个 late-lowered callable version；`layout.rs` 甚至已经会在 `root_counts[root_fqn] > 1` 时为不同 callable shell 生成不同的 LLVM symbol stem。
  - 但当前 P6 ABI/query 仍没有把“如何 authoritative 地从 surface callable / runtime callable carrier / known-instance call target 选到具体 callable version”显式发布出来：
    - `RefactorCallableLayout` / `RefactorAbiQuery` 主要按 `StepSchemaId` 暴露 callable shell；
    - `layout.rs::callable_layout_by_root_fqn(...)` 在同一 `root_fqn` 出现多个 published shell 时会直接拒绝，说明 carrier target 仍默认“每个 root 只有唯一 callable shell”；
    - `CallSiteEffectFacts` 当前只发布 `CallSiteTarget::{KnownInstance, CandidateSet, DynamicFallback}`、`invoke_args_tuple_ty` 与 `callee_schema`，没有显式说明 runtime callable carrier / known-instance target 应落到哪一个 callable version。
  - 若直接继续 `P6-T03`，一旦同一 surface callable 物化出多个 late-lowered versions，backend 就只能：
    - 按 `root_fqn`、遍历顺序、或“当前恰好 `StepSchemaId` 全局唯一”的未发布约定去碰运气；
    - 或把 runtime callable carrier / known-instance target 错误地塌缩成唯一壳层。
  - 这会让 body emitter、carrier publication、dynamic invoke 与 owner dispatch 再次依赖 backend 现场猜测，而不是 published contract，因此必须先补齐这层 version-selection handoff。

- 目标：
  - 为 refactor LLVM backend 发布 authoritative 的 callable version 选择 contract；
  - 让 `P6-T03` 能只消费已发布 handoff，在“当前 callable version”“callee callable version”“runtime callable carrier target”三类场景下唯一确定正确的 late-lowered body，而不再假定 `root_fqn -> 唯一 callable shell`。

- 必须实现的内容：
  1. 为 callable shell / ABI query 发布稳定的 callable version identity。
     - authoritative key 必须至少覆盖 `LateLoweredBodyVersionKey`，或一个等价且已明确冻结的 version selector；
     - query 必须能从该 key 直接回查：
       - direct entry
       - dynamic entry
       - frame layout
       - continuation object / owner dispatch 所需 shell
     - 明确禁止让 `P6-T03` 通过 `root_fqn` 唯一性假设或遍历顺序恢复 version。
  2. 把 runtime callable carrier / known-instance target 与 callable version 选择 contract 接通。
     - closure object / class vtable / interface itable 发布的 canonical dynamic entry target，必须显式说明对应哪个 callable version；
     - `CallSiteTarget::KnownInstance` 若当前只发布了 `InstanceKey + callee_schema`，则必须把这对信息显式冻结为 authoritative version selector，或最小化扩展 handoff 以直接发布 callee `LateLoweredBodyVersionKey`；
     - 明确禁止让 backend 在 body emitter 现场通过 `root_fqn`、symbol 名字、或“先找到哪个 shell 就用哪个”来补选版本。
  3. 对缺失、歧义或漂移的 version-selection contract fail fast。
     - 至少包括：
       - 同一 `root_fqn` 存在多个 published callable shell，但 carrier target / known-instance query 仍没有唯一 version selector；
       - published callable version identity 与 `callee_schema` / entry shell / continuation object 漂移；
       - `P6-T03` 若只消费 ABI query / late-lowered contract 仍无法唯一决定当前或 callee callable version。
  4. 补充定向测试与回归。
     - 至少覆盖：
       - 同一 surface callable 物化多个 late-lowered versions 时，ABI query 仍能唯一回查各自 callable shell；
       - runtime callable carrier publication 不再因为“同 root 多 shell”而回到 `callable_layout_by_root_fqn(...)` 的唯一性假设；
       - 缺失 version-selection contract 时显式拒绝，而不是留给 `P6-T03` 现场猜测。

- 必须遵从的约束：
  - 禁止把 `root_fqn` 当成 callable version 的 authoritative 主键。
  - 禁止依赖 `StepSchemaId`“当前恰好全局唯一”的隐含实现细节，除非这层关系被显式发布并冻结为 contract。
  - 禁止把 runtime callable carrier target 重新回退到 legacy callable wrapper 或其它 backend-private 规则。

- 验证：
  1. 新增/更新单元测试，推荐命名：
     - `refactor_llvm_callable_version_query_*`
     - `refactor_llvm_callable_carrier_version_selection_*`
     - `refactor_llvm_known_instance_version_selection_*`
  2. 新增/更新 build fixture，推荐至少包括：
     - `tests/fixtures/build/effect_refactor_multi_version_callable_emit_llvm.scoop`
       - 目标：锁定同一 surface callable 发布多个 late-lowered versions 时，carrier/query 仍能唯一选中正确 callable shell
  3. 运行：
      - `cargo test -p scoopc refactor_llvm_callable_version_query`
      - `cargo test -p scoopc refactor_llvm_callable_carrier_version_selection`
      - `cargo test -p scoopc refactor_llvm_known_instance_version_selection`
      - `cargo test -p scoopc refactor_llvm_`
      - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`

- 完成条件：
  - callable version 的 authoritative 选择 contract 已作为 P5/P6 handoff 一部分发布；
  - runtime callable carrier / known-instance target / 当前 body emitter 入口都不再依赖 `root_fqn -> 唯一 shell` 假设；
  - `P6-T03` 可以只消费 published contract 唯一决定当前/被调 callable version。
- 依赖：P6-T02o
- 完成记录：
  - 2026-05-04：在 `crates/scoopc/src/llvm/codegen/effect_refactor/types.rs` 中把 callable shell 的 published identity 从“仅有 `step_schema`”扩展为显式携带 `LateLoweredBodyVersionKey`：`RefactorCallableLayout` 新增 `body_version_key()` / `surface_instance()`，`RefactorAbiQuery` 新增 `callable_layout_by_version_key(...)`，并把 `callable_layout_by_root_fqn(...)` 收口为仅供单-version 场景使用的便利查询；一旦同 root 存在多个 published callable version，会显式要求调用方改用 version-key 查询，而不是继续依赖遍历顺序或 root 唯一性假设。
  - 2026-05-04：在 `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs` 中补齐了两类 authoritative selector：
    - `body_version_key -> step_schema/callable shell` 索引；
    - `(InstanceKey, callee StepSchemaId) -> LateLoweredBodyVersionKey` 的 known-instance selector。
    `RefactorAbiQuery::call_target_layout(...)` 现在会通过这两个已发布 contract 回查被调 callable version，并在 instance、callee schema、dynamic-entry signature 漂移时 fail fast。
  - 2026-05-04：为 runtime callable carrier 发布 `RefactorCallableCarrierTargetLayout`，把 closure/vtable/itable target 明确绑定到 `body_version_key + step_schema + symbol_name`。carrier target 发布仍只覆盖“当前 late-lowered program 已发布的 callable roots”；这样既保留了 refactor contract 的 published selection 语义，也避免把 `class_itables` / `class_vtables` 中未进入当前程序的 `Hashable.hash` 一类槽位误判成 P6-T02p blocker。
  - 2026-05-04：补齐定向测试：`refactor_llvm_callable_version_query_*`、`refactor_llvm_known_instance_version_selection_*`、`refactor_llvm_callable_carrier_version_selection_*`。其中 duplicate-version coverage 通过 `layout.rs` 内部 helper 人工构造“同 root 多 callable version”场景，锁定 carrier target 会显式拒绝缺失 authoritative selector 的发布，而不是把歧义留给 `P6-T03` backend 现场猜测。现有 build fixture `tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop` 继续覆盖 emitted LLVM 中 closure/vtable/itable carrier 指向 refactor dynamic entry 的 contract。
  - 2026-05-04：`PLAN.md` 无需改动。
- 已运行验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc refactor_llvm_callable_version_query`
  - `cargo test -p scoopc refactor_llvm_callable_carrier_version_selection`
  - `cargo test -p scoopc refactor_llvm_known_instance_version_selection`
  - `cargo test -p scoopc refactor_llvm_`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## [DONE] P6-T02qa：发布 escaped continuation aggregate/member write-read provenance contract，禁止 P6-T02q 在 late-lowered/ABI materialization 现场从 unresolved assign-lhs TODO 或 source shape 猜 `cell.k` 回读 continuation 的底层 surface route

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.5.2-§5.5.7, §8
  - [`TODO-P6-part2.md`](./TODO-P6-part2.md) `P6-T02kR`, `P6-T02o`, `P6-T02q`
  - `crates/scoopc/src/mir/{mod,lower}.rs`
  - `crates/scoopc/src/effect_lowered/{ir,materialize}.rs`
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs`
- 背景：
  - `P6-T02o` 已让 `Resume` boundary 发布 continuation source local 与 ordered resume args；`P6-T02kR` 也已让 handle continuation binder 发布 authoritative binder local/schema/object contract；
  - 但当前 canonical MIR 仍没有把 aggregate/member assignment lower 成可追踪 contract：`crates/scoopc/src/mir/lower.rs::lower_assign_stmt(...)` 只覆盖 `local = expr`，像 `cell.k = Some(k)` / `cell.k = none_k` 这类写入仍会落成 `StatementKind::Todo("assign lhs lowering pending")`；
  - `tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop` 真实暴露了这层缺口：handle arm binder 发布 `local10 -> continuation_schema=k3`，但后续 site25/site30/site35/site40 的 `Resume` boundary continuation local（`local95` / `local116` / `local137` / `local158`）只来自 `MemberAccess(Cell.k)` + `PatternExtract(Some[0])`，当前 handoff 并没有 authoritative 地说明这些 readback local 究竟承接了哪条已发布 continuation source route；
  - 若不先补齐这层 write-read provenance，`P6-T02q` 就只能依赖 unresolved assign-lhs TODO、source span、member 名字、或 continuation nominal type 去猜 `cell.k` 回读 continuation 的真实 surface route，直接违反 contract-first 边界。

- 目标：
  - 在进入 `P6-T02q` 之前，先把 compiler-owned continuation 值穿过 aggregate/member write/read 时的 authoritative provenance contract 显式发布出来；
  - 让后续 wrapper-schema bridge 能只消费已发布 handoff，就把 `cell.k` 这类 readback continuation authoritative 地接回原始 surface route，而不再回 MIR/source shape 猜测。

- 必须实现的内容：
  1. 为 continuation-bearing aggregate/member assignment 发布稳定 write contract。
     - 至少要覆盖 `cell.k = Some(k)` / `cell.k = none_k` 这类当前会落成 `assign lhs lowering pending` 的路径；
     - published contract 至少要能表达：写入目标的 member identity、写入值来源，以及写入发生在何处；
     - 明确禁止继续把这类路径留成只有 `Todo("assign lhs lowering pending")` 的不透明 source shape，然后期待 P6 现场自行恢复 provenance。
  2. 为后续 member readback / variant extract 发布 continuation provenance contract。
     - 至少要覆盖 `MemberAccess(Cell.k)` 后接 `PatternExtract(Some[0])` 的 canonical readback 路径；
     - handoff 必须能 authoritative 地说明：readback continuation local 承接了哪条已发布 continuation source route（例如 handle continuation binder 对应的 `ContinuationSchemaId` / shared surface schema / object route）；
     - 若同一 readback 可能承接多个互不兼容 route，必须显式 fail fast，而不是留给 `P6-T02q` 现场猜。
  3. 把这层 provenance 接到 late-lowered / LLVM query handoff。
     - 推荐落点包括但不限于：canonical MIR published contract、`LateLoweredResumeBoundaryLowering`、`LateLoweredHandleContinuationBinder`、`RefactorResumeBoundaryOperandLayout`，或一个等价的 compiler-owned provenance query；
     - 但禁止只在 `P6-T02q` / `P6-T03` body emitter 内偷偷缓存“local -> continuation route”私表。
  4. 对缺失、歧义或漂移的 continuation write-read provenance fail fast。
     - 至少包括：
       - continuation 经 aggregate/member 写入后，后续 readback 仍没有 published provenance；
       - published write target 与 readback member 不一致；
       - readback continuation local 可能对应多个互不兼容的 authoritative surface route；
       - `P6-T02q` 若只消费 published handoff 仍无法唯一决定 readback continuation 的底层 route。
  5. 补充定向测试与回归。
     - 至少覆盖：
       - `effect_multi_escape_indirect_direct_while.scoop` 中 `cell.k` 的 write/read path 会显式发布 continuation provenance，而不是只留下 `assign lhs lowering pending`；
       - 缺失或歧义 provenance 时显式拒绝；
       - 新 contract 不依赖 `Cell.k` 这个 fixture 私名，而是对 continuation-bearing aggregate/member write/read 作为一般 contract 生效。

- 必须遵从的约束：
  - 禁止对 `cell.k`、`Some(k)`、或某个具体 fixture block/site 做 task-private 特判。
  - 禁止让 `P6-T02q` / `P6-T03` 通过 unresolved `Todo("assign lhs lowering pending")`、source span、member 文本名、或 continuation nominal type 反推 provenance。
  - 若实现需要扩展 canonical MIR 对非 local 赋值 lhs 的 published 表达，必须把该表达作为 compiler-owned contract 显式发布，而不是把责任继续留给后续 backend。

- 验证：
  - `cargo test -p scoopc refactor_effect_lowered_`
  - `cargo test -p scoopc refactor_llvm_`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-mir tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`

- 完成条件：
  - compiler-owned continuation 值穿过 aggregate/member write/read 后，late-lowered/P6 handoff 仍能 authoritative 地保留其 surface-route provenance；
  - `P6-T02q` 可以只消费 published contract，把 `cell.k` 这类 readback continuation 唯一桥接回底层 surface route；
  - 缺失、歧义或漂移时会在 P5/P6 边界显式拒绝，而不是由 backend 现场猜测。
- 依赖：P6-T02kR，P6-T02o，P6-T02p
- 完成记录：
  - 2026-05-04：在 `crates/scoopc/src/mir/{mod,lower,materialize}.rs` 中新增 canonical `StoreMember` statement 与 `StoredContinuationRoutePublication` contract；`lower_assign_stmt(...)` 现在会对 member assignment 显式发布 receiver/member/value source，以及 continuation payload 穿过 wrapper/aggregate 的写入路径。`effect_multi_escape_indirect_direct_while.scoop` 中的 `cell.k = Some(k)` / `cell.k = none_k` 不再落成 `assign lhs lowering pending`。
  - 2026-05-04：在 `crates/scoopc/src/effect_lowered/{ir,materialize,dump}.rs` 中新增 `LateLoweredResumeBoundaryOperandContract::underlying_continuation_route()`，并实现 `PublishedContinuationProvenance` resolver：它会把 handle continuation binder seed、member write contract、member read、`PatternExtract(Some[0])` readback 串起来，为 resume boundary continuation local 发布 authoritative underlying route；缺失、路径不匹配、source-local 无 published route、或多条不兼容 route 时都会在 P5/P6 边界 fail fast。
  - 2026-05-04：在 `crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 中把该 provenance 接进 LLVM ABI/query 校验；若 resume boundary contract 引用的 underlying publication 未出现在 authoritative surface-resume dispatch inventory 中，会显式拒绝而不是留给 backend 现场猜测。
  - 2026-05-04：补齐定向回归：
    - `mir::lower::tests::dump_mir_publishes_member_write_contract_for_escape_continuation_cell`
    - `effect_lowered::materialize::tests::refactor_boundary_lowering_publishes_member_readback_resume_route`
    - `effect_lowered::materialize::tests::published_continuation_provenance_rejects_ambiguous_member_routes`
    - `llvm::codegen::effect_refactor::layout::tests::refactor_llvm_boundary_operand_contract_rejects_missing_underlying_continuation_route_publication`
    - `llvm::codegen::effect_refactor::layout::tests::refactor_llvm_boundary_operand_contract_resolves_perform_and_resume_sources`（扩展为覆盖 `effect_multi_escape_indirect_direct_while.scoop`）
  - 2026-05-04：已运行验证：
    - `cargo fmt --all`
    - `cargo test -p scoopc dump_mir_publishes_member_write_contract_for_escape_continuation_cell`
    - `cargo test -p scoopc published_continuation_provenance_rejects_ambiguous_member_routes`
    - `cargo test -p scoopc refactor_boundary_lowering_publishes_member_readback_resume_route`
    - `cargo test -p scoopc refactor_llvm_boundary_operand_contract_`
    - `cargo test -p scoopc refactor_effect_lowered_`
    - `cargo test -p scoopc refactor_llvm_`
    - `cargo run -p scoop -- --effect-pipeline refactor dump-mir tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
    - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
    - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
  - 2026-05-04：`PLAN.md` 无需改动。

## [DONE] P6-T02q：发布 resume-boundary wrapper -> underlying continuation surface route contract，禁止 P6-T03 在 backend 现场从 continuation local / source type 猜 `k.resume(...)` 实际调用的 schema

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.2-§5.3.6, §5.5.2-§5.5.7, §8
  - [`TODO-P6-part1.md`](./TODO-P6-part1.md) `P6-T02c`, `P6-T02m`, `P6-T02n`
  - `crates/scoopc/src/effect_facts/facts.rs`
  - `crates/scoopc/src/effect_lowered/ir.rs`
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs`
- 背景：
  - `P6-T02c` 已为 `Resume` boundary 发布 boundary-local `ContinuationSchemaId -> surface-resume layout`；`P6-T02m/n` 也已为 shared surface schema 发布 owner dispatch / packing contract；
  - 但当前 handoff 仍缺少一层更细的 authoritative bridge：当 `Resume` boundary 的 boundary-local schema 与 runtime continuation object 自身发布的 source-visible schema 不一致时，backend 还不知道“这个 boundary-local wrapper 到底应调用 runtime continuation object 上的哪条 surface route”；
  - `tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop` 已经真实暴露出这层缺口：
    - handle binder 把 `cell.k` 绑定为 `continuation_schema=k3`（`TODO-P6-part2.md` 当前 dump 中 `authoritative_surface_resume_dispatch_inventory` 可见 `k3 source=HandleContinuationBinderOnly`）；
    - 但后续显式 `k.resume(...)` site25/site30/site35/site40 的 `Resume` boundary 只发布了 boundary-local `continuation_schema=k5`，且 `k5 source=ResumeBoundaryOnly` 并没有 object-side method target；
    - `ResumeSiteEffectFacts` / `LateLoweredResumeBoundaryLowering` / `RefactorContinuationSurfaceResumeOwnerTrampolineLayout` 当前都只保留了 boundary-local `k5` 与 continuation source local，本身并没有 authoritative 地指出“这里实际应去调用 runtime continuation object 的哪条 published surface route（例如 `k3`）”。
  - 若直接继续 `P6-T03`，backend 只能：
    - 从 continuation local 的 raw source type / binder 来源 / object layout 反推真正的 surface schema；
    - 或在 `ResumeBoundaryOnly` owner trampoline 里现场扫描 continuation object / source shape 临时恢复 route；
    - 两者都直接违反本阶段 contract-first 边界。

- 目标：
  - 在进入 `P6-T03` 前，先把 `Resume` boundary-local wrapper schema 到 runtime continuation object 实际 surface route 的 authoritative bridge 显式发布出来；
  - 让后续 body emitter 能仅凭 published handoff，就正确 lower `k.resume(...)` 这类 boundary-local wrapper，而不再现场猜 runtime continuation object 的真实 schema / symbol / route。

- 必须实现的内容：
  1. 为 `Resume` boundary 发布“wrapper schema -> underlying continuation surface route”的 authoritative contract。
     - 至少要能让 `P6-T03` 直接查询到：
       - boundary-local wrapper 自己的 `ContinuationSchemaId`；
       - runtime continuation object 实际应调用的 published surface route（可表现为 underlying `ContinuationSchemaId`、shared surface symbol、owner trampoline，或等价稳定 bridge）；
       - 若 wrapper 本身直接承担全部 owner-specific resume 语义，也必须显式发布这条 bridge，而不是让 backend 靠 raw local/source type 猜测。
  2. 把这层 bridge 接到 late-lowered / LLVM query handoff。
     - 推荐落点：
       - `LateLoweredResumeBoundaryLowering`
       - `RefactorAbiQuery`
       - 或一个等价且已发布的 compiler-owned bridge query；
     - 但禁止只在 `P6-T03` body emitter 内部偷偷缓存一张“local -> schema”私表。
  3. 对缺失、歧义或漂移的 wrapper-route contract fail fast。
     - 至少包括：
       - boundary-local wrapper schema 已发布，但没有对应 underlying route；
       - 同一 `Resume` boundary continuation source 可能对应多个互不兼容的 underlying schema / symbol；
       - `P6-T03` 若只消费 published handoff 仍无法唯一决定 `k.resume(...)` 实际应调用哪条 surface route。
  4. 补充定向测试与回归。
     - 至少覆盖：
       - `effect_multi_escape_indirect_direct_while.scoop` 中 `cell.k` 的 published surface schema 与 site25/site30/site35/site40 的 boundary-local wrapper schema 可 authoritative 地桥接；
       - 缺失 bridge 时显式拒绝，而不是把责任留给 `P6-T03` backend 现场猜测。

- 必须遵从的约束：
  - 禁止让 backend 通过 continuation local 的 source type 文本、binder 变量来源、raw continuation object field 顺序、或 runtime header/type desc 猜 underlying surface schema；
  - 禁止假定 `Resume` boundary 的 boundary-local `ContinuationSchemaId` 一定等于 runtime continuation object 自己发布的 source-visible schema；
  - 禁止把这层关系继续埋在 owner trampoline 私有实现里而不先 published。

- 验证：
  - `cargo test -p scoopc refactor_llvm_surface_resume_dispatch_layout`
  - `cargo test -p scoopc refactor_llvm_boundary_operand_contract`
  - `cargo test -p scoopc refactor_llvm_`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`

- 完成条件：
  - `P6-T03` 可以仅凭 published handoff 唯一决定 boundary-local `k.resume(...)` wrapper 实际应调用的 runtime continuation surface route；
  - 缺失、歧义或漂移时会在 P5/P6 边界显式拒绝，而不是由 backend 现场猜测。
- 依赖：P6-T02m，P6-T02n，P6-T02o，P6-T02p，P6-T02qa
- 完成记录：
  - 2026-05-04：开始真正落地 `P6-T03` 时发现新的 blocker。当前 `Resume` boundary contract 仍只发布了 boundary-local wrapper schema，而没有 authoritative 发布它到 runtime continuation object 实际 surface route 的 bridge：`effect_multi_escape_indirect_direct_while.scoop` 中 handle binder 发布 `k3`，但后续 site25/site30/site35/site40 的 resume boundary 只发布 `k5`；`RefactorContinuationSurfaceResumeDispatchLayout` 对 `k5` 又是 `ResumeBoundaryOnly`、没有 object-side method target。若直接继续本任务，backend 必须回 continuation local/source type/shape 猜实际应调用的 surface route，违反 contract-first 约束。
  - 2026-05-04：继续追根后确认 blocker 更前移。`crates/scoopc/src/mir/lower.rs::lower_assign_stmt(...)` 当前只覆盖 `local = expr`，`cell.k = Some(k)` / `cell.k = none_k` 仍落成 `StatementKind::Todo("assign lhs lowering pending")`；因此 late-lowered/P6 handoff 只能看到 `MemberAccess(Cell.k)` + `PatternExtract(Some[0])` 生成的 continuation local，却看不到它与 handle binder `local10 -> k3` 之间的 authoritative write-read provenance。若不先补这层 contract，`P6-T02q` 仍会被迫通过 unresolved assign-lhs TODO 或 source shape 猜 route。为此新增前置任务 `P6-T02qa`。
  - 2026-05-04：`LateLoweredResumeBoundaryOperandContract` 现已把 `underlying_continuation_route` 升级为必填 published contract；`build_resume_boundary_operand_contract(...)` 会优先消费 `P6-T02qa` 发布的 member write/read provenance，把 boundary-local wrapper authoritative 地桥接回底层 continuation surface route。
  - 2026-05-04：若 resume operand 没有更深的 binder/member provenance，可直接发布 boundary 自身的 self-route（`ContinuationSchemaId + ResumeBoundary publication`），从而保证 `P6-T03` 后续不需要再回 continuation local/source type 猜测 `k.resume(...)` 的实际 surface schema。
  - 2026-05-04：`crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 现已把该 bridge 作为 ABI/query 校验的一部分：缺失 underlying route publication 或 publication 漂移时，P5/P6 边界会显式 fail fast；`dump-effect-lowered` 也会稳定打印 `underlying_route:`。
  - 2026-05-04：新增/更新定向测试覆盖 direct resume self-route、member readback bridge、以及 LLVM query 对缺失 publication 的拒绝；`effect_multi_escape_indirect_direct_while.scoop` 的 dump 已显示 site25/site30/site35/site40 从 boundary-local `k5` authoritative 地桥接到 handle binder 发布的 `k3`。
  - 2026-05-04：已运行验证：
    - `cargo test -p scoopc refactor_boundary_lowering_`
    - `cargo test -p scoopc refactor_llvm_surface_resume_dispatch_layout`
    - `cargo test -p scoopc refactor_llvm_boundary_operand_contract`
    - `cargo test -p scoopc refactor_llvm_`
    - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
    - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## [DONE] P6-T02qb：发布 cleanup/finally pending payload carrier contract，禁止 P6-T03 在 backend 现场发明 `ResumePayloadCarrier` 的 boxing / projection 规则

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.7, §5.3.9, §5.5.4-§5.5.7, §8
  - [`TODO-P6-part1.md`](./TODO-P6-part1.md) `P6-T02h`, `P6-T02j`, `P6-T02l`
  - `crates/scoopc/src/effect_lowered/{frame,materialize,ir}.rs`
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs`
- 背景：
  - `P6-T02j` / `P6-T02l` 已把 `HandleDispatch` 的 `CompletionTag`、`ResumePayloadCarrier`、pending completion 集合，以及 body/arm/finally/exit 的 region/routing contract 发布到 late-lowered + LLVM query handoff；
  - 但当前 handoff 仍只把 `SystemSlotKind::ResumePayloadCarrier` 固定成一个 `Any` typed system slot（`crates/scoopc/src/effect_lowered/frame.rs`），并没有继续发布“某个 pending outward / cleanup / finally path 的 typed case payload 应如何 authoritative 地装入这个 carrier、以及稍后如何再按 published contract 投影回具体 payload tuple”的 lowering 规则；
  - 这在 `handle_finally_boundary.scoop` / `dropped_continuation_abandons_remaining_work.scoop` 这类真实 `finally` / cleanup path 上已经成为直接 blocker：例如 `PropagateOutward(case)` 可能需要把 `Int` / tuple payload 穿过 cleanup state 与 `ResumeUnwind`，但当前 query 只告诉 backend 有一个 `Any` slot，并没有告诉它是否应 box、如何 box、何时 unbox、或是否应改走 typed per-case carrier；
  - 若直接继续 `P6-T03`，backend 只能：
    - 现场发明 `Int/Bool/Unit/tuple/ref -> Any` 的 boxing / projection 规则；
    - 或临时退回 raw word + gc_ref transport / legacy effect payload 语义；
    - 或按具体 fixture shape 偷偷为 `ResumeUnwind` / `finally` path 保留 backend-private 特判；
  - 以上都直接违反本阶段 contract-first / no-workaround 边界。

- 目标：
  - 在进入 `P6-T03` 前，先把 cleanup/finally/pending-outward path 所需的 payload carrier contract authoritative 地发布出来；
  - 让后续 body emitter 能只消费 published handoff，就正确把 typed case payload 穿过 `CompletionTag` / cleanup / `ResumeUnwind`，而不需要现场猜 `ResumePayloadCarrier` 的 boxing / projection 规则。

- 必须实现的内容：
  1. 为 pending completion / cleanup path 发布 authoritative payload transport contract。
     - 至少覆盖：
       - `LateLoweredHandlePendingCompletion::PropagateOutward(case_tag)`；
       - 与 `cleanup_state` / `ResumeUnwind` / `finally_complete_target` 相连、需要暂存 payload 后再继续 lowering 的 path；
       - typed payload tuple 如何在“boundary/arm/finally 现场”和“cleanup/ResumeUnwind/最终 outward emission”之间保持同一 contract。
     - 若继续复用 `ResumePayloadCarrier`，必须显式发布：
       - 哪些 payload 进入 carrier；
       - carrier 的 boxing / transport / projection 规则；
       - `Unit` / scalar / tuple / ref payload 的边界；
       - carrier 中哪些事实是 authoritative，哪些只是 layout/packing 细节。
     - 若改为 typed per-case carrier / frame slot，也必须把它 authoritative 地发布到 late-lowered + LLVM query handoff，而不是只在 backend 内部偷偷新建私表。
  2. 把该 contract 接到 LLVM query 与 verifier / fail-fast。
     - 至少要让 `P6-T03` 能直接查询到：
       - 某条 pending completion / cleanup path 应读取哪个 published payload transport；
       - `ResumeUnwind` / finally 结束后如何把 carrier authoritative 地还原成 typed payload；
       - 若最终要构造 outward `Step_F`，应使用哪个 published emission contract。
  3. 对缺失、歧义或漂移的 carrier contract fail fast。
     - 至少包括：
       - 存在 pending outward / cleanup path，但没有 published payload transport；
       - published carrier 需要 boxing/projection，但 handoff 没有 authoritative 规则；
       - `P6-T03` 若只消费 published handoff 仍无法唯一决定 payload transport / re-projection。
  4. 补充定向测试与回归。
     - 至少覆盖：
       - `handle_finally_boundary.scoop` 一类 pending completion/finally path 的 payload transport contract 会被稳定 dump/query；
       - `dropped_continuation_abandons_remaining_work.scoop` 一类 cleanup/drop path 在缺失 carrier contract 时显式拒绝，而不是由 backend 现场发明 transport；
       - LLVM query / verifier 会对缺失或漂移的 published carrier contract fail fast。

- 必须遵从的约束：
  - 禁止让 `P6-T03` 在 backend 现场临时决定 `Any` carrier 的 boxing / unboxing / pointer-word transport 规则；
  - 禁止把这层 payload transport 再借壳 legacy `EffectOutcome` / old resume payload channel；
  - 禁止仅针对某个 fixture/shape 私下缓存 `case_tag -> payload` 或 `cleanup state -> payload slot`，而不先把它作为 published contract 暴露。

- 验证：
  - `cargo test -p scoopc refactor_handle_dispatch_contract_`
  - `cargo test -p scoopc refactor_llvm_handle_dispatch`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/effect_lowered/handle_finally_boundary.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/effect_lowered/dropped_continuation_abandons_remaining_work.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

- 完成条件：
  - `P6-T03` 可以只凭 published handoff lower cleanup/finally/pending-outward payload transport；
  - backend 不再需要现场发明 `ResumePayloadCarrier` 的 boxing / projection / raw transport 规则；
  - 缺失、歧义或漂移时会在 P5/P6 边界显式拒绝。
- 依赖：P6-T02h，P6-T02j，P6-T02l
- 完成记录：
  - 2026-05-04：真正进入 `P6-T03` whole-body emitter 设计后确认新的未跟踪 blocker。当前 handoff 虽已发布 `CompletionTag` / `ResumePayloadCarrier` field index、pending completion 集合、以及 body/arm/finally/exit routing，但还没有 authoritative 发布“typed case payload 如何穿过 cleanup/finally/ResumeUnwind path”的 lowering contract。`ResumePayloadCarrier` 目前只是一格 `Any` system slot；对 `Int`/tuple 等非 ref payload，backend 若想继续实现 `PropagateOutward(case)` 或 cleanup 后的 outward/runtime-error emission，只能现场发明 boxing / projection 规则，违反 contract-first 边界。为此新增本前置任务，先把 payload carrier contract 发布清楚，再继续 `P6-T03`。
  - 2026-05-04：已改为发布 typed per-case pending payload transport contract，而不是让 `P6-T03` 继续依赖 `ResumePayloadCarrier` 的 backend-private boxing。late-lowered `FrameSchema` 新增 `HandlePendingPayload { site_id, case_tag }` 稳定 slot identity；`LateLoweredHandleDispatchContract` 新增 `pending_payload_transports`，把 `PropagateOutward(case)` 对应的 `payload_tuple_ty + frame_slot` authoritative 地发布给 P6。
  - 2026-05-04：LLVM ABI/query 已补齐对应发布与 fail-fast。`RefactorHandleDispatchLayout` 现在可直接按 `LateLoweredHandlePendingCompletion::PropagateOutward(case)` 查询 typed payload transport 的 frame field index；若缺失 published transport、outward emission、slot kind/ty、或 frame layout field，会在 P5/P6 handoff 物化阶段显式拒绝，而不是把歧义留给 `P6-T03` backend 现场猜测。
  - 2026-05-04：验证通过：`cargo test -p scoopc refactor_handle_dispatch_contract_ --no-fail-fast`、`cargo test -p scoopc refactor_llvm_handle_dispatch --no-fail-fast`、`cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/effect_lowered/handle_finally_boundary.scoop`、`cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/effect_lowered/dropped_continuation_abandons_remaining_work.scoop`、`cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`。

## [DONE] P6-T02qc：发布 shared surface-resume wrapper 的 owner-step -> wrapper-step 投影 contract，禁止 P6-T03 在 shared surface body 现场反推 inverse dispatch

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.2-§5.3.6, §5.5.2-§5.5.7, §8
  - [`TODO-P6-part2.md`](./TODO-P6-part2.md) `P6-T02q`, `P6-T03`
  - `crates/scoopc/src/effect_lowered/ir.rs`
  - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs`
- 背景：
  - `P6-T02q` 已把 boundary-local `ContinuationSchemaId` authoritative 地桥接到 runtime continuation object 实际应调用的 underlying surface route；
  - 但当前 handoff 仍只发布了“caller 侧如何消费 wrapper step”的 forward contract，没有发布“shared surface-resume wrapper 自身如何把 underlying owner step authoritative 地投影回 wrapper step”的 reverse/projection contract；
  - `tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop` 现在已经形成真实 blocker：
    - shared surface schema `k5` 的 published source 是 `ResumeBoundaryOnly`，`out_step_schema=s4`；
    - 它的 underlying route authoritative 地桥接到 handle-binder schema `k3`，而 `k3` 继续回到 owner callable `main` 的 `StepSchema s1`；
    - 当前 boundary lowering / dump 只发布了 caller 侧的 forward 消费：`Resume(site25/site30/site35/site40)` 观察 `dispatch_input_step_schema=s4`，并把 wrapper case `c0` forward 成 owner outward `c2`；
    - 对于要定义 `__scoop_refactor_surface_resume__k5` 的 `P6-T03` 而言，缺失的恰恰是相反方向：当 shared surface body 调用 underlying route 并拿到 owner step `s1` 后，究竟该如何 authoritative 地回投影成 wrapper step `s4`；当前 handoff 没有显式发布这层关系。
  - 若直接继续 `P6-T03`，backend 只能：
    - 现场反向推导 `LateLoweredStepDispatchPlan`；
    - 或按 case tag / `ConcreteOpKey` / boundary pairing 临时拼出 inverse mapping；
    - 这会把 shared surface-resume wrapper 的语义再次留给 backend 现场猜测，违反本阶段 contract-first 边界。

- 目标：
  - 在进入 `P6-T03` 前，先为 shared surface-resume wrapper 发布 authoritative 的 owner-step -> wrapper-step 投影 contract；
  - 让后续 LLVM body emitter 能仅凭 published handoff 定义 `ResumeBoundaryOnly` / 等价 wrapper schema 的 shared surface body，而不再现场反推 inverse dispatch。

- 必须实现的内容：
  1. 为 wrapper surface-resume schema 发布 authoritative projection contract。
     - 至少要覆盖：underlying route 返回 owner step 后，wrapper 的 `Complete` / outward case 应如何映射；
     - 可接受落点包括但不限于：
       - `LateLoweredResumeBoundaryLowering`
       - `LateLoweredContinuationRoute`
       - `LateLoweredSurfaceResumeDispatchInventoryEntry`
       - `RefactorAbiQuery`
       - 或一个等价的 compiler-owned published query；
     - 但明确禁止把这层关系继续留给 `P6-T03` 在 shared surface body 里自行“反向理解”现有 dispatch plan。
  2. 让缺失、歧义或漂移的 wrapper projection 在 P5/P6 边界 fail fast。
     - 至少包括：
       - wrapper schema 已桥接到 underlying route，但没有 published owner-step -> wrapper-step projection；
       - 多个 owner outward case 会塌缩到同一个 wrapper case，而 handoff 没有显式发布 authoritative 规则；
       - `P6-T03` 若只消费 published handoff，仍无法唯一决定 shared surface body 该返回哪条 wrapper case / payload / continuation。
  3. 补充 query / dump / regression。
     - 至少覆盖：
       - `effect_multi_escape_indirect_direct_while.scoop` 中 `k5 -> k3 -> owner s1` 的 shared surface wrapper 不再要求 backend 反推 inverse dispatch；
       - dump / query 能直接展示或校验这层 projection；
       - 缺失 projection 时显式拒绝，而不是把 shared surface body 语义留给 `P6-T03` 现场拼装。

- 必须遵从的约束：
  - 禁止让 `P6-T03` 通过反向遍历 `LateLoweredStepDispatchPlan`、比较 `CaseTag` / `ConcreteOpKey`、扫描 owner boundary list、或依赖当前 fixture 的 `in c0 -> out c2` 偶然形状来恢复 wrapper surface-resume 的返回语义；
  - 禁止把“caller 侧 forward dispatch 已存在”视作 shared surface body 反向 projection 已经显式发布；
  - 若 relation 需要 inversion，必须先把 inversion 本身 authoritative 地发布为 contract，而不是让 backend 自行推导。

- 验证：
  - `cargo test -p scoopc refactor_llvm_surface_resume_dispatch_layout`
  - `cargo test -p scoopc refactor_llvm_`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`

- 完成条件：
  - `P6-T03` 可以只凭 published handoff 定义 `ResumeBoundaryOnly` / 等价 shared surface-resume wrapper body；
  - owner step -> wrapper step 的 projection relation 已成为 authoritative contract，而不是 backend 现场反推；
  - 缺失、歧义或漂移时会在 P5/P6 边界显式拒绝。
- 依赖：P6-T02q
- 完成记录：
  - 2026-05-04：在真正实现 `P6-T03` shared surface-resume body 时确认新 blocker。当前 handoff 虽然已经 authoritative 地发布了 `k5 -> underlying route k3`，但并没有继续发布 `underlying owner step s1 -> wrapper step s4` 的投影 contract。`effect_multi_escape_indirect_direct_while.scoop` 中 site25/site30/site35/site40 的 `ResumeBoundaryOnly` wrapper 若继续由 `P6-T03` 落地，backend 只能通过反向推导现有 `dispatch_input_step_schema=s4` / `in c0 -> out c2` caller-side contract 来拼 shared surface body 的返回语义；这违反本阶段“不得把 effect/control contract 留给 backend 现场恢复”的约束。因此新增本前置任务，先把 wrapper projection contract 显式发布出来，再继续 `P6-T03`。
  - 2026-05-04：已在 `crates/scoopc/src/effect_lowered/ir.rs` 的 authoritative surface-resume dispatch inventory 上新增 `wrapper_projection` contract，显式发布 `underlying_route`、`owner_step_schema -> wrapper_step_schema`、以及 `complete` / outward case 的 owner -> wrapper 投影；`dump-effect-lowered` 现可直接展示 `k5` 的 `wrapper_projection`，后续 `P6-T03` 不再需要从 forward dispatch 反推 shared surface body 返回语义。
  - 2026-05-04：已在 `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs` 把该 contract 接到 owner trampoline query，并对缺失 published projection、derived/published 漂移、以及跨 resume site 的多候选歧义执行 fail-fast。验证通过：`cargo test -p scoopc refactor_surface_resume_dispatch_inventory_ --no-fail-fast`、`cargo test -p scoopc refactor_surface_resume_dispatch_dump_exposes_shared_wrapper_projection --no-fail-fast`、`cargo test -p scoopc refactor_llvm_surface_resume_dispatch_layout_ --no-fail-fast`、`cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`、`cargo fmt --all`、`cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`。

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
        - `Step_F` / continuation / invoke / surface-resume / published resume-method lookup LLVM signatures
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
        - 调用 continuation object 上已发布的 resume target；若实现仍保留 effect-level interface packing，也只能经由 object-side published method lookup 到达对应 method；
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
- 依赖：P6-T02R，P6-T02c，P6-T02d，P6-T02e，P6-T02f，P6-T02g，P6-T02h，P6-T02i，P6-T02j，P6-T02kR，P6-T02l，P6-T02m，P6-T02n，P6-T02o，P6-T02p，P6-T02q，P6-T02qb，P6-T02qc，P5-T07a，P5-T07b
- 完成记录：
  - 2026-05-04：继续真正实现 shared surface-resume body 时确认新的 blocker。当前 handoff 已能 authoritative 地桥接 `ResumeBoundaryOnly` wrapper schema 到 underlying route（例如 `effect_multi_escape_indirect_direct_while.scoop` 中 `k5 -> k3`），但还没有显式发布“underlying owner step -> wrapper step”的 projection contract。若继续实现 `__scoop_refactor_surface_resume__k5`，backend 只能反向推导现有 boundary dispatch（例如 `s4.c0 -> s1.c2`）来拼 wrapper body 返回语义，违反 contract-first 边界。因此新增前置任务 `P6-T02qc`，先发布 wrapper projection contract，再继续本任务。
  - 2026-05-04：继续真正落地 body emitter 时确认新的 blocker。当前 handoff 对 `HandleDispatch` cleanup/finally/pending-outward path 只发布了 `CompletionTag` / `ResumePayloadCarrier` 的 field index 与 pending completion 集合，但没有 authoritative 发布 typed case payload 如何穿过该 carrier。对 `handle_finally_boundary.scoop` / `dropped_continuation_abandons_remaining_work.scoop` 这类 path，backend 若继续实现就必须现场发明 `Any` boxing / projection 或 raw transport 规则。为此新增前置任务 `P6-T02qb`，先把 cleanup/finally pending payload carrier contract 显式发布出来，再继续本任务。
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
  - 2026-05-04：继续真正落地 body emitter 时发现新的 blocker。当前 late-lowered handoff 虽已发布 state graph / boundary semantics / dispatch plan，但 `LateLoweredCallBoundaryLowering` / `LateLoweredPerformBoundaryLowering` / `LateLoweredResumeBoundaryLowering` 仍没有 authoritative 发布 boundary lowering 的 operand/source contract：call/resume 的 ordered args source、dynamic call carrier source、resume continuation source、perform payload source，以及 statement-anchored boundary 在 source slice 中消费到哪一条语句的 contract 仍未显式 handoff。若直接继续 `P6-T03`，backend 只能回 raw `mir::Body` / `mir::Rvalue::Call` / `mir::TerminatorKind::Perform` / `mir::CallKind::Resume` 现场恢复 boundary 输入与 anchor 位置，这会重新把 boundary lowering 的语义事实留给 MIR shape，而不是 published contract。
  - 因此新增前置任务 `P6-T02o`，先发布 statement/terminator anchored boundary operand contract，再继续本任务。
  - 2026-05-04：继续核对 refactor callable shell / dynamic carrier target 时发现新的 blocker。P5 authoritative identity 已经是 `LateLoweredBodyVersionKey`，而 `layout.rs` 也已承认同一 `root_fqn` 可以发布多个 callable shell（会按 `schema*` 追加 symbol stem）；但当前 `RefactorAbiQuery` 仍主要按 `StepSchemaId` / `root_fqn` 暴露 callable layout，`layout.rs::callable_layout_by_root_fqn(...)` 仍把“同 root 只有唯一 shell”当成 carrier target 前提，`CallSiteEffectFacts` 也尚未显式发布 runtime callable carrier / known-instance target 应选择哪个 callable version 的 contract。若直接继续 `P6-T03`，backend 将不得不按 `root_fqn`、遍历顺序、或未发布的唯一性假设去碰运气选择 callee/current body version，违背本阶段 contract-first 边界。
  - 因此新增前置任务 `P6-T02p`，先把 callable version selection contract authoritative 发布到 late-lowered / LLVM query handoff，再继续本任务。
  - 2026-05-04：继续真正把 `Resume` boundary 接到 body emitter 时发现新的 blocker。当前 handoff 仍没有 authoritative 发布“boundary-local resume wrapper schema -> runtime continuation object 实际 surface route”的 bridge：`effect_multi_escape_indirect_direct_while.scoop` 中 handle binder 发布 `k3`，但 site25/site30/site35/site40 的 `Resume` boundary 只发布了 `k5`；`k5` 的 published dispatch 又是 `ResumeBoundaryOnly`，没有 object-side method target。若直接继续本任务，backend 只能回 continuation local / source type / runtime object shape 猜 `k.resume(...)` 实际应调用哪条 surface route，直接违反本阶段 contract-first 约束。
  - 因此新增前置任务 `P6-T02q`，先把 resume-boundary wrapper -> underlying continuation surface route contract 显式发布出来，再继续本任务。

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
