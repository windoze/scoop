# Effect Refactor Gaps

> 状态：基于当前工作树的缺口清单。
> 范围：只统计活跃实现与活跃测试/校验面；不把 `docs/archive/**` 里的历史文字当作实现状态。

## 当前状态

这轮删除已经把旧的 continuation/effect TLS 语义源头从活跃实现里物理拔掉：

- `runtime/c/scoop_runtime.c` 中旧的
  - `__scoop_effect_handler_stack_top`
  - `__scoop_effect_active`
  - `__scoop_effect_perform_slot`
  - `__scoop_callee_suspend_state`
  - `__scoop_continuation_resume_scope`
  相关实现段已删除。
- `crates/scoopc/src/llvm/codegen/effect/{mod,contract}.rs`
- `crates/scoopc/src/llvm/codegen/call/{dispatch,resume}.rs`
- 旧 runtime ABI/symbol 暴露面
- `sysroot/core.scoop` 中旧 `__scoop_effect_*` / slot surface

因此，**当前主要缺口不再是“旧 TLS 路径仍与新设计并存”**，而是：

- 旧 TLS 路径已经被硬删除；
- 但 `EFFECT_REFACTOR.md` 要求的显式 `EffectCtx` / `EffectOutcome` / `Step_F` / clean backend replacement 还没有补回；
- 当前仓库处于“legacy 已拆、target architecture 未接通”的中间态。

## 评判原则

本清单严格按 `EFFECT_REFACTOR.md` 中这几条原则审计：

1. 目标形态原则
   直接面向最终模型设计，不保留显然会被下一步推翻的兼容层。
2. 单管线原则
   所有优化级别共用同一条 lowering/codegen 管线。
3. 闭包原则
   下一阶段只能依赖上一阶段显式 facts/schema/table，不能回看 HIR/AST/旧缓存补语义。
4. Clean backend 原则
   P6 LLVM backend 必须自己拥有 entry ABI、state CFG、boundary、return/completion、continuation、frame、runtime error、GC/runtime contract。
5. 非 TLS 语义原则
   capture 链/handler context/propagation contract 不能再以 ambient TLS 为语义前提。

## 缺口清单

### 1. effectful callable 的显式 hidden ABI 还没有建立

违反原则：目标形态原则、非 TLS 语义原则、Clean backend 原则

当前证据：

- `crates/scoopc/src/llvm/codegen/mod.rs:2088-2145`
  - `declare_top_level_fun_with_symbol(...)` 仍只知道：
    - 普通参数
    - 可选 hidden sret
    - 可选单个 hidden `incoming_resume_token`
- `crates/scoopc/src/llvm/codegen/mod.rs:3705-3722`
  - `top_level_fun_uses_hidden_incoming_resume_token(...)` / `mir_fun_uses_hidden_incoming_resume_token(...)` 仍把“effectful callable 的额外 ABI”建模成单个 hidden token。

根据 `EFFECT_REFACTOR.md`，effectful callable / step / dispatch 的目标 ABI 应至少显式传递：

- `current_effect_ctx_ref`
- `incoming_resume_token_ref`
- `ScoopEffectOutcome *outcome`

当前缺口：

- 顶层函数 ABI 还没从“单 hidden token”升级成“显式 ctx + incoming token + outcome”；
- closure / function-value / dynamic invoke surface 也没有统一切到这套 ABI；
- 这导致删除 TLS 后，后端没有任何 authoritative 动态 effect 输入/输出协议。

### 2. clean backend 的 call/effect lowering 主体已经被拆空，但 replacement 不存在

违反原则：Clean backend 原则、目标形态原则

当前证据：

- 已删除：
  - `crates/scoopc/src/llvm/codegen/effect/mod.rs`
  - `crates/scoopc/src/llvm/codegen/effect/contract.rs`
  - `crates/scoopc/src/llvm/codegen/call/dispatch.rs`
  - `crates/scoopc/src/llvm/codegen/call/resume.rs`
- 但 `crates/scoopc/src/llvm/codegen/mod.rs` 仍保留大量 wrapper 入口，继续调用这些已不存在的实现，例如：
  - `declare_callee_resume_entry_function_impl`
  - `declare_top_level_fun_callee_resume_entry_impl`
  - `declare_top_level_fun_effect_call_wrapper_impl`
  - `ensure_top_level_fun_effect_call_wrapper_defined_impl`
  - `codegen_top_level_fun_effect_call_wrapper_impl`
  - `codegen_call_impl`
  - `codegen_top_level_fun_call_impl`
  - `try_codegen_class_vtable_call_impl`
  - `try_codegen_interface_itable_call_impl`
  - `codegen_funptr_value_call_impl`
  - `codegen_function_value_call_impl`
  - `codegen_function_value_call_from_closure_obj_impl`

`cargo check -p scoopc` 当前大量 `E0599` 直接说明这一点。

当前缺口：

- 旧 bridge 模块已删除；
- 但 clean backend 自己还没有新的 effect-call / dynamic-call / resume-call lowering 实现；
- `MainCodegen` 仍是“外壳在、语义主体没了”。

### 3. ordinary callee suspend / reentry 分析与 lowering 没有 replacement

违反原则：Clean backend 原则、闭包原则

当前证据：

`cargo check -p scoopc` 报错显示这些能力都缺失：

- `build_fun_callee_suspend_plan_impl`
- `build_ordinary_callee_suspend_plan`
- `declare_closure_callee_resume_entry_impl`
- `codegen_callee_resume_dispatch_impl`
- `codegen_callee_resume_entry_function_impl`
- `local_call_may_suspend_from_hir_ty`
- `hir_ty_declared_effectful`
- `known_fun_body_may_outward_effect`
- `function_value_expr_body_may_outward_effect_when_called_for_local`

影响文件包括：

- `crates/scoopc/src/llvm/codegen/mod.rs`
- `crates/scoopc/src/llvm/codegen/closure/mod.rs`
- `crates/scoopc/src/llvm/codegen/control_flow.rs`
- `crates/scoopc/src/llvm/codegen/class_ctor.rs`
- `crates/scoopc/src/llvm/codegen/stmt.rs`
- `crates/scoopc/src/llvm/codegen/effect_lowered/{body,layout,value}.rs`

当前缺口：

- ordinary callee suspend-state 的分析、保存、resume entry、resume dispatch 都没有 replacement；
- 这意味着“effectful ordinary callable 的 reentry 协议”当前在实现上是断开的。

### 4. explicit `EffectOutcome` contract 只剩名字，没有后端实现

违反原则：非 TLS 语义原则、闭包原则、Clean backend 原则

`EFFECT_REFACTOR.md` 要求：

- propagation 只以 explicit `EffectOutcome` 为 authoritative contract；
- 不允许再依赖 TLS active flag / perform slot / callee suspend scratch。

当前证据：

`cargo check -p scoopc` 缺失方法集中在这组 explicit outcome primitives：

- `alloc_effect_outcome_slot`
- `effect_outcome_is_propagating`
- `effect_outcome_payload_transport`
- `decode_effect_transport_value`
- `emit_current_effect_propagation_with_trace`
- `emit_ordinary_call_effect_propagation_check`
- `emit_ordinary_non_resuming_effect_exit`
- `split_task_transport_tuple_value`
- `coerce_u64_word`

影响文件包括：

- `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`
- `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`
- `crates/scoopc/src/llvm/codegen/mir_body.rs`
- `crates/scoopc/src/llvm/codegen/class_ctor.rs`
- `crates/scoopc/src/llvm/codegen/intrinsics/{containers,thread}.rs`
- `crates/scoopc/src/llvm/codegen/enum_lowering.rs`

当前缺口：

- explicit `EffectOutcome` 虽然是目标合同，但当前 backend 不再有任何完整 builder/query/write-back 实现；
- perform/raise/call-boundary/continuation-resume 都没有可用的 explicit propagation path。

### 5. `perform` / `handle` / `resume` / dynamic call 的表达式与 MIR lowering 入口已断

违反原则：Clean backend 原则

当前证据：

- `crates/scoopc/src/llvm/codegen/expr.rs:21-32, 150-160`
  - 仍调用：
    - `codegen_perform_expr`
    - `codegen_handle_expr`
- `cargo check -p scoopc` 直接报这些方法不存在。

同时，`crates/scoopc/src/llvm/codegen/mir_body.rs` 当前仍残留调用点，要求这些已删除的实现存在：

- `codegen_mir_perform_terminator`
- `codegen_mir_direct_call_with_policy`
- `codegen_mir_funptr_value_call`
- `codegen_mir_fun_value_call`
- `codegen_mir_closure_call`
- `codegen_mir_function_value_call_from_closure_obj`
- `codegen_mir_class_ctor_call`

当前缺口：

- HIR 表达式层的 `perform` / `handle` lowering 没有 replacement；
- MIR 层的 direct/dynamic/effect call lowering 也没有 replacement；
- 旧 TLS bridge 删掉后，这些入口没有被切到新的 `Step_F` / explicit outcome 实现。

### 6. runtime 侧已经没有 continuation/effect policy，但 codegen-owned replacement 也还不存在

违反原则：目标形态原则、Clean backend 原则、非 TLS 语义原则

当前证据：

- `runtime/c/scoop_runtime.c` 中整个旧 continuation runtime section已删除；
- 旧的
  - `scoop_continuation_alloc`
  - `scoop_continuation_resume_with`
  - `scoop_continuation_resume_into`
  - `scoop_continuation_resume_u64`
  - `scoop_continuation_set_captured_callee_suspend_state`
  - cross-thread resume helpers
  都已移除。

但按 `EFFECT_REFACTOR.md` / 当前目标方向，应该补回的是：

- codegen-owned `ScoopContinuation` layout
- codegen-owned one-shot `cmpxchg` driver
- codegen-owned `__scoop_continuation_resume_with(...)`
- explicit answer slot / `EffectOutcome` contract
- continuation capture 的 `current_effect_ctx_ref` / `captured_callee_suspend_state_ref`

当前缺口：

- runtime 已不再拥有 continuation/effect policy；
- codegen 也还没有接管这套 policy；
- 整个 continuation object model / resume driver 目前是缺席状态。

### 7. `EffectCtx` / handler graph 没有任何 replacement 实体

违反原则：非 TLS 语义原则、目标形态原则

`EFFECT_REFACTOR.md` 明确要求：

- handler capture 链必须成为 continuation/state machine 图的一部分；
- 不能再以 ambient TLS handler stack 为语义前提；
- 最终应显式存在 `current_effect_ctx_ref`，并可进一步演化到 managed handler node graph。

当前状态：

- old TLS handler stack 已删除；
- 但当前实现里也没有任何新的 `EffectCtx` 实体、managed handler node、或显式 `current_effect_ctx_ref` ABI。

当前缺口：

- “删掉 TLS handler stack”已经完成；
- “把 handler context 重建成 explicit data model”完全未完成。

### 8. function / block / site 级 effect facts 还没有成为真正的 authoritative lowering contract

违反原则：闭包原则

`EFFECT_REFACTOR.md` 要求至少显式输出：

- `StepSchema`
- `ContinuationSchema`
- `CallableEffectFacts`
- `BlockEffectFacts`
- `CallSiteEffectFacts`
- `PerformSiteEffectFacts`
- `ResumeSiteEffectFacts`
- `HandleSiteEffectFacts`

以及稳定 identity：

- `StepSchemaId`
- `ContinuationSchemaId`
- `CaseTag`
- `ConcreteOpKey`
- `SiteId`

当前证据：

- 仓库已有 `SiteId` 基础设施与 stage outputs；
- 但当前被删后的 backend 报错说明，lowering 仍大量依赖“某些 backend helper 是否存在”，而不是仅消费一个闭包的 facts/schema 包；
- 例如 `effect_lowered/body.rs` 仍需要从 `MainCodegen` 取 ad-hoc effect helpers，而不是直接从 authoritative facts 完成 lowered protocol。

当前缺口：

- `SiteId` 有了；
- 但 function/block/site 级 facts 还没有完全成为 backend 的唯一语义输入；
- 现在 backend 删除旧桥之后，立刻暴露出“effect contract 没有真正闭包输出”的问题。

### 9. `Step_F` / dynamic callable surface / plain-vs-effect ABI 分流没有重建完成

违反原则：目标形态原则、Clean backend 原则

`EFFECT_REFACTOR.md` 要求：

- 对需要 effectful ABI 的 callable，surface/dynamic surface 应围绕固定 `Step_F`；
- `NoOutward` / plain body 不应被强行物化成 state machine；
- dynamic callable surface 应是 `invoke(args_tuple) -> Step_F`；
- plain body 若需对接 effect-typed dynamic surface，应只通过 adapter 包装成 `Complete`。

当前证据：

- `cargo check -p scoopc` 中 `effect_lowered/body.rs`、`effect_lowered/value.rs` 的大量缺失方法，说明这套 dynamic/effect ABI surface 当前没有完整实现；
- `effect_lowered/value.rs` 还缺：
  - `is_task_transport_tuple_ty`
  - `split_task_transport_tuple_value`
  - `coerce_u64_word`
  - thread resume transport runtime declarations

当前缺口：

- 还没有一个完整的 plain/effect ABI 分流实现；
- 也没有稳定的 dynamic `Step_F` invoke surface replacement；
- 删除 TLS bridge 后，dynamic surface 的 fallback/adapter 直接断开。

### 10. 单管线原则尚未恢复到可运行状态

违反原则：单管线原则

当前状态不是“多条管线并存”，而是：

- 原先承载 legacy effect/TLS 语义的局部路径已被强删；
- 但统一单管线下应有的 replacement 还没补回；
- `O0` / `O2` 当前都没有可完成 effect lowering 的同一条主线实现。

当前缺口：

- 必须把 replacement 直接接回现有单管线，而不是再造一条“临时可编译的兼容路径”；
- 当前 compile breakage 说明这条唯一主线还没有重新闭合。

### 11. runtime C 文件内部还存在删除后的残余结构引用，需要继续物理清场

违反原则：目标形态原则

当前证据：

- `cargo check -p scoop_runtime` 现在首先报的是：
  - `runtime/c/scoop_runtime.c:315-384` 仍有已删类型的静态断言残余
  - `ScoopEffectPerformSlot`
  - `ScoopEffectCtx`
  - `ScoopValueTransport`
  - `ScoopEffectHandlerFrame`
  - `SCOOP_EFFECT_PERFORM_SLOT_MAX_WORDS`
- 同时，因为 bulk deletion 把 continuation section里的前置声明一起切掉，`runtime/c/scoop_runtime.c` 还出现了 `scoop_alloc` 的前置声明缺失。

当前缺口：

- runtime 语义层已经删了；
- 但 C 文件内部的物理残余还没清到“自洽的不可编译最小集”；
- 这属于继续清场的机械性缺口，不是新设计决策缺口。

### 12. 验证面仍然残留大量针对旧桥的测试语义

违反原则：目标形态原则、闭包原则

当前证据：

- `crates/scoopc/src/llvm/tests.rs` 里仍有一批“旧名不应出现”的负向断言字符串；
- 这类测试虽然不再驱动实现，但仍然在验证旧桥的名字级行为，而不是验证新 `Step_F` / explicit outcome / explicit ctx contract。

当前缺口：

- 活跃测试还没有切到新设计的 authoritative surface；
- 需要把验证入口从“旧 TLS/bridge 名字是否存在”迁移到：
  - `StepSchema`
  - `resolved_outward_cases`
  - explicit `EffectOutcome`
  - explicit `current_effect_ctx_ref`
  - explicit `incoming_resume_token_ref`
  - plain/effect ABI 分流

## 依赖顺序建议

按 `EFFECT_REFACTOR.md` 的实现原则，当前最合理的修复顺序是：

1. 先补 effectful callable 的显式 hidden ABI
   - `current_effect_ctx_ref`
   - `incoming_resume_token_ref`
   - `outcome`
2. 再补 codegen-owned explicit outcome primitives
3. 再补 ordinary callee suspend/reentry 与 continuation object model
4. 再补 direct/dynamic call、perform、handle、resume 的 clean backend lowering
5. 最后清 runtime C 内部残余结构/前置声明，并重写验证面

反过来做会再次掉回“为了先通过编译而临时补一层兼容桥”的旧路径，这正是 `EFFECT_REFACTOR.md` 明确反对的中间态。
