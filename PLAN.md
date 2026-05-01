# Scoop：下一轮计划（Continuation / Effect Runtime 收口）

> 生成时间：2026-05-01  
> 历史归档：`docs/archive/plans/PLAN-8.md` / `docs/archive/plans/TODO-8.md`  
> 本轮主题：按 [`CONTINUATION_RUNTIME_REFACTOR.md`](./CONTINUATION_RUNTIME_REFACTOR.md) 的最终设计，把 continuation/effect runtime 从 `runtime/c/scoop_runtime.c` 的 bridge 形态收口为 codegen-owned object model 与 hidden ABI；runtime 只保留 generic GC/thread/alloc substrate。

## 0. 工作原则

- [`CONTINUATION_RUNTIME_REFACTOR.md`](./CONTINUATION_RUNTIME_REFACTOR.md) 是本轮唯一设计基线；如果实现过程中改变主张，必须先回写该文档，再继续写代码。
- 上一轮 explicit root frame 工作已经完成并归档到 `PLAN-8.md` / `TODO-8.md`；本轮默认把 explicit root frame 视为既成前提，不重新开启旧 round 的总体设计讨论。
- 本轮不接受“过渡期长期保留”。
  - 最终状态里，旧的 continuation/effect runtime bridge API 必须被删除，而不是留下兼容层。
- runtime 只保留 generic substrate。
  - 包括 `scoop_alloc_typed`、对象头、type descriptor、GC trace/relocation、write barrier、线程注册/原生边界、通用容器与同步原语。
  - runtime 不再拥有 continuation object model、resume driver、handler stack policy、outcome bridge policy。
- continuation/effect 的 source of truth 必须显式化。
  - `EffectOutcome` 是唯一 propagation contract；
  - `resume_token` 必须显式存在于 `EffectOutcome.signal.resume_token`；
  - `callee_suspend_state`、`pending_continuation`、handler stack 不再经由 TLS scratch 传递语义。
- `ScoopContinuation` 必须成为普通 managed object。
  - 不再持有 stable handle；
  - 不再持有 native `malloc` 的 handler snapshot；
  - 不再需要 `release_fn`。
- correctness first，收缩边界 second，优化 third。
  - 先把 object model、hidden ABI 与删除旧 bridge 的 correctness 路径闭合；
  - 尺寸、性能与进一步优化不作为前置 blocker。
- 旧 TODO 里仍有效的任务会迁入本轮；已经被新设计否定的任务不再继续。
- 每个实现任务后都必须有 review 任务，且最终 full review 必须在 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 下完整验证相关 fixture 集。

## 1. 当前判断

- 旧 TODO 中仍然有效的剩余任务只有两类：
  1. state-machine mutable-local flush-back 收口；
  2. 最终 full regression / 文档收尾。
- 旧 TODO 中 `T5001f9 / T5001f9R` 的 stable-handle 路线已经被新设计否定。
  - 新设计明确要求：continuation 改为 traced refs，replay-state 删除，见 `CONTINUATION_RUNTIME_REFACTOR.md` 的“2. Authoritative Data Model”“6. Continuation Resume Algorithm”“7-8. Explicit Resume Token / Explicit Outcome”。
- 当前真正的主线已经从“把 runtime continuation owner 收口到 stable handle”转成“把 continuation/effect runtime policy 从 runtime bridge 收回 codegen”。

## 2. 顺序总览

1. 先完成旧 round 遗留但在新设计下仍必需的 state-machine flush-back 收口。
2. 再引入 managed `EffectCtx` / `EffectHandlerNode` 与显式 hidden effect ABI，替换 raw TLS handler stack。
3. 随后把 `ScoopContinuation` object model 与 generated resume driver 迁入 codegen，改成 traced refs + module-private helper。
4. 然后删除 TLS outcome/callee-state/pending-continuation bridge、replay-state，以及 runtime continuation/effect public ABI，并同步迁移测试与文档。
5. 最后做全量回归、GC env、IR 断言与文档收尾，确认新边界真正落地。

## 3. 分阶段目标

### P0. 承接旧 round 的 state-machine flush-back 收口

- 对应新 TODO 的 `T5002a / T5002aR`。
- 目标是把 mutable local 的持久化 source of truth 真正收口到 heap frame，而不是只在 block 内做 write-through。
- 这一阶段不是旧设计残留，而是新 continuation 设计的前提：即便 `Continuation.state` 改为 traced ref，resume 后仍然依赖 frame 中持久化的 locals/state。
- 重点 fixture：`tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`。
- 进展更新（2026-05-01）：`T5002a` 已完成。`write_back_outer_scope_frame_slots(...)` 现在会在 step-function return、handle/function return、suspend、arm exit，以及外层 handle 的 done/propagate 退出边界统一执行，使 frame 成为 mutable local 跨 resume / cleanup 的稳定 source-of-truth。
- 验证摘要（2026-05-01）：LLVM 回归 `escaped_continuation_resume_ir_records_outer_slot_storage_and_writeback`、`state_machine_frame_slots_materialize_stable_exec_local_homes`、`cleanup_enter_ir_checks_cleanup_flag_before_reentering_finally`、`cleanup_propagate_ir_restores_propagating_state_after_shared_finally_exit` 全通过；run-pass fixture `effect_escape_continuation_outer_mutable_writeback_basic.scoop`、`continuation_resume_enum.scoop`、`effect_multi_escape_direct_indirect_while.scoop`、`effect_multi_escape_indirect_direct_while.scoop` 在默认环境与 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 下通过。下一步按顺序进入 `T5002aR`。
- Review 更新（2026-05-01）：`T5002aR` 已完成。已复核 outer mutable local、arm binder、capture local、escape continuation binder 都采用“entry alloca exec home + frame slot backing”的统一持久化合同，mutable local 的赋值继续经 `frame_backing_ptr` 同步到 frame slot；并补跑 LLVM 回归 `escape_arm_gc_roots_use_frame_slot_or_entry_spill_contract` 以及 fixture `effect_escape_continuation_indirect_perform_binder_string_use.scoop`、`effect_escape_continuation_indirect_perform_closure_locals.scoop`（默认环境 + GC env 全开）确认 binder/capture 路径未偏离该合同。当前可按顺序进入 `T5002b`。

### P1. Managed `EffectCtx` / `EffectHandlerNode` 与 hidden effect ABI

- 对应 `CONTINUATION_RUNTIME_REFACTOR.md` 的“2.2 `ScoopEffectCtx`”“2.3 `ScoopEffectHandlerNode`”“3. Hidden ABI”“4. Effect Context Construction”。
- 当前 `runtime/c/scoop_runtime.c:434-468` 的 raw TLS handler stack、以及 `state_machine_emitter.rs:4848-4910` 的栈上 handler frame push/pop 仍深度耦合 ordinary call boundary、state-machine dispatch、handle 入口和 cross-thread resume；直接整块落地 `T5002b` 风险过高，因此拆为四个前置子任务顺序推进：
  1. `T5002b1` 先把 direct-call wrapper 的 `incoming_resume_token_ref` 显式化，至少把最外层 ordinary direct-call boundary 从“完全依赖 TLS token 缺省值”收口为显式 hidden input。
  2. `T5002b2a` 先把同一 token contract 扩到 ordinary indirect-call surface：closure/funptr/vtable/itable 相关 generated callable signature 与 indirect call IR 都显式携带 `incoming_resume_token_ref`，fresh path 统一传 `null`。
  3. `T5002b2b1` 先把 callee resume entry 自身的 hidden incoming token contract 收口：replay helper / resume entry 都显式把 replay token 当作 `incoming_resume_token_ref`，entry 先 publish 再 dispatch。
  4. `T5002b2b2a` 先让 resumed non-call suspend 的 fresh continuation materialization 显式继承当前 ordinary callee replay token，避免 replay 链在 materialize 边界直接被丢掉。
  5. `T5002b2b2b` 再修复 nested-handle immediate-resume replay-state 穿过 ordinary callee boundary 时的 owner / replay 错位，确保 `T5002b2b1` 不只是 IR 形状正确，而是 end-to-end 能继续 replay inner callee tail。
  6. `T5002b2c` 随后把 state-machine step/dispatch 与 runtime continuation bridge 扩成显式 token 传递，结束“resume/dispatch 仍靠 TLS scratch 注入 token”的旧路径。
  7. `T5002b3` 再引入 managed `ScoopEffectCtx` / `ScoopEffectHandlerNode` 最终布局与 descriptor，并把 handle 入口从 stack handler frame + runtime push/pop 切到 managed handler node graph。
  8. `T5002b4` 最后收口 arm derived ctx、captured outer redispatch 与 cross-thread resume，彻底移除 raw TLS top/swap 生产依赖。
- 进展更新（2026-05-01）：`T5002b1` 已完成。top-level outward-effect direct-call wrapper 现在显式接收 `incoming_resume_token_ref`，fresh direct call 会传 `null` token，wrapper 内则在 legacy call 前后执行 `publish -> consume outcome -> clear`。这一步还没有删除 raw TLS handler stack，但已经把最外层 direct-call token contract 从隐式 TLS 缺省值收口为显式 hidden input。
- 进展更新（2026-05-01，拆分）：原 `T5002b2` 同时触及 ordinary indirect-call surface、callee resume entry、step/dispatch 与 runtime continuation bridge，跨 Rust codegen 与 runtime C 边界，单任务过大。已按依赖拆为 `T5002b2a/b/c` 及对应 review；当前先执行 `T5002b2a`，优先消除 closure/funptr/vtable/itable 与 direct wrapper 之间的 token ABI 分叉。
- 进展更新（2026-05-01）：`T5002b2a` 已完成。ordinary indirect-call surface 现在与 direct wrapper 共享同一“显式 incoming token”边界约定：effect-capable generated callable signature 会在 hidden sret 后显式承接 `incoming_resume_token_ref`，closure / funptr / vtable / itable caller boundary 会在 legacy call 前 `publish(null)`，并在 `consume outcome` 后 `clear` TLS token scratch。定向 LLVM 回归 `closure_call_with_real_outward_effect_uses_explicit_outcome_boundary`、`effectful_funptr_call_uses_explicit_outcome_boundary`、`virtual_call_with_real_outward_effect_uses_explicit_outcome_boundary`、`interface_call_with_real_outward_effect_uses_explicit_outcome_boundary`、`direct_effectful_signature_without_outward_effect_skips_tls_check`、`closure_call_without_outward_effect_skips_tls_check`、`production_codegen_suspendability_observes_overridden_pass_summary` 已通过；下一步按顺序进入 `T5002b2aR`。
- Review 更新（2026-05-01，进行中）：在执行 `T5002b2aR` 时先复核了 closure / funptr / vtable / itable 的签名与 boundary helper，并发现、修复了两个既有缺口：
  - `mir_body.rs` 的 `pass_mir_closure_call` 之前会在安装 ordinary effect boundary 之前先读取 closure `env_ptr/fn_ptr`，现已改为像 HIR function-value path 一样 defer/reload closure object，并新增 LLVM 回归 `production_pass_mir_closure_call_reloads_closure_after_effect_boundary` 锁定 boundary 后 reload 合同；
  - top-level pass MIR body 绑定参数时此前只跳过 hidden sret，没有同步跳过新加入的 hidden incoming token，导致 effectful pass MIR body 会把 token slot 错当成用户参数；现已修正 param offset。
- Review 阻塞更新（2026-05-01）：进一步尝试把 production-lowered materialized MIR closure 的 effectful body 也纳入 review 时，暴露出另一个尚未收口的既有实现边界：closure body 若直接 perform effect，当前 production MIR bridge 仍会在 `mir_body.rs` 命中 `UnsupportedMainBody { kind: "pass MIR rvalue" }` / `pass MIR terminator`。这不是 review 可接受的“窄形状通过”结果，因此已把该缺口前插为新的前置任务 `T5002b2a1`，待其完成后再回到 `T5002b2aR`。
- 当前已完成的定向验证（2026-05-01）：`cargo test -p scoopc explicit_outcome_boundary -- --nocapture`、`cargo test -p scoopc production_pass_mir_closure_call_reloads_closure_after_effect_boundary -- --nocapture`、`cargo test -p scoopc direct_effectful_signature_without_outward_effect_skips_tls_check -- --nocapture`、`cargo test -p scoopc closure_call_without_outward_effect_skips_tls_check -- --nocapture`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_indirect_perform_nonresuming_function_value_local.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_hidden_suspend_virtual_helper_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_hidden_suspend_interface_helper_basic.scoop`、`cargo clippy --all-targets -- -D warnings` 均已通过。
- 进展更新（2026-05-01）：`T5002b2a1` 已完成。剩余的 effectful materialized MIR closure body 缺口不是 pass-view 发布本身，而是 `codegen_mir_perform_terminator(...)` 在 `Perform` 作为 closure body terminator 时会把 builder 停在未终止的 `pass_mir_effect_perform_dead` block；现已在 terminator lowering 末尾显式补 `unreachable`，避免 LLVM verifier 失败，同时保留表达式路径继续使用 dead landing block 的合同。
- 验证摘要（2026-05-01）：新增 LLVM 回归 `production_codegen_lowers_raw_mir_effectful_closure_body_direct_perform` 与 `production_pass_mir_effectful_closure_body_direct_perform_lowering`，分别锁定 raw materialized closure body 的 `Perform` terminator 收尾，以及 pass-visible caller 的显式 `incoming_resume_token_ref` / outcome boundary + closure body direct-perform lowering；新增 run-pass fixture `effect_indirect_perform_materialized_mir_closure_basic.scoop`，并在默认环境与 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 下通过。下一步按顺序回到 `T5002b2aR`。
 - Review 更新（2026-05-01）：`T5002b2aR` 已完成。已复核 top-level callable、HIR closure 与 materialized MIR closure 的 hidden ABI/参数绑定，确认 `incoming_resume_token_ref` 都在 hidden sret 之后、普通参数之前进入 generated callable surface；ordinary indirect-call boundary（closure / funptr / vtable / itable）与 pass-MIR closure caller 也都遵守 `publish incoming token -> consume outcome -> clear token -> restore handler top` 的同一收口顺序。
 - Review 验证（2026-05-01）：`cargo test -p scoopc explicit_outcome_boundary -- --nocapture`、`cargo test -p scoopc production_pass_mir_closure_call_reloads_closure_after_effect_boundary -- --nocapture`、`cargo test -p scoopc production_codegen_lowers_raw_mir_effectful_closure_body_direct_perform -- --nocapture`、`cargo test -p scoopc production_pass_mir_effectful_closure_body_direct_perform_lowering -- --nocapture` 均通过；run-pass fixture `effect_indirect_perform_nonresuming_function_value_local.scoop`、`effect_indirect_perform_materialized_mir_closure_basic.scoop`、`effect_handle_hidden_suspend_virtual_helper_basic.scoop`、`effect_handle_hidden_suspend_interface_helper_basic.scoop` 在默认环境与 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 下通过；`cargo clippy --all-targets -- -D warnings` 通过。随后进入原 `T5002b2b`。
 - 进展更新（2026-05-01）：原 `T5002b2b` 已拆成 `T5002b2b1 / T5002b2b2`。`T5002b2b1` 已完成：callee resume replay helper 与 resume entry 现在都显式把 replay token 当作 `incoming_resume_token_ref`，`codegen_callee_resume_entry_function_impl(...)` 会在 entry 先 `publish_incoming_resume_token(...)` 再做 resume-site dispatch；LLVM 回归 `suspend_ir_stores_callee_resume_token_on_frame_and_replays_via_resume_thunk` 与 `cargo clippy --all-targets -- -D warnings` 已通过。
 - 阻塞更新（2026-05-01）：在尝试用 nested-handle immediate-resume 最小程序做 `T5002b2b` 的 end-to-end 验证时，发现 resumed ordinary callee 第二次 outward suspend 穿过 `NestedHandleBoundary` 后，outer `k.resume(...)` 会直接把 outer payload 当成最终 answer，跳过 inner callee tail。这说明仅完成 `callee resume entry` ABI/publish 对齐仍不足以闭合 replay chain，因此已把该缺口前插为新的前置子任务 `T5002b2b2`；待其完成后再进入 `T5002b2bR`。
 - 进展更新（2026-05-01，拆分）：继续定位 `T5002b2b2` 时，发现该问题至少包含两个不同 owner 断点：
   1. resumed non-call suspend（direct perform / nested-handle boundary）materialize fresh continuation 时，会直接丢掉当前 ordinary callee replay token；
   2. nested-handle immediate-resume 路径还会把 legacy replay-state 误当成 ordinary callee token 穿过 outer call boundary。
   因此 `T5002b2b2` 已继续拆成 `T5002b2b2a / T5002b2b2b`，先收口第一个 owner 缺口，再处理 replay-state 穿越 ordinary boundary 的剩余问题。
 - 进展更新（2026-05-01）：`T5002b2b2a` 已完成。state-machine `Suspend` terminator materialize fresh continuation 时，现在会优先捕获当前 site 的 ordinary token slot；若该 site 没有 ordinary token slot，则回退读取 `scoop_callee_suspend_state_get()`，把当前 TLS incoming token 显式写入 fresh continuation 的 `captured_callee_suspend_state`。LLVM 回归 `resumed_non_call_suspend_ir_captures_current_callee_resume_token_on_materialized_continuation` 与 `cargo clippy --all-targets -- -D warnings` 已通过。
 - 当前剩余 blocker（2026-05-01）：`T5002b2b2b` 仍需处理 nested-handle immediate-resume 路径上的第二个 owner 问题：legacy replay-state 目前仍会穿过 ordinary callee boundary 并被误当成 callee resume entry token，导致 replay 调度落到错误对象形状；这一部分还未修完，因此本次先在 `TODO.md` / `PLAN.md` 中显式拆出后续子任务。

### P2. Codegen-owned `ScoopContinuation` 与 generated resume driver

- 对应 `CONTINUATION_RUNTIME_REFACTOR.md` 的“2.1 `ScoopContinuation`”“3.2-3.3”“5. Continuation Allocation”“6. Continuation Resume Algorithm”“9. Why No `release_fn` Is Needed Anymore”。
- 当前 `runtime/c/scoop_runtime.c:1140-1820` 中 continuation C struct + trace/release/alloc/discard/resume 主线必须退出生产路径。
- 新路径要做到：
  - `ScoopContinuation` 由 codegen 直接建模；
  - `captured_effect_ctx_ref` / `state_ref` / `captured_callee_suspend_state_ref` 都是 traced refs；
  - descriptor 只需 bitmap / `release_fn = NULL`；
  - `__scoop_continuation_resume_with` 由 codegen 生成，不再调用 runtime `scoop_continuation_resume_with(...)`。

### P3. 删除 TLS bridge / replay-state，并收缩 runtime public ABI

- 对应 `CONTINUATION_RUNTIME_REFACTOR.md` 的“7. Explicit Resume Token Instead of TLS Callee State”“8. Explicit Outcome Instead of TLS Outcome Bridge”“Source Changes Required”。
- 当前下列 bridge 必须从最终实现中消失：
  - `scoop_callee_suspend_state_*`
  - `scoop_effect_outcome_*`
  - `scoop_continuation_resume_publish_pending_continuation`
  - `ScoopContinuationResumeScope`
  - `ScoopContinuationResumeReplayState`
  - `runtime/c/scoop_runtime_api.h` 中 continuation/effect bridge allowlist
- 与此同时，需要把直接依赖这些 ABI 形状的 runtime tests 迁到 compiler IR / run-pass / end-to-end 覆盖。

### P4. Full Verification 与文档收尾

- 承接旧 TODO 的 `T5001g / T5001gR`，在新 round 中改写为 `T5002e / T5002eR`。
- 不仅要验证 explicit root frame 旧合同仍成立，还要新增验证：
  - production IR 不再调用 runtime continuation/effect bridge symbols；
  - runtime public API allowlist 不再导出 continuation/effect bridge API；
  - 文档、注释、测试入口都与 `CONTINUATION_RUNTIME_REFACTOR.md` 一致。

## 4. 主要风险与应对

### 4.1 mutable-local flush-back 仍可能阻塞 direct/indirect mixed 路径

- 即使 continuation object model 收回 codegen，只要 frame 上的持久化状态没在 suspend / cleanup / arm-exit 前统一 flush，direct/indirect mixed fixture 仍会继续失败。
- 因此 `T5002a` 必须先完成，不能跳过。

### 4.2 managed handler context 会触及大量 call boundary

- `current_effect_ctx_ref` / `incoming_resume_token_ref` / `EffectOutcome*` 一旦进入 hidden ABI，会影响 ordinary effect-capable call、state-machine step/dispatch、continuation resume、cross-thread resume 与 nested handle redispatch。
- 应对方式：按 TODO 顺序推进，并在每一步都补 focused IR/fixture 覆盖。

### 4.3 删除 runtime bridge ABI 会先打破测试

- `crates/scoop_runtime/tests/continuation_one_shot.rs`、`continuation_cross_thread_handler_stack.rs`、`effect_tls.rs` 当前直接依赖将被删除的 C ABI 形状。
- 本轮必须把这些测试视为“迁移对象”，而不是实现 blocker 本身。

### 4.4 cross-thread resume 不能因删 TLS bridge 而退化

- 新设计要求 cross-thread resume 仍成立，只是改由 captured managed `EffectCtx` graph 和 explicit token/outcome 维持，而不是 runtime TLS swap。
- 这条语义必须作为独立验收对象保留。

## 5. 完成标准

本轮完成时，必须能够明确陈述以下结论全部成立：

1. `ScoopContinuation` 已从 runtime bridge 收回 codegen，且是普通 managed object。
2. continuation 内部不再使用 stable handle，也不再持有 native handler snapshot。
3. continuation descriptor 不再使用 `release_fn`。
4. `EffectOutcome` 是唯一 propagation source of truth。
5. `resume_token` 显式存在于 `EffectOutcome.signal.resume_token`，不再经 TLS scratch 中转。
6. production path 已不再使用 raw TLS handler stack push/pop/top/swap。
7. runtime public ABI 中不再导出 continuation/effect bridge API。
8. 对应测试入口已迁移，full regression 与三项 GC env 全开矩阵均通过。
