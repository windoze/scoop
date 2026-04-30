# TODO（Scoop：Continuation / Effect Runtime 收口）

> 生成时间：2026-05-01  
> 历史归档：`docs/archive/plans/TODO-8.md` / `docs/archive/plans/PLAN-8.md`  
> 设计基线：[`CONTINUATION_RUNTIME_REFACTOR.md`](./CONTINUATION_RUNTIME_REFACTOR.md)  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 迁移说明：
> - 从旧 TODO 迁入并继续保留：`T5001f8c -> T5002a`、`T5001f8R -> T5002aR`、`T5001g -> T5002e`、`T5001gR -> T5002eR`。
> - 不再迁入：`T5001f9 / T5001f9R`。新设计明确改为 traced refs + 删除 replay-state，而不是 stable-handle continuation owner 路线。

## 全局约束

- [`CONTINUATION_RUNTIME_REFACTOR.md`](./CONTINUATION_RUNTIME_REFACTOR.md) 是本轮唯一设计基线；实现改变主张时，必须先回写设计文档。
- `PLAN.md` / 当前 `TODO.md` 是本轮唯一计划记录；`docs/archive/plans/*` 只作历史归档，不回写旧 round。
- 上一轮 explicit root frame 成果视为既成前提，不在本轮重开大方向讨论。
- 本轮不保留过渡期最终形态。
  - 若某个旧 bridge API 已被新路径替代，则在该轮实现收尾时必须删除，而不是长期保留兼容入口。
- runtime 只保留 generic substrate。
  - 允许保留 `scoop_alloc_typed`、对象头、GC trace/relocation、thread/native boundary、通用容器与同步原语；
  - 不允许继续把 continuation object model、resume driver、handler stack policy、outcome bridge policy 留在 runtime public API 中。
- `EffectOutcome` 是唯一 propagation source of truth。
  - `resume_token` 必须显式保存在 `EffectOutcome.signal.resume_token`；
  - `callee_suspend_state`、`pending_continuation`、handler stack 不得继续通过 TLS scratch 承担语义 owner 职责。
- `ScoopContinuation` 必须收口为普通 managed object。
  - 不再使用 stable handle；
  - 不再持有 native `malloc` handler snapshot；
  - 不再需要 `release_fn`。
- 旧测试若直接依赖将被删除的 C ABI 形状，应当迁移，而不是强行阻止实现。
- 每个实现任务后都必须紧跟 review 任务。
- 最终 full verification 必须在以下环境下完整执行相关 fixture：
  - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1`

## T5002：Continuation / Effect Runtime 收口

### [DONE] T5002a 完成 state-machine mutable-local flush-back 合同（承接旧 `T5001f8c`）
- 范围：
  - 在 suspend / return / arm-exit / cleanup 四类边界统一 flush mutable locals 回 heap frame，使 frame 成为跨 resume / cleanup 的稳定持久化 source of truth。
  - 收口 `CgLocal.frame_backing_ptr` 相关 contract，保证执行期 local home 不会只在 block 内正确、而在离开 state/arm 时遗漏最新值。
  - 以系统性设计修复当前剩余 blocker：
    - `tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  - 为 direct escape、indirect ordinary suspend、outer mutable local writeback 三类窗口补最小 fixture/LLVM 回归。
- 验收：
  - `effect_multi_escape_indirect_direct_while.scoop` 恢复 golden，不再出现 `missing1..missing4` 或顺序错乱；
  - mutable local 的 frame flush-back contract 已覆盖 suspend / return / arm-exit / cleanup 四类边界；
  - 不再存在“值只停留在执行期 local home，离开 state/arm 后没有及时写回 frame”的路径。
- 依赖：旧 round 到 `T5001f8bR` 为止的工作已完成，可直接继续。
- 完成记录：
  - `write_back_outer_scope_frame_slots(...)` 已统一落在 step-function return、`ReturnHandle`、`ReturnFromFunction`、`Suspend`、`ArmReturnHandle`、`ArmResumeMatchedSite`、`ArmMaterializeContinuation` 以及外层 handle `handle_propagate/handle_done` 退出边界上，flush-back 合同覆盖 suspend / return / arm-exit / cleanup 四类窗口。
  - LLVM 回归已锁定 outer mutable writeback / stable exec local home / cleanup 相关合同：`escaped_continuation_resume_ir_records_outer_slot_storage_and_writeback`、`state_machine_frame_slots_materialize_stable_exec_local_homes`、`cleanup_enter_ir_checks_cleanup_flag_before_reentering_finally`、`cleanup_propagate_ir_restores_propagating_state_after_shared_finally_exit`。
  - 定向 run-pass 已确认 `effect_escape_continuation_outer_mutable_writeback_basic.scoop`、`continuation_resume_enum.scoop`、`effect_multi_escape_direct_indirect_while.scoop`、`effect_multi_escape_indirect_direct_while.scoop` 在默认环境与所需 GC 环境下通过。

### [DONE] T5002aR Review：确认 state-machine flush-back 真正取代了 block-local write-through 偶然正确性
- 重点：
  - flush-back 是否覆盖 suspend / return / arm-exit / cleanup 四类边界，而不是只覆盖 block 内赋值；
  - outer mutable local、arm binder、capture local、escape continuation binder 是否都共享同一持久化合同；
  - `effect_multi_escape_indirect_direct_while.scoop` 是否真正锁住了 direct/indirect mixed 剩余窗口。
- 验收：
  - `T5002b` 可在不再被 state-machine 持久化 source-of-truth 阻塞的前提下继续推进；
  - review 阶段必须在三项 GC env 全开条件下重跑相关 direct/indirect fixture。
- 依赖：T5002a
- 完成记录：
  - 已复核 `write_back_outer_scope_frame_slots(...)` 的调用点覆盖 step-function return、`ReturnHandle`、`ReturnFromFunction`、`Suspend`、`ArmReturnHandle`、`ArmResumeMatchedSite`、`ArmMaterializeContinuation`，以及外层 handle `handle_propagate/handle_done`，flush-back 合同不再只依赖 block 内 write-through 偶然正确。
  - 已复核 outer mutable local、arm binder、capture local、escape continuation binder 的 env materialization 都收口到“entry alloca exec home + frame slot backing”合同；mutable local 的赋值路径会继续通过 `frame_backing_ptr` 同步到持久化 frame slot。
  - 已通过 LLVM 回归 `escaped_continuation_resume_ir_records_outer_slot_storage_and_writeback`、`state_machine_frame_slots_materialize_stable_exec_local_homes`、`cleanup_enter_ir_checks_cleanup_flag_before_reentering_finally`、`cleanup_propagate_ir_restores_propagating_state_after_shared_finally_exit`、`escape_arm_gc_roots_use_frame_slot_or_entry_spill_contract`。
  - 已在默认环境与 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 下复验 `effect_escape_continuation_outer_mutable_writeback_basic.scoop`、`continuation_resume_enum.scoop`、`effect_multi_escape_direct_indirect_while.scoop`、`effect_multi_escape_indirect_direct_while.scoop`、`effect_escape_continuation_indirect_perform_binder_string_use.scoop`、`effect_escape_continuation_indirect_perform_closure_locals.scoop`。

### [DONE] T5002b1 显式引入 direct-call wrapper 的 `incoming_resume_token_ref`
- 范围：
  - 把 top-level ordinary outward-effect direct call wrapper 的 hidden ABI 从 `current_effect_ctx_ref + ScoopEffectOutcome*` 扩成 `current_effect_ctx_ref + incoming_resume_token_ref + ScoopEffectOutcome*`。
  - fresh direct call（HIR path 与 raw-MIR direct-call bridge）显式传入 `null` incoming token，而不是继续把“无 token”这一状态完全隐含在 TLS scratch 缺省值里。
  - wrapper 内在安装 ctx 后，显式 `publish` incoming token；在 `consume_current_effect_outcome(...)` 之后再清空 TLS token scratch，避免丢失本次传播生成的 fresh token。
- 验收：
  - direct-call wrapper IR 明确出现 `@scoop_callee_suspend_state_publish`；
  - fresh direct outward-effect call IR 明确把 `ptr addrspace(1) null` 作为 `incoming_resume_token_ref` 传给 wrapper；
  - 原有 explicit outcome wrapper 合同继续成立，不回退到 post-call TLS active probing。
- 依赖：T5002aR
- 完成记录：
  - `declare_top_level_fun_effect_call_wrapper_impl(...)` 与 `codegen_top_level_fun_effect_call_wrapper_impl(...)` 现已显式接收 `incoming_resume_token_ref`，并在 wrapper 内围绕 legacy call 做 `publish -> consume outcome -> clear`。
  - HIR direct call 与 raw-MIR direct call 在构造 wrapper 实参时都会显式传入 `null_effect_resume_token()`，不再把 fresh-call 的 token 缺省值完全隐含在 runtime TLS 初值里。
  - LLVM 回归 `effect_contract_struct_types_are_registered_for_effect_codegen`、`direct_call_with_real_outward_effect_uses_wrapper_and_explicit_outcome` 已通过；`cargo clippy --all-targets -- -D warnings` 已通过。

### [TODO] T5002b2 把显式 `incoming_resume_token_ref` 扩到剩余 hidden effect ABI surface
- 范围：
  - 把 closure call、funptr call、vtable/itable call、callee resume entry、state-machine step/dispatch 的 hidden effect ABI 全部扩成显式承接 `incoming_resume_token_ref`。
  - 收口当前 boundary helper 的参数编排与函数声明，避免 direct call 已显式、其余路径仍隐式依赖 TLS scratch 的半切换状态。
- 验收：
  - closure / funptr / vtable / itable / callee-resume / step-dispatch 相关 production signature 与 indirect call IR 都已显式携带 `incoming_resume_token_ref`；
  - direct 与 indirect remaining path 不再混用“显式 token / 隐式 TLS token”两套 ABI 形状；
  - 仍未切走 raw TLS handler stack 的路径仅限后续 `T5002b3/T5002b4` 明确覆盖的部分。
- 依赖：T5002b1

### [TODO] T5002b3 引入 managed `ScoopEffectCtx` / `ScoopEffectHandlerNode` 并替换 handle 入口注册路径
- 范围：
  - 在 codegen 中落地 `ScoopEffectCtx { hdr, handler_top_ref }` / `ScoopEffectHandlerNode { hdr, prev_ref, op_tag, flags, owner_frame_ref, dispatch_fn }` 的最终 managed object 布局与 bitmap descriptor。
  - handle 入口改为分配 rooted managed `ScoopEffectHandlerNode` 链与 `ScoopEffectCtx`，不再为 production path 生成 stack `alloca` handler frame + runtime `push/pop`。
  - nested handle/body/finally/ordinary effect-capable call 统一显式接收当前 managed ctx。
  - `runtime_abi.rs` 中 raw handler-frame ABI 退出 production lowering 主路径。
- 验收：
  - production IR 不再调用 `@scoop_effect_handler_stack_push` / `@scoop_effect_handler_stack_pop`；
  - `__scoop_type_desc_runtime__ScoopEffectCtx*` / `__scoop_type_desc_runtime__ScoopEffectHandlerNode*` 由 production codegen 生成，trace bitmap 只覆盖 GC refs 字段；
  - handle 入口相关 IR 断言改为检查 managed node / ctx 分配与 rooted storage；
  - `effect_escape_continuation_arm_performs_outer_effect.scoop` 在默认环境与所需 GC env 下通过。
- 依赖：T5002b2

### [TODO] T5002b4 用 derived ctx / ctx graph dispatch 收口 arm self-inactive、outer redispatch 与 cross-thread resume
- 范围：
  - arm self-inactive 改为 derived effect context，而不是 runtime mutable `active` 位。
  - captured outer redispatch 改为基于 ctx graph 的显式 dispatch，而不是 runtime TLS `handler_stack_top` swap。
  - cross-thread resume 改为依赖 captured managed ctx graph 与显式 token/outcome，而不是 raw TLS handler stack。
- 验收：
  - production IR 不再调用 `@scoop_effect_handler_stack_top` / `@scoop_effect_handler_stack_swap_top`；
  - `tests/fixtures/run-pass/effect_escape_continuation_resume_cross_thread.scoop`、`effect_escape_continuation_arm_performs_outer_effect.scoop`、`effect_escape_continuation_nested_arm_indirect_performs_outer.scoop` 在默认环境与需要的 GC env 下继续通过；
  - `T5002b` 的语义目标已收口完成，可进入 review。
- 依赖：T5002b3

### [TODO] T5002bR Review：确认 production path 已不再依赖 raw TLS handler stack
- 重点：
  - handle 入口 / arm body / redispatch / cross-thread resume 是否已经统一依赖 managed `EffectCtx` graph；
  - 是否还残留“stack alloca handler frame + runtime push/pop/top/swap”生产旁路；
  - arm self-inactive 是否真正来自 derived ctx，而不是共享 node 上的就地 mutation。
- 验收：
  - `T5002c` 可在新的 effect context contract 上继续推进；
  - review 阶段必须同时复核 IR 断言和 end-to-end fixture，而不是只看类型声明改动。
- 依赖：T5002b4

### [TODO] T5002c 将 continuation object / generated resume driver 收回 codegen
- 范围：
  - 按 `CONTINUATION_RUNTIME_REFACTOR.md` 的“2.1 `ScoopContinuation`”“3.2-3.3”“5. Continuation Allocation”“6. Continuation Resume Algorithm”“9. Why No `release_fn` Is Needed Anymore”落地新的 continuation 主线。
  - 在 codegen 中定义最终 `ScoopContinuation` 布局：
    - `captured_effect_ctx_ref`
    - `state_ref`
    - `captured_callee_suspend_state_ref`
    - `resume_word`
    - `resume_gc_ref`
    - `step_fn`
    - `_Atomic resumed`
  - continuation descriptor 必须只依赖 traced fields / bitmap，不再要求 `release_fn`。
  - 生成 module-private `__scoop_continuation_resume_with(...)` helper，负责：
    - one-shot `cmpxchg`
    - payload 写入
    - `state_ref` / `captured_effect_ctx_ref` / `captured_callee_suspend_state_ref` 读取
    - 调用 generated step/dispatch hidden ABI
    - 读取 delimiter answer transport
  - production lowering 不再声明或调用 runtime continuation bridge：
    - `scoop_continuation_alloc`
    - `scoop_continuation_resume_with`
    - `scoop_continuation_set_captured_callee_suspend_state`
    - `scoop_continuation_resume_publish_pending_continuation`
- 验收：
  - production IR 中不再出现上述 runtime continuation symbols；
  - generated continuation type descriptor 的 `release_fn` 为 `NULL`；
  - continuation 内部不再使用 stable handle，也不再持有 native handler snapshot；
  - `tests/fixtures/run-pass/continuation_resume_enum.scoop`、`continuation_resume_struct_with_ref.scoop`、`effect_escape_continuation_gc_stress_multi_string.scoop` 在三项 GC env 全开条件下继续通过。
- 依赖：T5002bR

### [TODO] T5002cR Review：确认 continuation 已成为普通 managed object，而不是 runtime shell + side resources
- 重点：
  - continuation 内是否已经只剩 traced refs、标量与代码指针；
  - 是否还残留 stable handle、native snapshot、`release_fn`、runtime-owned one-shot/resume driver；
  - remaining pin 是否都只是短窗口（如 helper 调用期），而不是长期 owner。
- 验收：
  - `T5002d` 可在不再被旧 continuation object model 阻塞的前提下继续推进；
  - review 必须覆盖 IR、type descriptor 与 GC env fixture，而不是只看结构体定义。
- 依赖：T5002c

### [TODO] T5002d 删除 TLS bridge / replay-state / runtime continuation-effect public ABI，并迁移测试与文档
- 范围：
  - 按 `CONTINUATION_RUNTIME_REFACTOR.md` 的“7. Explicit Resume Token Instead of TLS Callee State”“8. Explicit Outcome Instead of TLS Outcome Bridge”“Source Changes Required”收空旧 bridge。
  - 删除或收空以下 runtime 语义入口：
    - `scoop_callee_suspend_state_publish/get/clear`
    - `scoop_effect_outcome_consume_current/publish`
    - `scoop_continuation_resume_publish_pending_continuation`
    - `ScoopContinuationResumeScope`
    - `ScoopContinuationResumeReplayState`
  - `runtime/c/scoop_runtime_api.h` 从 public allowlist 中删除：
    - 所有 `scoop_continuation_*`
    - 所有 `scoop_effect_handler_stack_*`
    - 所有 `scoop_effect_outcome_*`
    - 所有 `scoop_callee_suspend_state_*`
  - 迁移直接依赖 deleted C ABI 形状的测试：
    - `crates/scoop_runtime/tests/continuation_one_shot.rs`
    - `crates/scoop_runtime/tests/continuation_cross_thread_handler_stack.rs`
    - `crates/scoop_runtime/tests/effect_tls.rs`
    - 改为 compiler IR / run-pass / runtime_gc / end-to-end 验收，或收缩为 generic substrate 测试。
  - 同步 `SCOOP_RUNTIME.md` 与必要实现注释，使其与 `CONTINUATION_RUNTIME_REFACTOR.md` 的边界一致。
- 验收：
  - runtime public allowlist 中不再出现 continuation/effect bridge API；
  - production codegen 与测试不再依赖 deleted C ABI；
  - `tests/fixtures/run-pass/effect_escape_continuation_multi_perform_cross_thread.scoop` 与 `tests/fixtures/runtime_gc/task_step_cross_thread_sequential_handoff_gc_stress.scoop` 在三项 GC env 全开条件下继续通过；
  - 文档、注释与实际边界对齐。
- 依赖：T5002cR

### [TODO] T5002dR Review：确认 runtime 已收缩为 generic substrate，旧 bridge 已真正清零
- 重点：
  - runtime public ABI、production IR 与测试入口里是否都已没有 continuation/effect bridge 残留；
  - replay-state 与 TLS scratch 是否已经退出语义主线，而不是只“没人再主动调用”；
  - 文档是否仍残留 stable-handle continuation owner 或 runtime bridge 叙事。
- 验收：
  - `T5002e` 可在边界已经真正切换的前提下继续做全量验收；
  - review 阶段必须同时检查 allowlist、IR、测试入口和文档，而不是只看运行结果。
- 依赖：T5002d

### [TODO] T5002e 全量回归、GC env、文档收尾（承接旧 `T5001g`）
- 范围：
  - 运行并整理最小验收矩阵，至少覆盖：
    - `cargo test --all`
    - `cargo run -p scoop -- test`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/build`
  - 使用单个 fixture 顺序执行方式，在 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 条件下完整验证：
    - `tests/fixtures/run-pass/**`
    - `tests/fixtures/runtime_gc/**`
    - `tests/fixtures/build/**`
  - 补最小定向回归，锁定：
    - state-machine flush-back；
    - managed effect context / handler redispatch；
    - continuation descriptor 无 `release_fn`；
    - production IR 无 runtime continuation/effect bridge symbol；
    - runtime public allowlist 无 continuation/effect bridge API；
    - cross-thread resume 与 `Task.step()` handoff 继续成立。
  - 同步 `SCOOP_RUNTIME.md` 与必要实现注释，说明 runtime 与 codegen 的新责任边界。
  - 记录对象模型变化后的二进制 / 代码尺寸与主要 GC pause 观察结果，但不把性能调优作为 blocker。
- 验收：
  - 全量回归与定向回归都能支撑“continuation/effect runtime policy 已从 runtime bridge 收回 codegen，runtime 只保留 generic substrate”的结论；
  - 文档、实现注释与实际行为已对齐。
- 依赖：T5002dR

### [TODO] T5002eR Review：确认 continuation runtime refactor 已收口完成，并为后续优化划清边界
- 重点：
  - 是否还残留 continuation/effect runtime correctness 缺口；
  - regression 是否已覆盖：
    - flush-back
    - explicit outcome / resume token
    - managed effect context
    - codegen-owned continuation object
    - 无 runtime bridge symbols / API
    - cross-thread resume / task handoff
  - stable-handle continuation owner 旧路线是否已明确退休，不再混入后续任务；
  - 后续若还要评估性能、`mem2reg`、更细粒度 liveness 或 selective optimization，是否已经明确留到独立任务，而不是混入本轮 correctness 结论。
- 验收：
  - 本轮结论可明确表述为：continuation/effect runtime 已按 `CONTINUATION_RUNTIME_REFACTOR.md` 收口；runtime 只保留 generic substrate；旧 bridge API 与对应测试入口已退出主线。
- 依赖：T5002e
