# Scoop：Effect Refactor Target-Shape 重建计划

> 生成时间：2026-05-11  
> 设计基线：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md)  
> 缺口基线：[`EFFECT_REFACTOR_GAPS.md`](./EFFECT_REFACTOR_GAPS.md)  
> continuation/runtime 补充：[`CONTINUATION_RUNTIME_REFACTOR.md`](./CONTINUATION_RUNTIME_REFACTOR.md)  
> 格式参考：[`PLAN-old.md`](./PLAN-old.md)  
> 本轮主题：旧 continuation/effect TLS bridge 已被硬删除；本轮目标是在**不恢复任何 TLS 语义兼容层**的前提下，按 `EFFECT_REFACTOR.md` 的最终形态重建单一 effect pipeline。

## 0. 工作原则

- [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 是唯一设计基线；如果实现过程中需要改变 effect/continuation/object model/ABI 主张，必须先回写该文档，再继续实现。
- [`EFFECT_REFACTOR_GAPS.md`](./EFFECT_REFACTOR_GAPS.md) 是当前缺口基线；后续任务必须以“消除这些缺口”为目标，而不是以“先恢复编译”为目标。
- 绝对禁止重新引入任何 continuation/effect TLS 语义 source of truth，包括但不限于：
  - `__scoop_effect_handler_stack_top`
  - `__scoop_effect_active`
  - `__scoop_effect_perform_slot`
  - `__scoop_callee_suspend_state`
  - `__scoop_continuation_resume_scope`
  - 以及任何等价的 ambient TLS scratch / bridge contract。
- 绝对禁止恢复旧的 runtime continuation/effect policy API 作为过渡层，包括但不限于：
  - `scoop_continuation_alloc`
  - `scoop_continuation_resume_with`
  - `scoop_continuation_resume_into`
  - `scoop_effect_outcome_consume_current`
  - `scoop_effect_outcome_publish`
- 所有优化级别必须共用同一条编译管线；`O0` / debug build 不允许另走一条“简化但语义不同”的 effect lowering/codegen 通道。
- 后端只能依赖显式输入：
  - 当前 stage 输入
  - 上一阶段显式产出的 facts / schema / table
  - target ABI / opt level / feature flags
  不允许为了补语义而回看 HIR/AST 或 resurrect 已删除的旧 helper 缓存。
- P6 clean backend 必须重新拥有 whole-function protocol：
  - callable ABI
  - `Step_F`
  - `EffectOutcome`
  - `EffectCtx`
  - continuation object model
  - ordinary callee reentry
  - runtime error / non-resuming effect exit
  - GC/runtime contract
- runtime C 最终只保留 generic substrate：
  - alloc / type descriptor / trace / write barrier / thread register / native boundary / array/string/platform/sync helpers
  - 不再拥有 continuation object model 或 effect propagation policy。
- 本轮允许“大步重接”，但不允许为了短期通过编译故意保留一个下一步必然推翻的中间态。
- 验证顺序必须分层推进：
  1. 先恢复 `cargo check -p scoop_runtime`
  2. 再恢复 `cargo check -p scoopc`
  3. 再恢复 `cargo test -p scoopc`
  4. 再恢复 `cargo test -p scoop_runtime`
  5. 再恢复 `cargo test -p scoop`
  6. 最后恢复 `cargo test --all`

## 1. 顺序总览

1. G0：硬删除后的物理残余清场与最小一致基线恢复。
2. G1：effectful callable 的显式 hidden ABI 重建。
3. G2：backend-owned `EffectOutcome` / transport primitive 重建。
4. G3：显式 `EffectCtx` / handler graph 模型重建。
5. G4：ordinary callee suspend/reentry 分析与 lowering 重建。
6. G5：codegen-owned continuation object model 与 generated resume driver 重建。
7. G6：direct/static/dynamic call lowering 与 plain/effect ABI 分流重建。
8. G7：`perform` / `handle` / `resume` / `Step_F` lowering 重建。
9. G8：runtime generic substrate 收尾、验证面迁移与 full regression 恢复。

依赖说明：

- G1 必须先于 G6/G7，因为 call surface 的 hidden ABI 形状是后续所有 lowering 的基础。
- G2/G3 必须先于 G5/G7，因为 continuation resume 与 outward propagation 都依赖 explicit `EffectOutcome` / `EffectCtx`。
- G4 必须先于 G5/G6，因为 ordinary callee suspend/reentry contract 是 continuation 与 dynamic call 共享的底层协议。
- G8 只能在 G0-G7 全部闭合后执行；不得在实现中途用“回跑 full suite”替代结构性收口。

## 2. 分阶段计划

### G0. 硬删除后的物理残余清场与最小一致基线恢复

参考：[`EFFECT_REFACTOR_GAPS.md`](./EFFECT_REFACTOR_GAPS.md) §11、§12。

目标：

- 把这轮 bulk deletion 后残留的机械性破坏面清到“只剩目标 architecture 缺口”，避免后续被无意义的静态断言、前置声明缺失、旧测试字符串噪音淹没。

实现：

- 清理 `runtime/c/scoop_runtime.c` 中已删结构对应的静态断言、辅助 typedef、以及误删导致的中性前置声明缺口（例如 `scoop_alloc` 的前置声明）。
- 清理 `runtime/c/scoop_test.c` 中旧 effect/TLS test-only export 声明。
- 清理 `crates/scoopc/src/llvm/tests.rs`、`pipeline/llvm_codegen_stage.rs` 等活跃测试中仍直接提到已删除旧符号的断言块。
- 清理 `sysroot/core.scoop` 与 effect-facts builder 等活跃 surface/识别表中已删除的 `__scoop_effect_*` intrinsic surface。

阶段输出：

- 代码库中不再存在任何“仅为了旧 TLS bridge 存在”的活跃实现/测试 surface。
- `cargo check` 报错收口到 architecture gap，而不是残余死引用。

验证：

- 对活跃代码目录执行 grep：
  - `crates/scoopc/src`
  - `runtime/c`
  - `sysroot`
  不再出现旧 TLS/bridge 符号名。
- `cargo check -p scoop_runtime`
  - 不再首先报 `ScoopEffectPerformSlot` / `ScoopEffectCtx` / `ScoopEffectHandlerFrame` 这类已删类型残余。

完成条件：

- 剩余编译错误都对应 `EFFECT_REFACTOR_GAPS.md` 里的 target-shape 缺口，而不是物理清场尾巴。

### G1. effectful callable 的显式 hidden ABI 重建

参考：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.5、§4.10、§4.13，`EFFECT_REFACTOR_GAPS.md` §1。

目标：

- 把 effectful callable ABI 从“可选单个 hidden `incoming_resume_token`”升级成显式：
  - `current_effect_ctx_ref`
  - `incoming_resume_token_ref`
  - `ScoopEffectOutcome *outcome`
- 统一 top-level / closure / dynamic surface / resume-entry 的 callable ABI 入口约定。

实现：

- 修改 `crates/scoopc/src/llvm/codegen/mod.rs`：
  - `declare_top_level_fun_with_symbol(...)`
  - `codegen_top_level_fun(...)`
  - `top_level_fun_uses_hidden_incoming_resume_token(...)`
  - `mir_fun_uses_hidden_incoming_resume_token(...)`
  - `function_type_uses_hidden_incoming_resume_token(...)`
- 新增/替换 effect ABI 判定 helper：
  - 必须由 callable effect facts / schema contract 驱动，而不是只看 HIR “是否 effectful”。
- 把 closure callable、function value、callee resume entry 的参数顺序统一到同一 hidden ABI 协议。

阶段输出：

- 所有 effectful callable 的 surface ABI 形状固定；后续 direct/dynamic/continuation lowering 不再需要 wrapper/TLS 旁路补语义。

验证：

- `cargo check -p scoopc`
  - 不再报 `declare_top_level_fun_*wrapper*` / `declare_*resume_entry*` 仅因 ABI helper 缺失而失败。

完成条件：

- effectful callable 的 hidden ABI 在声明层上闭合，且不再以 TLS scratch 为补充输入。

### G2. backend-owned `EffectOutcome` / transport primitive 重建

参考：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.3、§4.13，`EFFECT_REFACTOR_GAPS.md` §4。

目标：

- 在 LLVM backend 内重建 explicit `EffectOutcome` / `EffectSignal` / `ValueTransport` 的 authoritative query/write-back primitive，替代所有已删 runtime bridge。

实现：

- 以 `crates/scoopc/src/llvm/codegen/runtime_abi.rs` 中保留的结构 type builder 为基础，重建一个 backend-owned helper 模块。
- 至少提供：
  - outcome slot alloc/load/store
  - `effect_outcome_is_propagating(...)`
  - `effect_outcome_payload_transport(...)`
  - `effect_outcome_resume_token(...)`
  - `build_value_transport(...)`
  - `build_effect_signal(...)`
  - `build_effect_outcome(...)`
  - `decode_effect_transport_value(...)`
  - `coerce_u64_word(...)`
  - `split_task_transport_tuple_value(...)`
- 禁止恢复 `scoop_effect_outcome_consume_current/publish` 或 `scoop_effect_*slot*` runtime API。

阶段输出：

- backend 自己就能表达 perform/raise/propagate/complete 合同，不再需要任何 TLS 中转 API。

验证：

- `cargo check -p scoopc`
  - 不再报 `alloc_effect_outcome_slot`、`effect_outcome_is_propagating`、`effect_outcome_payload_transport`、`decode_effect_transport_value`、`coerce_u64_word`、`split_task_transport_tuple_value` 缺失。

完成条件：

- explicit `EffectOutcome` 成为 backend 内部的活合同，而不是仅剩 layout type 名字。

### G3. 显式 `EffectCtx` / handler graph 模型重建

参考：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.5、§4.10，`CONTINUATION_RUNTIME_REFACTOR.md` §2.2、§2.3、§4.1-§4.3，`EFFECT_REFACTOR_GAPS.md` §7。

目标：

- 把已删的 TLS handler stack 语义替换为显式 `current_effect_ctx_ref` 和可 capture 的 handler graph / dispatch model。

实现：

- 引入 backend-owned `EffectCtx` 与 handler node layout/type descriptor。
- `handle` 入口需显式构造 handler node graph，并把 ctx 作为 hidden input 传入 nested callable / arm / finally。
- outward dispatch 需从显式 ctx 查找匹配 case，而不是依赖 ambient stack。
- arm self-inactive 必须由 derived ctx / immutable node 语义表达，而不是复活旧 `active` TLS frame 位。

阶段输出：

- handler context 的语义重新存在，但它是显式对象图，不是 TLS stack。

验证：

- `cargo check -p scoopc`
  - 不再报 `prepare_current_effect_call_contract`、`publish_incoming_resume_token`、`swap_effect_handler_stack_top` 这类旧 contract helper 缺失。

完成条件：

- backend 拥有显式 ctx 语义实体；删除 TLS 后不再缺失 handler context 抽象。

### G4. ordinary callee suspend/reentry 分析与 lowering 重建

参考：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §3.5、§4.4、§4.13，`EFFECT_REFACTOR_GAPS.md` §3。

目标：

- 恢复 ordinary callee suspend/reentry 的分析、状态保存、resume entry、resume dispatch；但全部建立在显式 incoming token 和 facts 上，而不是 TLS scratch。

实现：

- 重新集成当前孤立的 `crates/scoopc/src/llvm/codegen/effect/ordinary_callee.rs` 能力到新的 neutral module 中。
- 恢复并重接：
  - `build_fun_callee_suspend_plan_impl`
  - `build_ordinary_callee_suspend_plan`
  - `local_call_may_suspend_from_hir_ty`
  - `hir_ty_declared_effectful`
  - `known_fun_body_may_outward_effect`
  - `function_value_expr_body_may_outward_effect_when_called_for_local`
  - `codegen_callee_resume_dispatch_impl`
  - `codegen_callee_resume_entry_function_impl`
- `incoming_resume_token_ref` 必须成为 ordinary resumed path 的唯一恢复输入。

阶段输出：

- ordinary callee suspend/reentry 再次成为后端的显式协议，而不是删除前那条 TLS workaround。

验证：

- `cargo check -p scoopc`
  - 不再报上述 ordinary callee / outward-effect analysis helper 缺失。

完成条件：

- `needs_reentry` / ordinary suspend-state / resume-entry 全部重新落回单一 clean backend 协议。

### G5. codegen-owned continuation object model 与 generated resume driver 重建

参考：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.5、§4.10，`CONTINUATION_RUNTIME_REFACTOR.md` §2.1、§3.3、§5、§6，`EFFECT_REFACTOR_GAPS.md` §6。

目标：

- 把 runtime 已删除的 continuation policy 迁回 codegen：
  - object layout
  - one-shot driver
  - answer channel
  - explicit `EffectOutcome`
  - captured `EffectCtx`
  - captured ordinary callee suspend token

实现：

- 定义 codegen-owned `ScoopContinuation` layout：
  - `captured_effect_ctx_ref`
  - `state_ref`
  - `step_fn`
  - `resume_word`
  - `resume_gc_ref`
  - `captured_callee_suspend_state_ref`
  - no stable handle
  - no native snapshot
  - no `release_fn`
- 生成 module-private `__scoop_continuation_resume_with(...)` helper。
- one-shot 检查必须使用 LLVM `cmpxchg`；不能恢复 runtime C helper。
- answer transport 必须通过 explicit answer slots / frame result 读取。

阶段输出：

- continuation object model 与 resume driver 重新闭合，但 ownership 完全在 backend 侧。

验证：

- `cargo check -p scoopc`
  - 不再报 `declare_runtime_continuation_resume_with`、`declare_runtime_thread_spawn_join_resume_*` 相关缺失。

完成条件：

- repo 不再需要 runtime C continuation driver，就能表达 continuation alloc / resume / answer / outward propagation。

### G6. direct/static/dynamic call lowering 与 plain/effect ABI 分流重建

参考：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §3、§4.6-§4.10，`EFFECT_REFACTOR_GAPS.md` §2、§9。

目标：

- 重建 direct/static/dynamic call lowering，但直接面向最终 plain/effect ABI 分流，而不是复刻旧 wrapper/TLS boundary。

实现：

- 在新的 non-legacy module 中重建：
  - `codegen_call_impl`
  - `codegen_top_level_fun_call_impl`
  - `try_codegen_class_vtable_call_impl`
  - `try_codegen_interface_itable_call_impl`
  - `codegen_funptr_value_call_impl`
  - `codegen_function_value_call_impl`
  - `codegen_function_value_call_from_closure_obj_impl`
  - `emit_enter_native_for_extern_call_impl`
  - `emit_extern_native_call_impl`
- Plain ABI callable 继续 direct return。
- Effect ABI callable 直接接显式 hidden ABI，不再走 effect call wrapper / TLS probing。
- Dynamic surface 必须按 `Step_F` / invoke(args_tuple) 组织；plain body 只通过 adapter 包成 `Complete`。

阶段输出：

- `call` 层再次存在，但不再依赖任何 deleted legacy bridge file。

验证：

- `cargo check -p scoopc`
  - 不再报 `codegen_call_impl` / `try_codegen_*call_impl` / `codegen_*value_call_impl` 族缺失。

完成条件：

- plain/effect ABI 分流在 direct/vtable/itable/funptr/closure surface 上重建完成。

### G7. `perform` / `handle` / `resume` / `Step_F` lowering 重建

参考：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.3、§4.5、§4.9-§4.10、§4.13，`EFFECT_REFACTOR_GAPS.md` §5、§8、§9。

目标：

- 把 expression/MIR 层的 effectful constructs 接回新的 target-shape：
  - `perform`
  - `handle`
  - `Continuation.resume`
  - MIR `Perform` terminator
  - MIR direct/dynamic effectful call
  - `Step_F`

实现：

- 重建 `crates/scoopc/src/llvm/codegen/expr.rs` 中：
  - `codegen_perform_expr`
  - `codegen_handle_expr`
- 重建 `crates/scoopc/src/llvm/codegen/mir_body.rs` 中：
  - `codegen_mir_perform_terminator`
  - `codegen_mir_direct_call_with_policy`
  - `codegen_mir_funptr_value_call`
  - `codegen_mir_fun_value_call`
  - `codegen_mir_closure_call`
  - `codegen_mir_function_value_call_from_closure_obj`
  - `codegen_mir_class_ctor_call`
- `Continuation.resume` 必须切到 generated resume driver，而不是 runtime helper。
- `Step_F` case identity、payload tuple、resume tuple 必须仅由 facts/schema 驱动；不得再回 HIR 猜测。

阶段输出：

- 语言层 effect constructs 重新接通到新的 backend-owned protocol。

验证：

- `cargo check -p scoopc`
  - 不再报 `codegen_perform_expr`、`codegen_handle_expr`、`codegen_mir_perform_terminator` 及其相关 MIR call helper 缺失。

完成条件：

- `perform` / `handle` / `resume` / `Step_F` 不再依赖任何 deleted TLS bridge surface。

### G8. runtime generic substrate 收尾、验证面迁移与 full regression 恢复

参考：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.11-§4.15，`EFFECT_REFACTOR_GAPS.md` §10-§12。

目标：

- 在新 target-shape 重接完成后，清掉 runtime/golden/tests 里剩余的旧桥假设，并恢复完整验证矩阵。

实现：

- 清理 `runtime/c/scoop_runtime.c` 里 bulk deletion 后留下的所有静态断言、注释、前置声明碎片。
- 清理 `runtime/c/scoop_runtime_api.h`、`runtime/c/scoop_test.c`、`crates/scoopc/src/llvm/tests.rs` 等活跃验证面中对旧 bridge 名字的残留假设。
- 新验证必须转为：
  - explicit `EffectOutcome`
  - explicit `current_effect_ctx_ref`
  - explicit `incoming_resume_token_ref`
  - `StepSchema` / `resolved_outward_cases`
  - plain/effect ABI 分流
- 完整恢复：
  - `cargo check -p scoop_runtime`
  - `cargo check -p scoopc`
  - `cargo test -p scoopc`
  - `cargo test -p scoop_runtime`
  - `cargo test -p scoop`
  - `cargo test --all`

阶段输出：

- 仓库重新达到“单一 effect refactor 主线 + 无 TLS continuation/effect 语义”的可编译、可验证状态。

验证：

- 上述完整矩阵全部通过。

完成条件：

- 当前活跃实现、活跃测试、活跃文档都不再保留旧 TLS continuation/effect 语义或名字级兼容面。
