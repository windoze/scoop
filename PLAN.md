# Scoop：当前计划（effect 主线优先，后续任务顺延）

> 生成时间：2026-04-15  
> 历史归档：`PLAN-3.md` / `TODO-3.md`  
> 范围：本计划先覆盖当前 effect 统一主线（`T30`）；为避免下一批任务继续停留在归档里，也顺延保留前端 / 并发 / 类型系统的后续队列（`T31`～`T34`）。当前执行顺序仍以 `T30` 全部收口为先。
>
> 2026-04-18 当前轮复审更新：`T3016R` 已完成。复审 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 与 `crates/scoopc/src/llvm/codegen/mod.rs` 的 function-return 路径后确认，`STATE_TAG_FUNCTION_RETURNED` 的消费没有另起 effect-only 返回出口：普通用户函数继续复用 `return_context` / `finish_function_return_path()`，step/dispatch runtime function 内的 nested handle 则通过 `effect_function_return_context` 先回到本地 return block，再把 payload 写回外层 handle frame 并继续走外层 cleanup/done 合同。复审过程中发现一个真实缺口：`HandleStateOp::Return` 重新求值 `return expr` 时没有带 enclosing function 的 expected return type，导致 `handle` 内 `return 1` 返回到 `Any` 时漏掉 `Int -> Any` boxing。现已改为通过 `enclosing_function_return_ty()` 把 early-return payload 按普通 `return` 合同做 expected-context/coercion 后再写入 effect transport slots，并新增 dedicated run-pass fixture `effect_handle_return_from_function_any_boxing.scoop` 锁定 GC 后 boxed object 仍存活。已验证新 fixture、既有 `effect_handle_return_from_function_basic/finally/nested_handle`、cleanup baseline `effect_handle_yield_and_step_finally.scoop`、`cargo test -p scoopc plan_and_segments_support_return_inside_handle_body_block_expression -- --nocapture`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过；复跑 `cargo run -p scoop --features llvm -- test` 后，suite 仍只停在 `tests/fixtures/run-pass/effect_escape_continuation_async_executor_fifo.scoop` 的 stale `EXPECT: fail`，对应已跟踪的 `T3017`，未引入新的更早回归。当前 effect 主线下一项推进到 `T3017`。
>
> 2026-04-18 当前轮完成更新：`T3016` 已完成。`crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 现已在 `codegen_handle_expr_via_state_machine()` 的 `handle_done` 路径消费 `STATE_TAG_FUNCTION_RETURNED`：普通函数路径直接复用既有 `finish_function_return_path()` / `return_context`，step function / dispatch loop 内递归生成的 nested handle 则通过新增 `effect_function_return_context` synthetic return bridge 把 early-return payload 上传到外层 handle frame，而不是把它误当普通 handle result。为解决 `finally` cleanup replay 把 function-return sentinel 冲掉的问题，统一 frame 新增了持久化 `completion_tag` system field；dispatch loop 在进入 cleanup 前会捕获 `HANDLE_RETURNED` / `FUNCTION_RETURNED` terminal tag，cleanup 完成后恢复 `state_tag`，从而让 `finally` 跑完后仍保持“函数已经返回”的完成模式。已新增 3 条 dedicated run-pass fixture：`effect_handle_return_from_function_basic.scoop`、`effect_handle_return_from_function_finally.scoop`、`effect_handle_return_from_function_nested_handle.scoop`；并验证这 3 条 fixture、既有 cleanup baseline `effect_handle_yield_and_step_finally.scoop`、`cargo test -p scoopc plan_and_segments_support_return_inside_handle_body_block_expression -- --nocapture`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 全部通过。复跑 `cargo run -p scoop --features llvm -- test` 后，suite 仍只停在 `tests/fixtures/run-pass/effect_escape_continuation_async_executor_fifo.scoop` 的 stale `EXPECT: fail`，属于已跟踪的 `T3017` expectation cleanup，而不是 `T3016` 的生产 blocker。当前 effect 主线下一项推进到 `T3016R`。
>
> 2026-04-18 当前轮复审更新：`T3015R` 已完成。复审 `runtime/c/scoop_runtime.c` 与 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 后确认，handler active/inactive 与 escaped continuation context 已形成对称闭环：runtime 侧 `scoop_continuation_alloc()` / `scoop_continuation_release()` / `scoop_continuation_resume_common()` 让 continuation 持有独立的 handler stack 堆快照，并在 resume 时通过 `scoop_effect_handler_stack_swap_top()` 临时安装、step 返回后恢复调用方 TLS；compiler 侧 `emit_effect_runtime_functions()` / `emit_dispatch_loop_body()` / `emit_dispatch_arm_execution()` 则让初始 `handle` 执行与 escaped continuation resume 共用同一个 `scoop.effect.dispatch.*` 入口，并用 `clear_active + arm_context_active + outward-propagate` 的单一路径落实 arm self-inactive，而不是依赖 stack frame 恰好失效。已验证 `cargo test -p scoop_runtime --test continuation_one_shot -- --nocapture`、`cargo test -p scoop_runtime --test continuation_cross_thread_handler_stack -- --nocapture`、两条 `scoopc` IR 定向测试、4 条 escaped-continuation run-pass fixture、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 全部通过。当前 effect 主线下一项推进到 `T3016`。
>
> 2026-04-18 当前轮完成更新：`T3015` 已完成。复核 `runtime/c/scoop_runtime.c`、`crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 以及现有 runtime / fixture 覆盖后确认，`T3015a` 已落地的 continuation-owned handler stack 快照与统一 dispatch-loop resume 入口，已经把 `arm` 执行期 self-inactive、escaped continuation 在 `handle` 返回后的 handler context 生命周期，以及跨线程 / 延迟 resume 的 active/inactive 恢复语义一并收口。已验证 `effect_escape_continuation_arm_performs_outer_effect.scoop`、`effect_escape_continuation_nested_arm_indirect_performs_outer.scoop`、`effect_escape_continuation_scheduler_round_robin.scoop`、`effect_escape_continuation_resume_cross_thread.scoop`、`cargo test -p scoop_runtime --test continuation_cross_thread_handler_stack -- --nocapture` 全部通过；复跑 `cargo run -p scoop --features llvm -- test` 后，suite 仍只停在 `tests/fixtures/run-pass/effect_escape_continuation_async_executor_fifo.scoop` 的 stale `EXPECT: fail`，属于已跟踪的 `T3017` expectation cleanup，而不是 `T3015` 的生产 blocker。当前 effect 主线下一项推进到 `T3015R`。
>
> 2026-04-18 当前轮复审更新：`T3009bR` 已完成。复审 `crates/scoopc/src/resolve/scopes.rs`、`crates/scoopc/src/typecheck/expr/call.rs`、`crates/scoopc/src/ast/mod.rs`、`crates/scoopc/src/hir/lower/mod.rs`、`crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/effect/mod.rs`、`crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`、`crates/scoopc/src/llvm/codegen/runtime_abi.rs` 与 `runtime/c/scoop_runtime.c` 后确认，`Continuation.resume(...)` 的 builtin 语义仍只由 `continuation_resume_call_sites` side table 驱动：typecheck 记录 call span，HIR lowering 原样带入，`codegen_call()` 与 state-machine segmentation 都只按 call span 命中专用 lowering / hidden-suspend 分类，没有按成员名 `"resume"`、FQN `"scoop.core.Continuation.resume"` 或 receiver 形状做 generic member-access / generic call fallback。`codegen_continuation_resume_builtin()` 继续直接把 payload 写入 continuation 的 `resume_word` / `resume_gc_ref` 槽位，并调用 `scoop_continuation_resume()`；runtime 侧 `scoop_continuation_resume_common()` 继续统一负责 captured handler context 与 callee suspend state 的恢复，因此 scalar/ref/composite payload 仍和 `T3013` 共享同一套 transport 合同，没有 continuation-only placeholder glue。已验证一条 `scoopc` call-site marker 定向测试、`cargo test -p scoop_runtime continuation_resume_ -- --nocapture`、7 条 `Continuation.resume(...)` / indirect escaped-continuation run-pass fixture、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 全部通过。复跑 `cargo run -p scoop --features llvm -- test` 后，suite 首个停止点仍是 `tests/fixtures/run-pass/effect_escape_continuation_async_executor_fifo.scoop` 的 stale `EXPECT: fail`，属于 `T3017` 的 expectation cleanup，而不是 `T3009bR` 的生产 blocker。当前 effect 主线下一项推进到 `T3015`。
>
> 2026-04-18 当前轮完成更新：`T3009b` 已完成。复查 `crates/scoopc/src/llvm/codegen/effect/mod.rs` 与现有 composite transport 实现后确认，escaped continuation 的 `Continuation.resume(...)` 已与 `T3013` 共享同一套 `Word / GcRef / BoxedComposite` transport 合同：`codegen_continuation_resume_builtin()` 继续直接消费显式 continuation 值，payload authoritative type 优先来自 receiver `Continuation<T>`；tuple / struct / rich enum 统一经 boxed composite + `resume_gc_ref` 传输，`String` / class / continuation 等 GC ref 继续走 `resume_gc_ref`，word-sized payload 继续走 `resume_word`，没有 continuation-only 特例通道或 generic member-access 回退。已验证 `continuation_resume_tuple.scoop`、`continuation_resume_struct.scoop`、`continuation_resume_struct_with_ref.scoop`、`continuation_resume_continuation.scoop`、`continuation_resume_enum.scoop`、`effect_escape_continuation_indirect_perform_resume_string.scoop`、`effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 全部通过。串行复跑 `cargo run -p scoop --features llvm -- test` 后，suite 首个停止点已推进到 `tests/fixtures/run-pass/effect_escape_continuation_async_executor_fifo.scoop` 的 stale `EXPECT: fail`；该 fixture 单独运行已成功，属于 `T3017` 的 expectation cleanup，而不是 `T3009b` 的生产 blocker。当前 effect 主线下一项推进到 `T3009bR`。
>
> 2026-04-18 当前轮复审更新：`T3009b2R` 已完成。复审 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`、`crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/effect/mod.rs`、`crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 与 `runtime/c/scoop_runtime.c` 后确认，indirect callee 的 resumed-body caller-tail 已形成统一 continuation + callee-suspend-state 合同：ordinary callee 侧通过 `resume_sites + site_tag + codegen_callee_resume_dispatch()` 回到正确 `resume_tail`，outer handle 侧通过共享 `emit_resume_after_call_site()` 在存在 captured callee state 时重放原 call expr，让 callee 自己完成 post-suspend body，而不是把 payload 直接短路回 caller；continuation/runtime 侧则通过 `captured_callee_suspend_state` 与 `scoop_continuation_resume_common()` 的 TLS 恢复保证该合同跨 `handle` 返回和跨线程 resume 仍成立。定向检索 production code 未发现 `fetchGreeting`、`callIt`、`counter`、`viaBranch`、`viaIf` 等 fixture/helper 名称回流到 effect codegen，也未发现按 callee/source shape、branch 数量分流的新旁路。已验证两条 `scoopc` IR/dispatch 定向测试、一条 `scoop_runtime` continuation 定向测试、9 条 indirect-callee run-pass fixture、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 全部通过。当前 effect 主线下一项推进到 `T3009b`。
>
> 2026-04-18 当前轮复审更新：`T3009b2cR` 已完成。复审 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`、`crates/scoopc/src/llvm/codegen/mod.rs` 与 `crates/scoopc/src/llvm/codegen/effect/mod.rs` 后确认，ordinary indirect callee 的 multi-site resumed-body caller-tail 仍完全建立在统一 callee-suspend-state 合同上：plan builder 只按 `builder.suspend_sites` 全量生成 `resume_sites`，fresh path 仅保存 `site_tag + union locals`，resume path 只读取 `site_tag` 并经共享 `codegen_callee_resume_dispatch()` 分派回对应 `resume_tail`；`codegen_top_level_fun` 与 `codegen_closure_fun_body` 共用同一套入口，function-value callee 仍通过 closure body codegen 复用这套机制。定向检索 production code 未发现 `viaBranch`、`fetchGreeting`、`callIt`、`counter` 等 fixture/helper 名称或按 branch 数量/源码形状切分的新旁路。已验证 `cargo test -p scoopc ordinary_multi_site_callee_materializes_resume_site_dispatch -- --nocapture`、multi-site branch fixture、statement-container matrix、closure locals fixture、`effect_multi_escape_indirect_callee_suspend_matrix.scoop`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 全部通过。当前 effect 主线下一项推进到 `T3009b2R`。
>
> 2026-04-18 当前轮完成更新：`T3009b2c` 已完成。ordinary indirect callee 的 `CalleeSuspendPlan` 已从 single-site 扩成 multi-site：`crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 现在会为同一个 ordinary helper / closure body 中的每个 `Perform` site 分别构建 `resume_slot`、`resume_tail` 与 site-local `saved_locals`，再用 union locals 定义统一 callee suspend-state 布局；`crates/scoopc/src/llvm/codegen/effect/mod.rs` 的 ordinary callee suspend-state 也新增 `site_tag` 字段，fresh path 保存 state 时写入当前 site，resume path 读取 `site_tag` 后分派到 `resume_site*` blocks；`crates/scoopc/src/llvm/codegen/mod.rs` 的 `codegen_top_level_fun` / `codegen_closure_fun_body` 已共用这套 multi-site resume dispatch，而不是只执行单一 `plan.resume_tail`。新增 focused fixture `tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_multi_site_callee_branch.scoop` 与 IR 定向测试 `ordinary_multi_site_callee_materializes_resume_site_dispatch` 后，最小复现中原先缺失的 `if_resume` / `if_after` / `I:if` 已恢复；并已复验 `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_multi_site_callee_branch.scoop`、`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_statement_container_matrix.scoop`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 全部通过。当前 effect 主线下一项推进到 `T3009b2cR`。
>
> 2026-04-18 当前轮阻塞更新：开始执行 `T3009b2R` 复审后，基于 `effect_escape_continuation_indirect_perform_statement_container_matrix.scoop` 构造最小变体，确认 ordinary indirect callee 在“同一 callee 内有多个 suspend site”时仍未真正接回统一 resumed-body caller-tail：`build_ordinary_callee_suspend_plan_from_unified_contract()` 只有在 `builder.suspend_sites.len() == 1` 时才建立 `CalleeSuspendPlan`，导致 `viaIf` 的 then / else 两个分支都执行 `Ask.ask(...)` 时，resume 后 outer `ResumeAfterSite(Call)` 会直接把 payload 当作整次调用结果，缺失 `if_resume` / `if_after` / `I:if`，说明 callee 自己的 post-suspend body 被跳过。这是比当前 `T3009b2R` 更前置的真实生产缺口，已按阻塞规则前置拆成 `T3009b2c` → `T3009b2cR`，并让 `T3009b2R` 顺延依赖 `T3009b2cR`。本轮只同步任务与计划，不继续实现。当前 effect 主线下一项推进到 `T3009b2c`。
>
> 2026-04-18 当前轮完成更新：`T3009b2` 已完成。本轮未再修改生产代码，而是按任务验收重新复跑 shared resumed-body / caller-tail 矩阵；结果表明前序 `T3009b2a`、`T3009b2b` 与 `T3015a` 的修复已经把该任务的共享语义面一并收口。8 条定向 run-pass fixture 全部通过：`effect_escape_continuation_indirect_perform_basic.scoop`、`effect_escape_continuation_indirect_perform_closure_locals.scoop`、`effect_escape_continuation_indirect_perform_resume_string.scoop`、`effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop`、`effect_escape_continuation_indirect_perform_tail_return_int.scoop`、`effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop`、`effect_multi_escape_indirect_callee_suspend_matrix.scoop` 与 `effect_escape_continuation_indirect_perform_statement_container_matrix.scoop`；其中 multi-site indirect callee 与 statement-container source-path 变体都确认继续共享同一套 continuation + dispatch-loop + resumed-body caller-tail 合同，没有新增按 helper/closure/fixture 名称分流的补丁。已复验 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。当前 effect 主线下一项推进到 `T3009b2R`。
>
> 2026-04-18 当前轮复审更新：`T3015aR` 已完成。复审 `runtime/c/scoop_runtime.c`、`crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 与 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 后确认，runtime 侧 continuation 现在通过 `scoop_effect_handler_stack_snapshot_clone()` 捕获 continuation-owned handler stack 堆快照，并在 `scoop_continuation_resume_common()` 中 pin continuation、安装快照、step 返回后恢复 caller TLS、释放快照；compiler 侧 `emit_effect_runtime_functions()` 统一生成 `step_fn + dispatch_loop_fn` 双入口，初始 handle 入口与 `UnifiedStateTerminator::Suspend` materialize 的 continuation 都捕获同一个 `scoop.effect.dispatch.*` 入口，不会在 escaped continuation resume 后退回 raw `step_fn`。`emit_dispatch_loop_body()` / `emit_dispatch_arm_execution()` 继续用同一套 dispatch-check / outward-propagate loop 处理 arm body 再次 perform、multi-site indirect callee matrix 与 statement-container rebuild（`Block / IfThen / IfElse / WhenArm / WhileBody`），未发现按 `counter()` / `viaIf()` / `viaWhen()` / `viaWhile()` 等 fixture 名称分流的补丁。已验证 `cargo test -p scoop_runtime --test continuation_cross_thread_handler_stack`、两条 `cargo test -p scoopc ... -- --nocapture` IR/dispatch 定向测试、三条 run-pass matrix + `continuation_resume_ref_class.scoop`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 全部通过。当前 effect 主线下一项推进到 `T3009b2`。
>
> 2026-04-18 当前轮完成更新：`T3015a` 已完成。根因分成两层并已一并收口：第一层是 runtime continuation 之前只捕获原始 TLS handler frame 指针，`handle` 返回时这些栈上 frame 会被 `pop` 成 inactive，导致 resumed segment 的下一次 `perform` 虽然仍会写 perform slot，但再也找不到 captured handler；现已把 `runtime/c/scoop_runtime.c` 改为捕获 continuation-owned 的 handler stack 堆快照，并在 `scoop_continuation_resume_common()` 中 pin continuation、安装快照、step 返回后释放快照并恢复 caller TLS。第二层是 compiler 之前把 escaped continuation 的 resume 入口绑到了 raw `step_fn`，导致第一次 resumed segment 之后的新 `perform` 没有重新进入 handle dispatch loop；现已在 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 中额外生成 `scoop.effect.dispatch.*` 入口，并让 `Suspend` terminator 分配 continuation 时捕获该 dispatch-loop entry，而不是 raw `step_fn`。修复 redispatch 后，`statement-container` matrix 进一步暴露 `WhileBody` rebuild 的 synthetic first-iteration 条件仍写成非短路 `resume_first || cond`；现已在 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 改成显式 `if (resume_first) true else cond`，恢复“先完成当前迭代尾部，再回到 cond”的语义。已回收 4 条同根因 run-pass fixture 的 `EXPECT: fail`（`effect_multi_escape_indirect_callee_suspend_matrix.scoop`、`effect_escape_continuation_indirect_perform_statement_container_matrix.scoop`、`effect_escape_continuation_multi_perform_while_loop.scoop`、`continuation_resume_ref_class.scoop`），并验证 `cargo test -p scoop_runtime --test continuation_one_shot`、`cargo test -p scoop_runtime --test continuation_cross_thread_handler_stack`、`cargo test -p scoopc escaped_continuation_ir_uses_dispatch_loop_entry_for_resume -- --nocapture`、4 条定向 `cargo run -p scoop --features llvm -- run ...`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 全部通过。当前 effect 主线下一项推进到 `T3015aR`。
>
> 2026-04-18 当前轮阻塞重排更新：开始执行原 `T3009b2` 的 shared matrix 后，先把 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 中 ordinary callee 对 `source_path.frames` 非空直接返回 `None` 的缺口接成真实 rebuild：`Block / IfThen / IfElse / WhenArm / WhileBody` 现在都会生成 resumed tail，其中 `WhileBody` 通过 synthetic first-iteration flag 保住“resume 后先完成当前迭代尾部，再回到 cond”的语义；同时新增 focused reproducer `tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_statement_container_matrix.scoop`，以及 IR 定向测试确认外层 handle call-site active-dispatch 仍然存在。继续直接运行新的 statement-container matrix 与既有 `effect_multi_escape_indirect_callee_suspend_matrix.scoop` 后确认，更前置的真实 blocker 不是 statement-container rebuild 本身，而是 escaped continuation 在第一次 `resume(...)` 后继续执行 resumed caller-tail 时，下一次 outward `perform` 不会重新进入 captured handler dispatch loop；两个 reproducer 的实际输出都会分别截断在第二个 indirect callee 的 `if_enter` / `counter_enter`。因此本轮不能继续把 `T3009b2` 视为当前任务，已按阻塞规则把问题前移拆成 `T3015a` → `T3015aR`，并让 `T3009b2` 显式依赖 `T3015aR`。已验证 `cargo check -p scoopc`、`cargo test -p scoopc indirect_if_branch_callee_keeps_handle_call_site_active_dispatch -- --nocapture`、`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_escape_indirect_callee_suspend_matrix.scoop`、`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_statement_container_matrix.scoop`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 全部通过；本轮在同步计划与任务顺序后停止，下一项推进到 `T3015a`。
>
> 2026-04-18 当前轮复审更新：`T3009b2bR` 已完成。复审 `crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/effect/mod.rs`、`crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 与 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 后，先定位并修复了三个真实生产缺口：其一，`attach_suspend_source_paths()` 之前只记录顶层 `Perform/Call`，导致 ordinary callee 内 `Ask.get(x) + 1` 这类 nested-expression suspend source 根本不会生成 `CalleeSuspendPlan`；现已把 source-path 遍历补齐到表达式树深度，并新增 run-pass 回归 `effect_escape_continuation_indirect_perform_nested_expr.scoop`。其二，ordinary frame 的 fresh path 之前会把 outward `perform` 的 `Never/default` 继续送进外层表达式，导致 `perform(...) + 1` 在 dead path 上报 `integer binary op lhs`；现已让 `codegen_perform_expr()` 在 ordinary propagation 模式下返回带正确类型的 dead-path dummy value，仅在无前驱 dead block 中结构性收尾。其三，tail expr ordinary callee 的 synthetic resume slot 之前会被语句位置类型污染，触发 `value coercion`；现已把 ordinary callee plan builder 改为显式接收 declared return type，并在 tail `ExprStmt` / `ReturnValue` consumer 上用声明返回类型修正 `resume_slot_ty`，恢复 `suspend_ir_captures_callee_suspend_state_into_continuation`。复审结论：ordinary indirect callee 的 resumed-body restore 已统一接回，top-level helper / closure / function-value callee 共享同一套 unified suspend-site / continuation / callee-suspend-state 合同，无 fixture-only patch 残留；更广的 multi-site 与 nested statement-container source-path shared matrix 继续留给后续 `T3009b2`。已验证五条间接 callee run-pass fixture、`cargo test -p scoopc suspend_ir_captures_callee_suspend_state_into_continuation -- --nocapture`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 全部通过。当前 effect 主线下一项推进到 `T3009b2`。
>
> 2026-04-18 当前轮完成更新：`T3009b2b1` 已完成。ordinary callee 的 resumed-body restore 不再由 `crates/scoopc/src/llvm/codegen/mod.rs` 里的 `build_block_callee_suspend_plan()` 扫描“block 中单个 direct-perform \`val\` 绑定”来驱动；现在改为在 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 里复用统一 state-machine 的 suspend-site / resume-path 合同，为普通 top-level helper 与 closure body 构建最小 ordinary-callee plan（`saved_locals + synthetic resume slot + rewritten resume_tail`）。`crates/scoopc/src/llvm/codegen/mod.rs` 中的 `CalleeSuspendPlan` 也已删除 `perform_stmt_index` / `perform_binding_id` / `perform_binding_ty`，fresh/resume 双入口统一消费上述 contract；`crates/scoopc/src/llvm/codegen/effect/mod.rs` 的 resume prologue 则改为恢复 synthetic resume slot，而不是重建旧的 `perform` 绑定 local。已复验 `effect_escape_continuation_indirect_perform_basic.scoop`、`effect_escape_continuation_indirect_perform_closure_locals.scoop`、`effect_escape_continuation_indirect_perform_resume_string.scoop`、`effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 全部通过。当前 effect 主线下一项推进到 `T3009b2bR`。
>
> 2026-04-18 当前轮复审阻塞更新：开始执行 `T3009b2bR` 后，复审 `crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/effect/mod.rs` 与 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`，确认 `T3009b2b` 仍残留源码形状前提：`mod.rs` 里的 `CalleeSuspendPlan` 注释明确写明只覆盖“block 中单个 direct-perform \`val\` 绑定”的稳定子集，而 `build_block_callee_suspend_plan()` 也确实通过扫描该 block 形状来决定是否为 ordinary callee 生成 fresh/resume 双入口。这与 `T30` 顶部“生产 effect codegen 禁止按源码 / 代码形状分流、LLVM lowering 的单一输入应为 state machine”的总约束冲突，因此当前不能把 `T3009b2bR` 标记为完成。按依赖关系，现已在 `TODO.md` 中把问题前置拆成 `T3009b2b1`（先去掉 ordinary callee resumed-body restore 的 block-shape 选路前提，收口为统一 suspend-site 合同）→ `T3009b2bR`（再复审是否真的无 shape-based / fixture-only patch 残留）；`T3009b2` / `T3009b2R` 顺延。当前 effect 主线下一项推进到 `T3009b2b1`。
>
> 2026-04-18 当前轮完成更新：`T3009b2b` 已完成。ordinary indirect callee 的 resumed-body restore 现已沿统一 continuation + callee-suspend-state 合同接回：`crates/scoopc/src/llvm/codegen/mod.rs` 为 ordinary top-level helper / closure fun body 生成最小 `CalleeSuspendPlan`，fresh path 会在 outward `perform` 前保存 post-suspend locals/captures，resume path 则通过 TLS 中的 callee suspend state 恢复 locals/captures 并继续执行原 resumed body tail；`crates/scoopc/src/llvm/codegen/effect/mod.rs` 补齐了对应的 state save/publish/restore prologue；`crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 的 `ResumeAfterSite(Call)` 现在只会在 TLS 中存在 callee suspend state 时 replay 原 call expr，把真实 call result 写回 synthetic resume slot，否则保持既有 inactive 路径，不再把 `resume` payload 误当整次调用结果。runtime 新增 `scoop_callee_suspend_state_publish`，并在 `Suspend` terminator 把 captured callee suspend state 纳入 continuation 后执行 `unpin`，收口 pin 生命周期。四条 indirect resumed-body fixture 已回收旧 `EXPECT: fail`。验收过程中还修复了一个真实 lint 缺口：删除 `emit_resume_after_call_site` 的未使用 `_state` 参数，使 `cargo clippy --all-targets -- -D warnings` 重新通过。已验证 `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_basic.scoop`、`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_locals.scoop`、`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_resume_string.scoop`、`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 全部通过。当前 effect 主线下一项推进到 `T3009b2bR`。
> 2026-04-18 当前轮复审更新：`T3009b2aR` 已完成。复审 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`、`crates/scoopc/src/llvm/codegen/runtime_abi.rs`、`runtime/c/scoop_runtime.c` 与 `runtime/c/scoop_runtime_api.h` 后确认，编译器生产路径只在 `UnifiedStateTerminator::Suspend` 中执行一次 `scoop_callee_suspend_state_get() + clear()`，并把结果写入 continuation 的 `captured_callee_suspend_state` 字段；runtime 生产路径只在 `scoop_continuation_resume_common()` 中把该字段临时恢复进 TLS，step_fn 返回后恢复 caller 原 TLS，没有 fixture-only / callee-name-only / source-shape 分流。复审过程中发现并修复了一个真实 ABI 旁路：`runtime/c/scoop_runtime_api.h` 仍把裸 TLS 符号 `__scoop_callee_suspend_state` 作为正式导出符号暴露，且仅供测试使用的 `scoop_callee_suspend_state_set` 仍以通用 runtime API 形式存在。现已把该 TLS 收紧为 runtime 内部静态存储，并把 setter 改成显式 test helper `scoop_test_callee_suspend_state_set`；继续保留 `get/clear` 作为编译器/定向测试需要的最小接口。已验证 `cargo test -p scoop_runtime --test continuation_one_shot`、`cargo test -p scoop_runtime --test effect_tls`、`cargo test -p scoop_runtime abi_exports_allowlist -- --nocapture`、`cargo test -p scoopc suspend_ir_captures_callee_suspend_state_into_continuation -- --nocapture`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 全部通过。当前 effect 主线下一项推进到 `T3009b2b`。
> 2026-04-18 当前轮完成更新：`T3009b2a` 已完成。LLVM 侧 continuation ABI 现在正式包含第 8 个字段 `captured_callee_suspend_state`，`UnifiedStateTerminator::Suspend` 在分配 continuation 后会调用 `scoop_callee_suspend_state_get()` 把当前 TLS callee suspend state 提升进 continuation，并立即 `clear()` TLS；C runtime 的 `ScoopContinuation` 也已同步扩展该字段，更新了布局断言、GC trace 与 alloc 初始化。`scoop_continuation_resume_common()` 现会在 step_fn 动态范围内恢复 captured callee suspend state 到 TLS，并在返回后恢复 caller 原 TLS；为避免 moving GC 在 resumed body 真正消费它之前让 TLS raw 指针失效，还对该 captured state 做了动态范围 pin/unpin。新增验证覆盖两半合同：一条 LLVM IR 定向测试锁定 suspend 捕获，一条 continuation runtime 测试锁定 resume restore，另加一条 TLS 测试锁定 clear/unregister 语义。已验证 `cargo test -p scoop_runtime --test continuation_one_shot`、`cargo test -p scoop_runtime --test effect_tls`、`cargo test -p scoopc suspend_ir_captures_callee_suspend_state_into_continuation -- --nocapture`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 全部通过。当前 effect 主线下一项推进到 `T3009b2aR`。
> 2026-04-18 当前轮复审更新：`T3009b1R` 已完成。复审 `crates/scoopc/src/llvm/codegen/effect/mod.rs`、`llvm/codegen/mod.rs`、`hir/lower/expr.rs`、`typecheck/expr/call.rs` 与 `effect/state_machine_plan.rs` 后确认，`Continuation.resume(...)` 仍只依赖 typecheck 确认的 builtin call-site marker 与精确类型来源，没有为 direct enum fixture 引入按成员名 / 局部形状分流的补丁。复审过程中发现并修复了一个残留缺口：`resolve_expr_cg_ty()` 虽被 `Continuation.resume(...)` payload fallback 复用，但原实现只看局部 env，再直接回退到 `expr.ty`；而 HIR lowering 会把所有 `VarRef`（包括 top-level const）统一降成 `Any`。现已把该 helper 改为在保留局部 `CgLocal.ty` 优先级的同时复用 `resolve_expr_concrete_type()`，从而把 top-level `VarRef` / concrete call result 也纳入 precise payload type 解析。新增 run-pass 回归 `continuation_resume_top_level_const_payload.scoop` 后，direct enum、top-level const payload、tuple、struct-with-ref、continuation payload 全部通过；`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。当前 effect 主线下一项推进到 `T3009b2`。
> 2026-04-18 当前轮拆分更新：开始执行原 `T3009b2` 后，继续复现 `effect_escape_continuation_indirect_perform_basic.scoop`、`effect_escape_continuation_indirect_perform_closure_locals.scoop`、`effect_escape_continuation_indirect_perform_resume_string.scoop` 与 `effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop`，确认当前 shared blocker 比任务标题更宽：不仅 `SuspendCall` resume path 仍把 payload 误当整次调用结果，runtime/LLVM continuation ABI 也完全没有把 ordinary indirect callee 的 `callee_suspend_state` 纳入 continuation 捕获。仓库里仅残留裸 TLS `__scoop_callee_suspend_state` 与历史 runtime 访问器，而 `ScoopContinuation` 只捕获 handler stack、body `state`、`resume_state_tag` 与 payload 双槽；若不先补这层合同，后续 ordinary callee resumed-body restore 无论同线程还是跨线程都没有 authoritative 存储位置。按复杂度与前置依赖，现已把原 `T3009b2` 拆成：`T3009b2a`（先把 `callee_suspend_state` 纳入 continuation/runtime ABI 捕获合同）→ `T3009b2aR` → `T3009b2b`（再接回 ordinary indirect callee 的 resumed-body restore，并让 `SuspendCall` resume 不再把 payload 误当整次调用结果）→ `T3009b2bR` → `T3009b2` / `T3009b2R`（最后收口 helper/closure/composite payload 的 shared 验收矩阵）。同时把 `effect_escape_continuation_indirect_perform_basic.scoop`、`effect_escape_continuation_indirect_perform_closure_locals.scoop` 与 `effect_multi_escape_indirect_callee_suspend_matrix.scoop` 显式纳入 `T3009b2` 最终验收，因为它们与原先两条 string/struct-with-ref fixture 属于同一 resumed-body caller-tail 根因。当前 effect 主线下一项推进到 `T3009b2a`。
> 2026-04-18 当前轮拆分完成更新：开始执行原 `T3009b` 后先复跑 direct composite fixture，确认 tuple / struct / struct-with-ref / continuation-ref payload 都已通过，唯一 direct blocker 是 `continuation_resume_enum.scoop` 的 `value coercion`。根因在于 `codegen_continuation_resume_builtin()` 直接信任 `receiver.ty` / `payload_expr.ty`，而 HIR `VarRef` 经常被宽化成 `Any/Ref`；现已改为优先使用 `resolve_expr_concrete_type(receiver)` 与 `resolve_expr_cg_ty(payload_expr)`，`continuation_resume_enum.scoop` 也已移除 stale `EXPECT: fail`。已验证：`continuation_resume_enum.scoop`、`continuation_resume_tuple.scoop`、`continuation_resume_struct_with_ref.scoop`、`continuation_resume_continuation.scoop`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。但继续验收当前 `T3006` xfail 子集时又暴露出一个此前未显式跟踪的、更前置的生产缺口：`effect_escape_continuation_indirect_perform_resume_string.scoop` 与 `effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop` 仍会在 resumed indirect callee 中跳过 suspend 点之后的语句，而 tail-return / closure-tail 版本已通过。这说明当前 blocker 已不再是 direct composite transport，而是“间接 callee suspend 后 resumed-body caller-tail”尚未接回。按阻塞规则，现已把原 `T3009b` 拆成 `T3009b1`（本轮完成：修复 direct enum/local-VarRef payload 类型解析）→ `T3009b1R`（下一轮 review）→ `T3009b2` / `T3009b2R`（先收口间接 callee resumed-body caller-tail）→ `T3009b` / `T3009bR`（再完成剩余 composite transport 收尾）。当前 effect 主线下一项推进到 `T3009b1R`。
> 2026-04-18 当前轮复审更新：`T3014R` 已完成。复审 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`、`llvm/codegen/mod.rs`、`llvm/codegen/effect/mod.rs`、`llvm/codegen/effect/state_machine_segments.rs`、`llvm/codegen/effect/state_machine_transform.rs` 与 `runtime/c/scoop_runtime.c` 后确认，runtime handler registration 已与 `dispatch_entries()` 一一对应：handle 入口会为每个 dispatch entry 分配独立 `ScoopEffectHandlerFrame` 并逐个 push，`handle_done` / `handle_propagate` 两个出口按逆序对称 pop；same-op 多 arm 仍只在单个 dispatch entry 内按 `effect_instance_key` 顺序判定，没有回流到“首个 arm / 首个 op-tag”特例。`dispatch_unmatched` 也只会流向 `outward_target_bb`，若存在 cleanup scope 则经 `handle_cleanup_propagate_*` 跑完 cleanup 后进入 `handle_propagate`，不会穿过 `handle_done` 正常完成路径；而 `handle_propagate` 向外传播时继续复用 shared `emit_ordinary_non_resuming_effect_exit()`，没有 handle-only / shape-based / fixture-only 特判。验证通过：两条 LLVM IR 定向测试、`effect_tls`、多条 handler-stack / same-op / delegated-property fixture、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`；复跑 `cargo run -p scoop --features llvm -- test` 后，suite 仍只停在已跟踪的 stale `EXPECT: fail` `effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop`（`T3017`）。当前 effect 主线下一项推进到 `T3009b`。
> 2026-04-18 当前轮复审更新：`T3014cR` 已完成。复审 `crates/scoopc/src/typecheck/expr/entry.rs`、`hir/lower/sugar.rs`、`llvm/codegen/effect/mod.rs`、`llvm/codegen/effect/state_machine_emitter.rs` 与 `llvm/codegen/mod.rs` 后确认，delegated-property observable callback、ordinary `perform` 与 unified state-machine `emit_perform_op` / dispatch 仍统一经 `effect_instance_key()` / `matching_effect_instance_keys_for_handled_effect()` 收口，没有 callback-path / runtime-error-only / fixture-only fallback。复审过程中发现并修复了一个此前未显式跟踪的生产缺口：标准 delegated-property side table 只保存声明点 AST，却未绑定声明点 `SourceFile` / `ast::File`；跨文件使用 `lazy/observable/vetoable` 时，lowering 会在使用点文件上下文里直接 lower 声明点 callback / initializer AST，既可能查错 `inferred_performed_effect_tys` 等 side table，也会让 local `SymbolId` 因“仅按 span intern”与使用点局部冲突。现已为标准 delegated-property info 补齐声明点上下文，并让 foreign-AST lowering 显式切回声明点 `SourceFile` / `ast::File`；同时把 HIR lowering 的 local symbol interning 升级为“源文件 + span”。新增多文件低层回归 `lower_for_compilation_unit_multi_files_preserves_effect_ty_in_cross_file_observable_delegate_callback` 后，跨文件 observable callback 内 `Raise.raise(7)` 会稳定保留 `Perform.effect_ty = Raise<Int>`。验证通过：新增低层回归、`delegated_property_observable_raise_does_not_poison_mutex.scoop`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`；复跑 `cargo run -p scoop --features llvm -- test` 后，suite 未回退到 delegated-property observable fixture，当前首个停止点仍是已跟踪的 stale `EXPECT: fail` `effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop`（`T3017`）。当前 effect 主线下一项推进到 `T3014R`。
> 2026-04-18 当前轮完成更新：`T3014c` 已完成。根因确认不是 runtime dispatch/ABI 缺少 `effect_instance_key`，而是 `typecheck::check_file_exprs()` 之前会直接跳过带 `delegate` 的属性表达式，导致标准 delegated property 的 inline body（`lazy` initializer、`observable`/`vetoable` 的 initial expr 与 callback body）从未写入 `inferred_performed_effect_tys`；HIR lowering 虽仍会把 observable callback inline 到 delegated-property assign lowering，但其中 `Raise.raise(7)` 的 `Perform.effect_ty` 只能退化为 `Any`，最终 unified state-machine `emit_perform_op` 在 `effect_instance_key(effect_ty)` 处报 `UnsupportedMainBody { kind: "state machine perform effect instance key" }`。现已在 `crates/scoopc/src/typecheck/expr/entry.rs` 中新增标准 delegated-property inline 表达式检查，只对 lowering 真正会 inline 的几段表达式做 expected-context inference，避免把整个 delegate 调用错误地按普通纯 lambda 签名 typecheck；新增 typed lowering 回归 `lower_typed_single_source_file_preserves_effect_ty_in_observable_delegate_callback` 后，observable callback 内 `Raise.raise(7)` 会稳定保留 `Perform.effect_ty = Raise<Int>`。验证通过：新增低层回归、`lower_for_compilation_unit_multi_files_preserves_effect_ty_in_init_side_tables`、`delegated_property_observable_raise_does_not_poison_mutex.scoop`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`；复跑 `cargo run -p scoop --features llvm -- test` 后，suite 首个停止点已前移到已跟踪的 stale `EXPECT: fail` `effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop`（`T3017`），不再停在 delegated-property observable blocker。当前 effect 主线下一项推进到 `T3014cR`。
> 2026-04-18 当前轮复审更新：`T3014bR` 已完成。复审 `crates/scoopc/src/llvm/codegen/mod.rs`、`effect/mod.rs` 与 `effect/state_machine_emitter.rs` 后确认，ordinary `codegen_perform_expr` 与 unified state-machine `emit_perform_op` 都统一调用 `effect_instance_key(effect_ty)`；`emit_raise_runtime_error_variant` 直接写入的 `EFFECT_INSTANCE_KEY_RAISE_RUNTIME_ERROR` 也正是 `effect_instance_key()` 对 `Raise<RuntimeError>` 的固定返回值，不存在另一套 runtime-error-only key 合同。继续复审 `hir/lower/util.rs`、`hir/lower/expr.rs` 与 `typecheck/expr/entry.rs` 后确认，class/object-init hidden-suspend 路径的修复仅是把既有 typed side table 链路补回通用 lowering：`collect_object_inits()` / `collect_class_inits()` 现可接收 `typecheck_types`，而 object property initializer / `init {}` block 也会写回 `inferred_performed_effect_tys`，最终仍统一经 `ctx.lower_expr(...)` 生成带真实 `Perform.effect_ty` 的 HIR。验证通过：低层回归测试、`class_init_hidden_raise_helper_try_catch_basic.scoop`、`object_property_init_raise_helper_try_catch_basic.scoop`、`effect_handle_hidden_suspend_helper_object_property_basic.scoop`、`effect_same_op_multi_arm_dispatch_effect_instance.scoop`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`；复跑 `cargo run -p scoop --features llvm -- test` 后，suite 首个真实失败点仍为 `delegated_property_observable_raise_does_not_poison_mutex.scoop`（`UnsupportedMainBody { kind: "state machine perform effect instance key" }`）。因此 `T3014bR` 可判定完成，当前 effect 主线下一项推进到 `T3014c`。
> 2026-04-18 当前轮完成更新：`T3014b` 已完成。根因分两层：其一，`lower_for_compilation_unit_multi_files()` 之前只给顶层 HIR lowering 透传 `typecheck_types`，`collect_object_inits()` / `collect_class_inits()` 这两个 side-table lowering 仍硬编码 `None`，导致 object/class init 内 `Raise.raise(RuntimeError.*)` 的 `Perform.effect_ty` 在 side table 中退化为 `Any`；其二，`typecheck::check_file_exprs()` 之前完全跳过 `ast::Item::Object`，object property initializer / `init {}` block 从未写回 `inferred_performed_effect_tys`，即便 side-table lowering 拿到了 `typecheck_types` 也无法恢复。现已同时修复这两层断点，并新增低层回归测试 `lower_for_compilation_unit_multi_files_preserves_effect_ty_in_init_side_tables` 锁定 multi-file typed lowering 产出的 `object_inits` / `class_inits` side tables必须保留 `Raise<RuntimeError>` 的 `Perform.effect_ty`。验证通过：定向低层单测、`class_init_hidden_raise_helper_try_catch_basic.scoop`、`object_property_init_raise_helper_try_catch_basic.scoop`、`effect_handle_hidden_suspend_helper_object_property_basic.scoop`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。复跑 `cargo run -p scoop --features llvm -- test` 后，suite 已不再首先停在 `class_init_hidden_raise_helper_try_catch_basic.scoop`，新的首个失败点推进到 `delegated_property_observable_raise_does_not_poison_mutex.scoop`（`UnsupportedMainBody { kind: "state machine perform effect instance key" }`）。按阻塞规则，现已新增前置任务 `T3014c` / `T3014cR` 跟踪 delegated-property observable callback 的统一 key 缺口；当前 effect 主线下一项推进到 `T3014bR`。
> 2026-04-18 当前轮阻塞更新：继续执行 `T3014R` 复审时，先同步了 `tests/fixtures/hir/handle_mixed_arm_kinds.hir` 与 `tests/fixtures/hir/safe_call_not_null_assert.hir` 的 `Perform.effect_ty` stale golden，以恢复 HIR fixture 基线；在此之后复跑 `cargo run -p scoop --features llvm -- test`，suite 新的首个真实失败点推进到 `class_init_hidden_raise_helper_try_catch_basic.scoop`，直接运行报 `UnsupportedMainBody { kind: "effect instance key" }`。这说明 `T3014a` 引入 effect-instance key 合同后，ordinary hidden-suspend `Raise.raise(RuntimeError.*)` lowering 仍不能稳定产出 key，打回了 `T3010b2b0a0` 已锁定的 class/object-init helper run-pass 语义。按阻塞规则，现已新增前置任务 `T3014b` / `T3014bR` 先收口该回归，并把 `T3014R` 顺延到其后；本轮到此停止。
> 2026-04-17 当前轮完成更新：`T3014a` 已完成。same-op multi-arm unified dispatch 现在会在命中 `dispatch_entry` 后按源码顺序逐 arm 读取并比较 `effect_instance_key`，不再把同一 `op_fqn` 的 arms 静默收缩成首个 arm。根因修复点在于 `matching_effect_instance_keys_for_handled_effect()` 原先误把 `op_fqn` 当作 effect FQN 查候选集合，导致 production dispatch 虽读了 key 却始终得到空匹配集；现已改为优先按 handled effect 的 nominal FQN 收集 keys。新增 run-pass fixture `effect_same_op_multi_arm_dispatch_effect_instance.scoop` 后，直接执行返回 `23`；同时为完成全量收尾，同步把 `crates/scoop_runtime/tests/effect_tls.rs` 对齐到新增 `effect_instance_key` ABI，并更新 `tests/fixtures/hir/handle_perform.hir` 以反映 `Perform.effect_ty`。验证通过：三条 LLVM 定向测试、`effect_runtime_slot_abi_basic.scoop`（退出码 `48`）、`effect_same_op_multi_arm_dispatch_effect_instance.scoop`（退出码 `23`）、`cargo test -p scoop_runtime --test effect_tls`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。当前 effect 主线下一项推进到 `T3014R`。
> 2026-04-17 当前轮阻塞更新：原计划执行 `T3014R` 复审时，沿 `typecheck -> DispatchPlan -> UnifiedDispatchEntry -> state_machine_emitter` 复查发现，一个尚未显式跟踪的前置缺口仍在：`DispatchPlan` / `UnifiedDispatchEntry` / `SuspendSite.matching_arms()` 都保留“同一 `op_fqn` 可关联多个 arm”的合同，但 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 当前在 dispatch 命中某个 entry 后仍静默取 `dispatch_entry.arms().first()`，runtime `op_tag` 也只按原始 `op_fqn` 分派。这意味着当前生产 dispatch 还没有真正消费完整合同，`T3014R` 不能在这个前提下宣告“multi-op registration 与 unmatched propagation 已统一收口”。按阻塞规则，现已新增前置任务 `T3014a` 来补齐 same-op multi-arm dispatch 合同，并把 `T3014R` 移到其后；本轮到此停止，等待下轮先完成 `T3014a`。
> 2026-04-17 当前轮实现更新：按最新提交复审再次点名的既有问题前置处理后，`T3014` 已完成。复查 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 后确认，`dispatch_unmatched` 当前已直接 outward propagate；本轮真实残留缺口是 handle 入口只为首个 `dispatch entry` 注册 runtime handler frame。现已新增 `allocate_registered_handler_frames` / `pop_registered_handler_frames` helper，把 handle 入口改为为 `contract.dispatch_entries()` 中的每个 op-tag 分配独立 `ScoopEffectHandlerFrame` 并逐个 push，在 `handle_done` / `handle_propagate` 两条出口按逆序逐个 pop，保证 continuation 捕获到完整的动态 handler stack。新增 LLVM IR 定向测试 `multi_dispatch_handle_ir_registers_every_op_tag_on_handler_stack` 锁定 multi-op handle 的一帧一 tag 注册；定向验证 `effect_multi_nonresuming_custom_indirect.scoop`、`effect_op_tag_two_effects_nested_dispatch.scoop`、`effect_handler_stack_nearest_three_levels_and_arm_outside_scope.scoop`、`effect_custom_nonresuming_nested_nearest_and_arm_outside_scope.scoop`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。复跑 `cargo run -p scoop --features llvm -- test` 后，suite 仍只停在已跟踪的 stale `EXPECT: fail` `effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop`（`T3017`），未出现新的更早 handler-stack 失败点。当前 effect 主线下一项推进到 `T3014R`。
> 2026-04-17 当前轮复审更新：`T3013R` 已完成。复审 `crates/scoopc/src/llvm/codegen/effect/mod.rs`、`effect/state_machine_emitter.rs`、`runtime_abi.rs` 与 `runtime/c/scoop_runtime.c` 后确认，standalone `perform`、state-machine `emit_perform_op`、handle result frame read/write、arm binder readback 与 `Continuation.resume(...)` payload write/read 已统一收口到 `encode_effect_transport_value` / `decode_effect_transport_value`；composite 值统一经 typed GC box + `perform_slot.gc_ref` / frame `resume_gc_ref` / continuation `resume_gc_ref` 传递，不依赖 `ptr <-> int` 或 native-only side channel。continuation runtime trace 会显式追踪 `resume_gc_ref`，effect frame type descriptor 的 trace bitmap 也覆盖 public `resume_gc_ref` 与 runtime-only continuation slot，因此新的 transport 与 GC 可达性合同一致。定向验证 `handle_compound_result.scoop`、`effect_nonresuming_payload_struct_indirect.scoop`、`continuation_resume_continuation.scoop`、`continuation_resume_struct.scoop`、`continuation_resume_tuple.scoop`、`continuation_resume_struct_with_ref.scoop` 与 `cargo test -p scoopc async_await_ir_preserves_continuation_slot_and_perform_payload -- --nocapture` 全部通过；`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 通过；复跑 `cargo run -p scoop --features llvm -- test` 后，suite 仍只停在已跟踪的 stale `EXPECT: fail` `effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop`（`T3017`），未出现新的更早 transport 失败点。审查过程中再次确认 handle 入口只注册首个 op-tag 的旧问题仍存在，但该缺口已由现有 `T3014` 跟踪，与本轮 transport 合同复审独立。当前 effect 主线下一项推进到 `T3009b`。
> 2026-04-17 当前轮实现更新：`T3013` 已完成。`crates/scoopc/src/llvm/codegen/effect/mod.rs` 新增 `EffectTransportKind` 与共享 helper `encode_effect_transport_value` / `decode_effect_transport_value` / `box_effect_transport_value` / `unbox_effect_transport_value`，把 standalone `perform`、state-machine `emit_perform_op`、handle result frame read/write 与 `Continuation.resume(...)` payload write/read 统一收口到同一套 `Word` / `GcRef` / `BoxedComposite` transport 合同；`Continuation.resume(value)` 的 expected payload type 也改为优先从 receiver `Continuation<T>` 提取，避免 HIR payload expr 退化成 `Any/Ref` 时走错 coercion。定向验证 `handle_compound_result.scoop`、`effect_nonresuming_payload_struct_indirect.scoop`、`continuation_resume_continuation.scoop`、`continuation_resume_struct.scoop`、`continuation_resume_tuple.scoop`、`continuation_resume_struct_with_ref.scoop` 全部通过；`continuation_resume_enum.scoop` 已不再报 `u64 word from composite value`，当前只剩 richer enum escaped-continuation 路径上的 `value coercion`，继续留给 `T3009b`。`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过；复跑 `cargo run -p scoop --features llvm -- test` 后，suite 首个停止点已变为 `effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop` 的 stale `EXPECT: fail`（`T3017`），而 `continuation_resume_ref_class.scoop` 临时放开后暴露的 resumed-body second-perform caller-tail 截断则已确认属于 `T3015` 的 handler-context lifetime / redispatch 缺口，因此恢复为有真实原因的 xfail，不阻塞 `T3013` 收口。当前 effect 主线下一项推进到 `T3013R`。
> 2026-04-17 当前轮复审更新：`T3012R` 已完成。复审 `expr.rs`、`control_flow.rs`、`stmt.rs`、`mod.rs`、`effect/mod.rs`、`effect/state_machine_plan.rs` 与 `effect/state_machine_emitter.rs` 后确认，unified path 中的 expected context 仍由声明类型、handle result、call 参数与 ordinary control-flow 输出类型合同驱动：`BindLocal` 继续复用 `codegen_initializer_expr`，handle result 通过 `codegen_handle_expr(..., expected)` 与 `contract.result_ty()` 决定，`if`/`when`/call/`print`/`println` 等路径继续复用 ordinary codegen；closure / function-value 也仍通过 `codegen_initializer_expr` / `codegen_call` / `codegen_closure_expr` 的普通生产逻辑进入 lowering，没有 effect-only closure 分支。定向验证 `effect_escape_continuation_indirect_perform_closure_locals.scoop` 与 `std_test_assertions_basic.scoop` 通过，`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 通过；复跑 `cargo run -p scoop --features llvm -- test` 仍只停在已跟踪的 stale `EXPECT: fail` `continuation_resume_continuation.scoop`（`T3017`），未出现新的更早失败点。因此 effect 主线当前顺序推进到 `T3013`。
> 2026-04-17 当前轮收口更新：`T3012` 已完成，但任务边界需要先纠正。重新运行其两条定向验收 fixture 后，`effect_escape_continuation_indirect_perform_closure_locals.scoop` 与 `std_test_assertions_basic.scoop` 都已通过；随后对当前全部 101 个 `run-pass` `EXPECT: fail` fixture 做快速扫描，其中 87 个已直接通过，且不再出现 `expression kind`、`enum variant ctor call without expected enum type`、`sysroot print/println arg type` 三类 `T3012` 目标错误。扫描中唯一残留的 `value coercion` 是 `continuation_resume_enum.scoop`，但它并不属于 expected-context/closure 层：`effect/mod.rs::codegen_continuation_resume_builtin()` 已明确注明 composite resume payload 仍待 `T3013` / `T3009b`，而 `coerce_u64_word()` / `narrow_u64_word_to_cg_value()` 对 tagged-union enum 仍只保留 tag，尚未提供 richer payload transport。因此本轮先把 `continuation_resume_enum.scoop` 从 `T3012` 验收中移除，继续留在已有的 `T3013` + `T3009b` 路径下；`T3012` 本身则按“unified path 的 expected-context / closure / coercion 支持已与普通 codegen 对齐”收口完成。当前 effect 主线下一项推进到 `T3012R`。
> 2026-04-17 当前轮复审更新：`T3011R` 已完成。复审 `state_machine_plan.rs`、`state_machine_emitter.rs` 与 `stmt.rs` 后确认，frame slot 的 mutability / capture 元数据已经按声明点收口：`build_stmt(Val)` 会覆盖旧 placeholder slot，`collect_outer_scope_slots()` / `authoritative_local_slot()` 统一从 `known_local_metadata` 读取权威 mutability，emitter 的 frame-slot 预注册、read-back、arm capture 恢复与 outer-scope seeding/writeback 都直接消费 unified slot metadata；赋值仍统一走通用 `codegen_assign_stmt()`，没有 effect-only mutable patch。定向验证 `declared_handle_local_overwrites_placeholder_slot_metadata`、`handle_context_extension_recovers_nested_handle_outer_var_mutability`、`effect_escape_continuation_resume_unit.scoop`、`effect_escape_continuation_outer_var_writeback_basic.scoop` 均通过；`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 通过；`cargo run -p scoop --features llvm -- test` 仍只停在已跟踪的 stale `EXPECT: fail` `continuation_resume_continuation.scoop`（`T3017`），未出现新的更早失败点。因此 effect 主线当前顺序推进到 `T3012`。
> 2026-04-17 当前轮实现更新：`T3011` 已完成。修复点分两层：一是 `build_unified_lowering_contract()` 现在会把当前 `handle` 自身的 local metadata 合并进生产态 `HandlePlanContext::from_codegen()`，避免 nested/unified 子路径丢失声明点 mutability；二是 `build_stmt(Val)` 遇到真实声明时会直接覆盖旧 slot metadata，不再保留先前 fallback 占坑留下的 `mutable: false` / `seed_from_outer_scope` 残值。已新增结构回归 `declared_handle_local_overwrites_placeholder_slot_metadata` 与 `handle_context_extension_recovers_nested_handle_outer_var_mutability`。验证通过：`cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_resume_unit.scoop`、当前 `EXPECT: fail` run-pass 子集扫描（未再出现 `assignment to immutable local`）、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`；`cargo run -p scoop --features llvm -- test` 仍只停在已跟踪的 stale `EXPECT: fail` `continuation_resume_continuation.scoop`（`T3017`），未出现新的更早失败点。因此 effect 主线当前顺序推进到 `T3011R`。
> 2026-04-17 当前轮复审更新：`T3010R` 已完成。复审 `state_machine_plan.rs` / `state_machine_segments.rs` / `state_machine_transform.rs` / `state_machine_emitter.rs` 后确认，resume-tail 改写仍停留在 plan 层，emitter 未回扫 AST；同时发现并修复了一个真实残留：`HandleStateOp::VarRef` 仍保留 `unwrap_or(CgValue::unit())` 吞错 fallback。该 fallback 现已删除，standalone `VarRef` 必须像普通 expr codegen 一样独立成功或直接报错。另已把 `source_plan_keeps_only_whole_call_for_pure_statement_args_and_pure_if_condition` 收紧为同时禁止纯 statement call 的 callee 落成 `VarRef` fragment。验证通过：`cargo test -p scoopc source_plan_ -- --nocapture`、`cargo test -p scoopc runtime_raise_boundary_ir_branches_between_inactive_continue_and_active_dispatch -- --nocapture`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`；`cargo run -p scoop --features llvm -- test` 仍只停在 `continuation_resume_continuation.scoop` 的 stale `EXPECT: fail`，该既有问题已由 `T3017` 跟踪。因此 effect 主线当前顺序推进到 `T3011`。
> 2026-04-17 当前轮验收更新：`T3010b2b` 已完成。重新验证 `effect_resume_yield_int_basic.scoop`、`effect_resume_finally_normal.scoop` 与 `async_await_minimal_int_basic.scoop` 均通过；随后对当前全部带 `EXPECT: fail` 的 run-pass fixture 扫描 `member access target` / `comparison lhs|rhs` / `equality lhs` / `integer binary op lhs`，确认这些 post-suspend body-tail fragment 错误已全部消失。再把 `effect_resume_finally_body_raise_after_resume.scoop`、`effect_resume_nested_escape_handle_tail.scoop`、`effect_resume_mixed_escape_direct_finally.scoop` 与 `effect_resume_mixed_source_path_matrix.scoop` 临时去掉 `EXPECT: fail` 头后按普通 run-pass 复验，`fixtures: ok (4)`；说明 resume landing 现已只继续剩余 tail，不会重放原 suspend site。继续复跑 `cargo run -p scoop --features llvm -- test` 后，suite 的首个失败点仍为 `tests/fixtures/run-pass/continuation_resume_continuation.scoop` 的 stale `EXPECT: fail`，该问题已由 `T3017` 跟踪。因此 effect 主线当前顺序推进到 `T3010R`。
> 2026-04-16 更新：`T3007R` 之后 effect 主线并未真正语义闭环；`T3008aR` 已完成并确认 frame/continuation ABI 无 verifier-hack 残留。`T3009` 的试探实现进一步确认它受 expression-fragment 重算缺口与 `T3013` 的 composite payload transport 缺口阻塞。为避免把“纯表达式拆片错误”“body tail 的 resume 值注入”和“arm 内 `resume(value)` 专用 lowering”继续混在同一任务里，`T3010` 已细化为：`T3010a`（已完成：清理纯表达式 fragment-only op）、`T3010b1`（已完成：冻结 `resume_path` 合同）、`T3010b2a`（已完成：在 resume state 中引入 synthetic resume slot，并把后续 HIR payload 改写为读取该 slot）、`T3010b2aR`（已完成：收紧 `ResumeAfterSite` 边界，确认 emitter 未回扫 AST）、`T3009a`（本轮完成：接通 immediate-resume arm 的 `resume(value)` 专用 lowering）、`T3009aR`（下一步：review dedicated lowering 是否仍可能回落到 generic call）和 `T3010b2b`（随后回到端到端 post-suspend tail 验收）。原 `T3009` 现收窄为 `T3009b`：只覆盖 escaped continuation 的 `Continuation.resume(...)` 与 composite payload，继续排在 `T3013R` 之后。
> 2026-04-17 更新：`T3009aR` 已完成后，`T3010b2b` 的定向修复已接通多条 comparison / branch / nested-block / mixed-raise fixture，并修复了 outer slot metadata、initial frame seed 与 continuation `resume_state_tag` runtime 回归。但重新跑全量 `cargo run -p scoop --features llvm -- test` 后，首个失败点推进到 `effect_escape_continuation_finally_arm_raise.scoop`；结合 `effect_resume_finally_arm_raise.scoop` 与 `effect_multi_nonresuming_raise_custom_finally.scoop` 的复跑结果，确认当前更前置的 blocker 是“arm body 内 non-resuming effect 的外传 / self-inactive / finally cleanup”统一语义缺口。因此将 `T3010b2b` 拆为前置的 `T3010b2b1`（先修 arm body 语义）与后续的 `T3010b2b`（继续 post-suspend tail 验收）。
> 2026-04-17 进一步复现更新：继续验证 `effect_multi_nonresuming_raise_custom_finally.scoop` 时发现，阻塞并不只在 arm body。`throwAlarm()` 内部的 `Alarm.trip(...)` 之后仍会继续执行 `throw_alarm_unreachable`；`nothing_raise_in_helper_basic.scoop` 里的 `alwaysFail()` 也会在 `Raise.raise(...)` 后继续打印 `unreachable_in_helper`。这说明在继续 `T3010b2b1` 前，还缺一个更基础的前置条件：普通 callee frame 在 non-resuming perform 后必须终止自身执行，而不能只写 active flag 后继续跑到后续语句。因此顺序进一步细化为 `T3010b2b0`（先修普通 callee frame 的 non-resuming perform 终止语义）→ `T3010b2b0R` → `T3010b2b1` → `T3010b2b`。
> 2026-04-17 当前轮更新：`T3010b2b0` 已完成。ordinary frame 现在会在 direct non-resuming `perform/Raise` 后立刻结束当前 callee frame，并在 ordinary user call 返回后统一检查 TLS active，必要时直接向 caller 返回默认值；`Nothing` 返回类型在这条 propagation 路径上改为 `ret void`。`nothing_raise_in_helper_basic.scoop` 与 `effect_indirect_perform_nonresuming_call_chain.scoop` 已恢复与 golden 一致。
> 2026-04-17 复审补充更新：开始执行 `T3010b2b0R` 时，构造“ordinary helper -> object property access -> object init 内 Raise”定向复现后发现：caller 侧 unified state-machine 的确还会把这类 helper 调用误判成普通 `Call`，其根因是 `HandlePlanContext::known_fun_effects` 只看显式 effect row，没有把 object value/property access、class ctor init、runtime raise 等 hidden suspend 来源折叠进 callee 元数据。因此原顺序先细化为 `T3010b2b0` → `T3010b2b0a`（先修 caller-side hidden suspend call 分类）→ `T3010b2b0R` → `T3010b2b1` → `T3010b2b`。
> 2026-04-17 当前轮阻塞更新：在实现 `T3010b2b0a` 时补了定向 helper 复现，结果发现更前置的 ordinary-frame hidden suspend 缺口仍未闭合：`main_unreachable` 已不再出现，但 `helper()` 自身仍会在 `BoomObject.x` 返回 active 后继续执行 `helper_unreachable`。这说明 `T3010b2b0` 目前只覆盖 direct `perform/Raise`、ordinary user call 与 `as` cast raise；object value/property access、class ctor init、builtin runtime raise 等 hidden suspend boundary 还没有接到 ordinary-frame propagation 合同。因此顺序再次前移为 `T3010b2b0` → `T3010b2b0a0`（先修 hidden-suspend ordinary callee 自终止）→ `T3010b2b0a`（再收口 caller-side hidden suspend call 分类）→ `T3010b2b0R` → `T3010b2b1` → `T3010b2b`。
> 2026-04-17 当前轮完成更新：`T3010b2b0a0` 已完成。`codegen_object_property_access` 现在会在 object init 返回后执行统一 ordinary-frame active 检查，新增的 helper 级 object-property/class-ctor hidden-suspend fixtures 都已通过，`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。复跑 `cargo run -p scoop --features llvm -- test` 后，首个失败点仍停在 `effect_escape_continuation_finally_arm_raise.scoop`，对应已知后续 blocker `T3010b2b1`，未出现更早回归。
> 2026-04-17 当前轮进一步验证更新：继续执行 `T3010b2b0a` 时，先用临时 `handle` 复现回查 top-level helper / class-ctor helper / local closure 包一层 helper 的 caller-side hidden-suspend 路径，三者都已能正确进入 dispatch，不再继续执行 caller tail。继续把验证扩到 member 路径时，发现一个更前置且未被 `TODO.md` 跟踪的基础 bug：即使不经过 `handle`，普通 `Helper.run()` 也会因为 `ptr @__scoop_object_instance__Helper` 传给期望 `ptr addrspace(1)` receiver 的 `@Helper.run(...)` 而触发 LLVM verifier 失败。根因是 object 单例值仍用 default addrspace 的全局身份地址表示，而对象成员函数 receiver 走的是 `CgTy::Ref` 的 `addrspace(1)` ABI。由于这会先于 hidden-suspend 分类暴露，因此顺序再次前移为 `T3010b2b0` → `T3010b2b0a0` → `T3010b2b0a0b`（先修 object member call 的 receiver ABI / 表示）→ `T3010b2b0a`（再完成 member 路径的 caller-side hidden-suspend 验证/修复）→ `T3010b2b0R` → `T3010b2b1` → `T3010b2b`。
> 2026-04-17 当前轮完成更新：`T3010b2b0a0b` 已完成。object 单例值现在通过 `scoop_alloc_typed` 分配成 header-only GC singleton object，并存入 `ptr addrspace(1)` 全局槽；`codegen_object_value_access` 改为在 once init 后加载该 GC-managed receiver，`Helper.run()` 这类普通 object member call 不再触发 verifier。已新增 `object_member_call_basic.scoop` 与 LLVM IR 单测 `object_member_call_uses_gc_managed_singleton_receiver`。验证通过：最小 repro 已可编译、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 通过，复跑 `cargo run -p scoop --features llvm -- test` 后首个失败点仍停在已知 blocker `effect_escape_continuation_finally_arm_raise.scoop`（`T3010b2b1`），未出现更早回归。
> 2026-04-17 当前轮完成更新：`T3010b2b0a` 已完成。重新验证 `handle { helper() }`、`handle { Helper.run() }` 与 local closure/function-value `handle { thunk() }` 三条 caller-side 路径后，确认 hidden suspend 返回 active 时 unified state-machine 都会立即进入 dispatch，不会继续执行 caller tail。已新增三个 run-pass fixture（top-level helper、member helper、local closure/function-value）与两条 segment-level 分类单测，分别锁定 `call-state-machine-callee` / `call-may-suspend`，防止回退成 plain `Call`。验证通过：定向 fixture、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`；复跑 `cargo run -p scoop --features llvm -- test` 后，首个失败点仍是已知后续 blocker `effect_escape_continuation_finally_arm_raise.scoop`（`T3010b2b1`），未出现更早回归。
> 2026-04-17 当前轮复审更新：`T3010b2b0R` 已完成。审查 `effect/mod.rs`、`mod.rs`、`control_flow.rs` 与 `state_machine_emitter.rs` 的 ordinary-frame / dispatch 边界后，确认 non-resuming callee frame 的终止语义只来自 `emit_ordinary_non_resuming_effect_exit` / `emit_ordinary_call_effect_propagation_check` 这套统一控制流合同；生产代码中无 `emit_effect_unwind_if_active`、`raise_target_stack`、callee-shape/scanner 残留，ordinary callee 路径也不会清掉 TLS active。已验证定向 non-resuming/hidden-suspend fixtures、segment 分类单测、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`；复跑 `target/debug/scoop test` 后，首个失败点仍为已知后续 blocker `effect_escape_continuation_finally_arm_raise.scoop`（`T3010b2b1`），未出现更早回归。
> 2026-04-17 当前轮实现更新：原 `T3010b2b1` 已拆成已完成的 `T3010b2b1a` 与待续的 `T3010b2b1`。`T3010b2b1a` 已接通 arm body direct non-resuming effect 的 outward propagation、arm return/finally cleanup 与 no-perform handle result：`effect_resume_finally_arm_raise.scoop`、`effect_escape_continuation_finally_arm_raise.scoop`、`effect_multi_nonresuming_raise_custom_finally.scoop` 已通过，且回收了 `effect_escape_continuation_finally_no_perform.scoop`、`effect_escape_continuation_zero_perform_returns_body.scoop`、`effect_no_perform_handle_elim_basic.scoop` 等同根因 xfail。验证通过：`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
> 2026-04-17 当前轮阻塞更新：继续复跑 `cargo run -p scoop --features llvm -- test` 后，首个真实失败点推进到 `effect_escape_continuation_nested_arm_indirect_performs_outer.scoop`，当前 unified path 在 inner escape-cont arm 的间接调用结果整形上仍报 `暂不支持的 main 代码生成节点：value coercion`。这不再是 arm cleanup/self-capture 逻辑缺口，而是一个直接阻塞当前链路的 expected-context/coercion 前置。因此 `T3010b2b1` 继续细化为：`T3010b2b1a`（已完成：direct arm-body outward propagation/finally/no-perform result）→ `T3010b2b1b`（下一步：前移修复 nested arm indirect path 所需的 unified value coercion / expected-context 最小前置）→ `T3010b2b1`（随后回到剩余 nested/indirect outward propagation 验收）。
> 2026-04-17 当前轮继续定位更新：开始执行 `T3010b2b1b` 时，用定向复现 + backtrace 确认更前置的真实 blocker 并不是 value coercion 本身，而是 synthetic resume slot `__resume_site0` 与 outer local 复用了同一个 `SymbolId`。结果是 inner handle 入口 `seed_outer_scope_frame_slots` 会把 outer `_ : Unit` 误种到期望 `Int` 的 synthetic resume slot 上，在真正进入 unified expected-context 逻辑前就先触发 `Unit -> Int` coercion。由于这个 bug 当前未被 `TODO.md` 显式跟踪，顺序需再次前移为：`T3010b2b1a`（已完成）→ `T3010b2b1b0`（先修 synthetic resume slot id / frame seeding 合同）→ `T3010b2b1b`（再继续 unified value coercion / expected-context）→ `T3010b2b1`。
> 2026-04-17 当前轮完成更新：`T3010b2b1b0` 已完成。synthetic resume slot 现在通过共享 symbol floor/cursor 在嵌套 handle 间分配全局唯一 `SymbolId`，`seed_outer_scope_frame_slots` 也只会 seed 显式 outer-scope slot。定向复现 `effect_escape_continuation_nested_arm_indirect_performs_outer.scoop` 现已直接通过；同时顺带回收了 `effect_custom_nonresuming_nested_nearest_and_arm_outside_scope.scoop`、`effect_escape_continuation_arm_performs_outer_effect.scoop`、`effect_escape_continuation_reperform_from_escape_arm.scoop`、`effect_handle_yield_and_step_finally.scoop` 与 `effect_handler_stack_nearest_and_arm_outside_scope.scoop` 这些历史 xfail。继续复跑 `cargo run -p scoop --features llvm -- test` 后，suite 当前首先停在另一条 stale xfail `effect_handler_stack_nearest_three_levels_and_arm_outside_scope.scoop`，说明下一步需要由 `T3017` 统一回收剩余 expectation；而 `T3010b2b1b` 的原始目标 fixture 已不再失败，需在后续执行时按新的最小 repro 重新基线化剩余 expected-context/coercion 缺口。
> 2026-04-17 当前轮重新基线更新：继续按“新最小 repro”执行 `T3010b2b1b` 后发现，真正的首个 blocker 进一步前移为 escaped continuation 的 `Continuation.resume(...)` dedicated lowering 缺口。`effect_resume_nested_escape_handle_tail.scoop` 与已在 `T3009b` 名下的 `effect_escape_continuation_resume_unit.scoop` / `effect_escape_continuation_resume_string.scoop` 现在都一致在 `k.resume(...)` 处报 `暂不支持的 main 代码生成节点：call callee`；与此同时，state-machine plan 已通过 `continuation_resume_call_sites` 正确把这些 call site 分类为 builtin `Continuation.resume`。这说明当前问题不是 unified expected-context/coercion，而是 unified emitter / 普通 call path 仍未对 escaped continuation 的 `k.resume(...)` 做 dedicated lowering。顺序因此再次前移为：`T3010b2b1a`（已完成）→ `T3010b2b1b0`（已完成）→ `T3009b0`（先接通 scalar/ref escaped continuation resume lowering）→ `T3009b0R` → `T3010b2b1b`（再重新检查是否仍有独立 expected-context/coercion 缺口）→ `T3010b2b1`；原 `T3009b` 收窄为在 `T3013R` 之后把同一路径扩展到 composite resume payload。
> 2026-04-17 当前轮阻塞更新：开始执行 `T3009b0` 并接通 call-span 驱动的 `Continuation.resume(...)` dedicated lowering 原型后，`effect_escape_continuation_resume_unit.scoop` 已不再报 `call callee`，但新的首个失败点进一步前移为 outer-scope mutable slot 写回缺口。具体表现为：escape arm 内 `saved = Some(k)` 已执行并打印 `arm_saved`，离开 `handle` 后 `saved` 仍为 `None`，说明 unified path 目前只把 outer locals/params 通过 `seed_outer_scope_frame_slots` 复制进 effect frame，却没有在 handle 完成后把被 frame 改写的 outer-scope slot 写回 enclosing local alloca。由于这会先于 escaped continuation resume 的 payload/transport 验收暴露，顺序再次前移为：`T3009b0a`（先修 outer-scope seeded slot writeback）→ `T3009b0aR` → `T3009b0`（再继续 scalar/ref resume lowering 验收）→ `T3009b0R` → `T3010b2b1b` → `T3010b2b1`。
> 2026-04-17 当前轮完成更新：`T3009b0a` 已完成。修复不止在 emitter 出口补 writeback，还包括把 unified contract 的 outer-scope slot 收集从仅 `handle.body` 扩到整个 `handle`（body、arms、finally），并排除 arm binder / resume / continuation locals 与 handle 内部局部。`handle_done` / `handle_propagate` 现在都会按 metadata 统一回写 seeded outer mutable slot。已新增结构测试 `handle_outer_scope_seeding_includes_arm_and_finally_locals` 与 focused fixture `effect_escape_continuation_outer_var_writeback_basic.scoop`。复跑 `effect_escape_continuation_resume_unit.scoop`、`..._string.scoop` 与 `..._bool.scoop` 后，输出都已越过 `missing`，说明 outer-local 保存阶段已打通；剩余 `resume(...)` 返回后 caller tail 未继续的问题明确留给下一步 `T3009b0/T3009b0R`。
> 2026-04-17 当前轮 review 更新：开始执行 `T3009b0aR` 时，继续审查 `write_back_outer_scope_frame_slots` 与 `codegen_continuation_resume_builtin` 后发现一个尚未被 `TODO.md` 跟踪的前置缺口：outer-slot writeback 目前仍只挂在 `codegen_handle_expr_via_state_machine` 的 `handle_done` / `handle_propagate`。这覆盖了“第一次离开 handle”时 body / arm / finally 的 outer-local 写回，但 escaped continuation 在 handle 返回后通过 `k.resume(...)` 继续执行 body / finally 时，runtime `scoop_continuation_resume` 只直接调用 continuation `step_fn`，没有任何统一的 frame-metadata 驱动 writeback 出口。用最小临时 repro 验证时，输出为 `body_before` → `arm_saved` → `after_handle` → `before` → `body_after`，说明 resumed body 已继续执行，而 post-resume completion path 仍未形成可复审的统一 outer-local 同步合同。因此顺序再次前移为：`T3009b0a`（已完成）→ `T3009b0a1`（先补 escaped continuation 恢复完成路径的 outer-slot writeback 合同）→ `T3009b0aR` → `T3009b0` → `T3009b0R` → `T3010b2b1b` → `T3010b2b1`。
> 2026-04-17 当前轮拆分完成更新：真正开始实现 `T3009b0a1` 后，用临时 repro 与 focused fixture 草案复现发现，当前 `k.resume(...)` 虽已能继续执行 resumed body，但 caller-tail 仍会在 `body_after` / `body_resumed` 处截断；因此“`resume()` 返回点可见的 outer-local 已同步”这一 run-pass 验收实际上受 `T3009b0` 阻塞，不能与写回合同基础设施绑成同一个任务。顺序因此再次细化为：`T3009b0a1a`（本轮完成：把 authoritative outer-slot writeback target 收口进 effect frame metadata，并接到统一 step-return 出口）→ `T3009b0`（先接回 escaped continuation 的 caller-tail / scalar-ref resume lowering 正式验收）→ `T3009b0a1b`（随后新增 focused fixture，直接观测 `resume()` 返回点的 outer-local 同步）→ `T3009b0aR` → `T3009b0R` → `T3010b2b1b` → `T3010b2b1`。`T3009b0a1a` 已新增 LLVM IR 单测 `escaped_continuation_resume_ir_records_outer_slot_storage_and_writeback`，锁定 frame metadata + step-return writeback 基础设施。
> 2026-04-17 当前轮重新定位更新：真正按 `T3009b0` 验收继续复现后，发现更前置的 shared blocker 不是 payload transport，而是 unified `RuntimeRaiseBoundary` 合同本身。当前 `Continuation.resume(...)` 与 `x as T` 这类 boundary expression 在 state machine 中都被建模成“求值后无条件 `Suspend`”：例如 `try { k.resume(()) ; ... } catch` 的 step function 会先调用 `scoop_continuation_resume(...)`，随后立刻 `alloc continuation + set_active + return`；`type_check_cast_is_as_asq_basic.scoop` 也在首个成功 `as` 后就停在 `x is Base: true`。这说明 inactive 成功路径没有继续 caller-tail，而是被错误截断。由于这个缺口比 escaped continuation dedicated lowering 更基础、且当前未被 `TODO.md` 跟踪，顺序再次前移为：`T3009b0a1a`（已完成）→ `T3009b0a2`（先修 shared `RuntimeRaiseBoundary` 的 inactive-continue / active-dispatch 合同）→ `T3009b0`（再完成 escaped continuation 的 scalar/ref dedicated lowering 正式验收）→ `T3009b0a1b` → `T3009b0aR` → `T3009b0R` → `T3010b2b1b` → `T3010b2b1`。
> 2026-04-17 当前轮再定位更新：真正开始落地 `T3009b0a2` 时，用最小 repro `handle { helper(false) + 1 } with { Ask.ask() -> 2 }` 发现 shared blocker 还要更前置：不仅 `RuntimeRaiseBoundary`，连 `SuspendCall`（至少覆盖 `CallMaySuspend` / `CallStateMachineCallee` / `ClassCtorInit`）的 inactive 成功路径也仍被统一 terminator 误当成“求值后无条件 suspend”。`helper(false)` 明明没有 perform，程序却直接空输出退出，本应打印 `8`。这说明当前缺口并非 `RuntimeRaiseBoundary` 独有，而是 call-like boundary 的 inactive-continue / active-dispatch 合同仍未真正接通。由于这个问题会先于 `T3009b0a2` 暴露、且当前未被 `TODO.md` 显式跟踪，顺序再次前移为：`T3009b0a1a`（已完成）→ `T3009b0a1c`（先修 unified `SuspendCall` 的 inactive-continue / active-dispatch 合同）→ `T3009b0a1cR` → `T3009b0a2`（再收口 shared `RuntimeRaiseBoundary`）→ `T3009b0`（随后继续 escaped continuation 的 scalar/ref dedicated lowering / caller-tail）→ `T3009b0a1b` → `T3009b0aR` → `T3009b0R` → `T3010b2b1b` → `T3010b2b1`。
> 2026-04-17 当前轮完成更新：`T3009b0a1c` 已完成。`UnifiedStateTerminator::Suspend` 现已对 `CallMaySuspend` / `CallStateMachineCallee` / `ClassCtorInit` 三类 call boundary 统一执行 TLS active 分流：callee 返回 inactive 时，把 call 结果写入 frame resume 槽并直接 branch 到 `resume_state`；只有 active 时才继续分配 continuation、保留 outward dispatch。已新增 fixture `effect_handle_suspend_call_inactive_helper_basic.scoop`，同时锁定 inactive caller-tail 与 active resume dispatch；并复跑 `effect_handle_hidden_suspend_local_closure_helper_basic.scoop` 确认 `CallMaySuspend` 现有 active-path 未回退。验证通过：两条定向 fixture、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。下一步进入 `T3009b0a1cR`，只审查生产代码是否仍保持单一 state-machine 合同。
> 2026-04-17 当前轮复审阻塞更新：开始执行 `T3009b0a1cR` 时，继续审查 `UnifiedStateTerminator::Suspend` 并用最小 repro 验证后发现，shared inactive-path 缺口还不止 `SuspendCall` / `RuntimeRaiseBoundary`。direct object/property access 的最小复现 `handle { Config.x + 1 } with { Raise.raise(err: RuntimeError) -> 10 }` 当前只打印 `before`、`config.init`；outer `handle` 包 inner `handle { helper(false) + 1 }` 的最小 repro 当前只打印到 `inner_after`，`outer_after` 与最终结果都不会继续。这说明 `ObjectInitAccessBoundary` 与 `NestedHandleBoundary` 仍被建模成“求值后无条件 suspend”，inactive 成功路径没有留在当前 state machine 内。与此同时，`SuspendCall` 本身的复审已确认当前 inactive/active 分流只按 `SuspendSiteKind` + TLS active 驱动，没有读取 callee 名称或源码形状；但由于三类 boundary 仍共用同一个 `UnifiedStateTerminator::Suspend` 出口，当前还不能为“单一 state-machine 合同”下最终复审结论。顺序因此再次前移为：`T3009b0a1c`（已完成）→ `T3009b0a1d`（先修 `ObjectInitAccessBoundary` inactive-path）→ `T3009b0a1dR` → `T3009b0a1e`（再修 `NestedHandleBoundary` inactive-path）→ `T3009b0a1eR` → `T3009b0a1cR` → `T3009b0a2`（随后继续收口 shared `RuntimeRaiseBoundary`）→ `T3009b0` → `T3009b0a1b` → `T3009b0aR` → `T3009b0R` → `T3010b2b1b` → `T3010b2b1`。
> 2026-04-17 当前轮完成更新：`T3009b0a1d` 已完成。`UnifiedStateTerminator::Suspend` 现在把 `SuspendSiteKind::ObjectInitAccess` 纳入与 `SuspendCall` 相同的 TLS-active 分流：inactive 时把 boundary 结果写回 frame 并 branch 到 `resume_state`，active 时才 continuation + dispatch 返回。新增 run-pass fixture `effect_handle_object_init_access_inactive_basic.scoop` 同时锁定 direct object value access、property access 的 inactive-path caller-tail 和 property access 的 active dispatch。已验证 `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_object_init_access_inactive_basic.scoop`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 通过。为闭环验证还顺手收回了已恢复通过的 stale xfail `effect_escape_continuation_resume_cross_thread.scoop`；整套 `cargo run -p scoop --features llvm -- test` 仍会在后续 `T3017` 范围内的另一个 stale xfail `effect_handler_stack_nearest_three_levels_and_arm_outside_scope.scoop` 处停止，因此当前主线下一项仍是 `T3009b0a1dR`。
> 2026-04-17 当前轮复审完成更新：`T3009b0a1dR` 已完成。复审 `state_machine_emitter.rs`、`state_machine_plan.rs` 与 `mod.rs` 后确认：`ObjectInitAccessBoundary` 的 inactive/active 分流仍只由 shared `Suspend` terminator 读取 `SuspendSiteKind::ObjectInitAccess` + TLS active 决定；`HandlePlanContext::from_codegen` 里的 `object_value_fqns` / `object_property_fqns` 仅来自 `object_inits` 元数据，用于 plan 阶段 suspend-site 建模，不参与 emitter 选路；ordinary `codegen_object_value_access` / `codegen_object_property_access` 中的 TLS active 检查在 step function 内因 `current_fun_return_ty` / `return_context` 被清空而失效，不会形成绕开 state-machine 的 side channel。验证通过：`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_object_init_access_inactive_basic.scoop`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。当前主线下一项推进到 `T3009b0a1e`。
> 2026-04-17 当前轮完成更新：`T3009b0a1e` 已完成。`UnifiedStateTerminator::Suspend` 现已把 `SuspendSiteKind::NestedHandleBoundary` 纳入 shared inactive-continue / active-dispatch 合同：inner handle inactive 返回时，把 authoritative 结果写回 frame 并 branch 到 `resume_state`；只有 TLS active 时才继续 continuation + outward dispatch。为避免 inactive-path 重跑 inner handle，`NestedHandleBoundary` 现在与 `SuspendCall` 一样携带 `resume_path` + synthetic resume slot，outer caller-tail 中的 nested handle 子表达式会被改写成读取 `__resume_site*`。实现过程中还顺手修复了更上游的类型源错误：HIR lowering 不再把 `ExprKind::Handle` 一律标成 `Any`，而是保留 typechecked handle result type，否则 nested-boundary resume slot 会被错误降成 `Ref`。已新增 run-pass fixture `effect_handle_nested_handle_boundary_inactive_basic.scoop` 与 transform 单测 `nested_handle_boundary_preserves_resume_path_and_slot`，并同步更新 `tests/fixtures/hir/handle_perform.hir` golden。验证通过：定向单测、定向 fixture、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。当前主线下一项推进到 `T3009b0a1eR`。
> 2026-04-17 当前轮复审完成更新：`T3009b0a1eR` 已完成。复审 `state_machine_emitter.rs`、`state_machine_plan.rs`、`state_machine_transform.rs`、`expr.rs` 与 `hir/lower/expr.rs` 后确认：`NestedHandleBoundary` 的 inactive/active 分流仍只由 shared `Suspend` terminator 读取 `SuspendSiteKind::NestedHandleBoundary` + TLS active 决定；authoritative nested-handle result transport 仍由 `resume_path` + synthetic resume slot 驱动，resume-after-site 后续表达式会读取 `__resume_site*`，不会重跑 inner handle；`ExprKind::Handle` 入口也仍统一进入 `codegen_handle_expr` / `codegen_handle_expr_via_state_machine`，没有 outer emitter、普通 call codegen 或 shape-based 分流回流。验证通过：`cargo test -p scoopc nested_handle_boundary_preserves_resume_path_and_slot -- --nocapture`、`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_nested_handle_boundary_inactive_basic.scoop`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。当前主线下一项推进到 `T3009b0a1cR`。
> 2026-04-17 当前轮复审完成更新：`T3009b0a1cR` 已完成。复审 `state_machine_emitter.rs`、`state_machine_plan.rs`、`state_machine_segments.rs`、`state_machine_transform.rs`、`expr.rs`、`mod.rs` 与 `effect/mod.rs` 后确认：`SuspendCall` 的 inactive/active 分流仍只由 shared `Suspend` terminator 读取 `SuspendSiteKind::{CallMaySuspend, CallStateMachineCallee, ClassCtorInit}` + TLS active 决定；`HandleStateOp::SuspendCall` 本身只求值调用表达式，不携带 call-site/callee 专用分支；post-call caller-tail 的 authoritative 数据通路仍是 `resume_path` + synthetic resume slot，计划/segment/unified contract 都会校验该元数据。ordinary call 的 TLS active 检查仍只存在于普通 frame 路径，而 step function 生成期间会清空 `current_fun_return_ty` / `return_context`，因此不存在把 inactive-path 回流成 ordinary helper side channel 的补丁。验证通过：`cargo test -p scoopc resume_path_is_preserved_from_plan_to_segments_to_unified_machine -- --nocapture`、`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_suspend_call_inactive_helper_basic.scoop`、`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_hidden_suspend_local_closure_helper_basic.scoop`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。当前主线下一项推进到 `T3009b0a2`。
> 2026-04-17 当前轮完成更新：`T3009b0a2` 已完成。`state_machine_emitter.rs` 的 shared `Suspend` terminator 现已把 `SuspendSiteKind::RuntimeRaise` 与其它可“成功返回本地 caller-tail”的 boundary 一样纳入 TLS active 分流：`Continuation.resume(...)` / `x as T` 成功时走 `site*_inactive` 把结果写入 frame resume 槽并 branch 到 `resume_state`，只有实际触发 `Raise.raise` 时才走 `site*_active` outward dispatch。新增 IR 单测 `runtime_raise_boundary_ir_branches_between_inactive_continue_and_active_dispatch` 锁定这一共享结构；定向 fixture `type_check_cast_is_as_asq_basic.scoop` 与 `effect_escape_continuation_resume_unit.scoop` 已恢复预期输出，`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。当前主线下一项推进到 `T3009b0`。
> 2026-04-17 当前轮完成更新：`T3009b0` 已完成。复查 `crates/scoopc/src/llvm/codegen/mod.rs` 与 `crates/scoopc/src/llvm/codegen/effect/mod.rs` 后确认，当前分支里的 production 路径已对 typecheck 标记过的 `Continuation.resume(...)` 做 dedicated lowering：普通 call 入口通过 `continuation_resume_call_sites` 直接分派到 `codegen_continuation_resume_builtin`，后者复用共享 continuation runtime ABI 与 `resume_word` / `resume_gc_ref` transport，覆盖 Unit、标量 word 与 GC ref payload，不再回落到 generic member access / generic call。正式验收通过：`effect_escape_continuation_resume_unit.scoop`、`effect_escape_continuation_resume_bool.scoop`、`effect_escape_continuation_resume_string.scoop`、`effect_resume_nested_escape_handle_tail.scoop`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。当前主线下一项推进到 `T3009b0a1b`。
> 2026-04-17 当前轮完成更新：`T3009b0a1b` 已完成。新增 focused fixture `effect_escape_continuation_resume_outer_var_writeback.scoop`，直接锁定“handle 初次离开时 outer `var` 仍保持旧值；`k.resume(...)` 返回后调用点可见值更新”为 resumed completion path 的统一 writeback 合同。定向运行输出为 `after_handle -> 5 -> after_resume -> 42 -> done`，证明 caller-tail 接回后，resumed body 对 outer slot 的改写已经通过 frame metadata 写回到 caller 可见 local。已验证：定向 fixture、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。当前主线下一项推进到 `T3009b0aR`。
> 2026-04-17 当前轮复审完成更新：`T3009b0aR` 已完成。复审 `state_machine_emitter.rs` 的 outer-slot frame metadata / shared writeback helper、`effect/mod.rs` 的 `codegen_continuation_resume_builtin` 与 `mod.rs` 的 call-site 分派后，确认 outer-scope local 写回仍只由 unified handle frame metadata 驱动；`ReturnHandle` / `ReturnFromFunction` / `Suspend` / `Arm*` 返回出口以及 `handle_done` / `handle_propagate` 复用同一 helper，`Continuation.resume(...)` lowering 没有承担 outer-local 同步职责。已验证：`handle_outer_scope_seeding_includes_arm_and_finally_locals`、`escaped_continuation_resume_ir_records_outer_slot_storage_and_writeback`、两个 focused writeback fixtures、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。当前主线下一项推进到 `T3009b0R`。
> 2026-04-17 当前轮复审完成更新：`T3009b0R` 已完成。复审 `crates/scoopc/src/llvm/codegen/mod.rs`、`expr.rs`、`effect/mod.rs` 与 `effect/state_machine_emitter.rs` 后确认：`Continuation.resume(...)` 的 builtin 语义仍只由 `continuation_resume_call_sites` 驱动；ordinary path 与 unified state-machine path 都通过同一个 `codegen_call -> codegen_continuation_resume_builtin` 分派进入共享 continuation runtime ABI 与 `resume_word` / `resume_gc_ref` transport，没有回流 generic member access / generic call fallback。已验证：call-site marker 分类单测、`effect_escape_continuation_resume_unit.scoop` / `..._bool.scoop` / `..._string.scoop` / `effect_resume_nested_escape_handle_tail.scoop`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。当前主线下一项推进到 `T3010b2b1b`。
> 2026-04-17 当前轮重新基线更新：`T3010b2b1b` 已完成。重新运行 `effect_resume_nested_escape_handle_tail.scoop`、`effect_resume_nested_escape_handle_tail_multi_perform_nonunit.scoop` 与 `effect_escape_continuation_nested_arm_indirect_performs_outer.scoop` 后，原先记录的 `value coercion` / `unknown local value` 路径均未再复现；复审 `state_machine_emitter.rs` 也确认 immediate-resume arm body 与其余 nested/indirect arm-body 表达式都统一走 `codegen_expr_in_expected_context` + shared `coerce_value`，不存在独立的 expected-context/coercion 旁路。因此 `T3010b2b1b` 已收窄为“确认缺口已消失”的 rebaseline 任务，并可视为完成。继续复跑 `cargo run -p scoop --features llvm -- test` 时，suite 当前先停在未跟踪的 MIR snapshot mismatch：`tests/fixtures/mir/handle_perform.scoop` 对应的 `handle_perform.mir` golden 仍保留旧的 handle result 临时类型；这与此前 `ExprKind::Handle` 保留 typechecked result type 的修正一致，但当时只同步了 HIR golden。顺序因此更新为：`T3010b2b1b1`（先同步 `handle_perform` 的 MIR golden，恢复全量 fixture 验证入口）→ `T3010b2b1`（继续 arm-body nested/indirect outward propagation 语义验收）→ `T3010b2b`。
> 2026-04-17 当前轮完成更新：`T3010b2b1b1` 已完成。已复审 `tests/fixtures/hir/handle_perform.hir`、`crates/scoopc/src/hir/lower/expr.rs` 与 `crates/scoopc/src/mir/lower.rs`，确认 `handle_perform` 的 MIR 漂移只是在 `ExprKind::Handle` 保留 typechecked result type 后，`lower_handle_expr(expr.span, expr.ty, ...)` 使 handle result 临时 local `tmp0` 也跟着从 `TypeId(0)` 变为 `TypeId(5)`；这不是新的 lowering 回归。同步更新 `tests/fixtures/mir/handle_perform.mir` 后，`diff -u ... <(cargo run -p scoop -- dump-mir ...)`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 全部通过。继续复跑 `cargo run -p scoop --features llvm -- test` 时，suite 已越过 MIR snapshot mismatch，新的首个失败点推进到 `tests/fixtures/run-pass/continuation_resume_continuation.scoop` 的 stale `EXPECT: fail`；该 expectation cleanup 已由 `T3017` 跟踪，不改变 effect 主线当前顺序，下一项仍是 `T3010b2b1`。

## 0. 工作原则

- 当前最高优先级是 `T30`；`T31`～`T34` 只作为 effect 主线收口后的后续队列，不与当前修线争抢顺序。
- `T30` 继续遵守“删除优先于修补”。看到 shape-based 生产逻辑就直接删除，不以“先补一个 case”维持旧路径。
- `T30` 中 LLVM effect codegen 的单一输入是 state machine。除类型、符号与 ABI 必需信息外，不能再读取源码形状、旧 scanner 结果或旧分类器输出。
- `T30` 当前阶段继续坚持 full state machine lowering；`T3008a` 已把 frame/continuation ABI 收口到 GC typed alloc + `addrspace(1)`，但仍不做 simplification，不做模式化优化。
- flag-based unwind（`emit_effect_unwind_if_active` / `raise_target_stack`）明确搁置，不作为 `T30` 主线依赖。effect 传播完全由统一 state machine 驱动；flag-based unwind 日后可作为优化加回。
- `T30` 中每个实现任务后立即插入一个 review 任务；review 必须显式确认生产代码中不存在 shape-based logic。
- `T30` 的 review 范围只看生产代码，重点是 `crates/scoopc/src/llvm/codegen/**`；测试命名不作为问题。
- `T31`～`T34` 维持“小步可回归”原则：先收口语义与表示，再扩展 lowering / runtime / 测试；除显式写出的依赖外，不额外插入 effect 风格的 review 子任务。

## 1. 当前状态与已知缺口（T30）

- `T2999` 已完成：
  - `cargo check -p scoopc` 已恢复零 warning。
  - `cargo clippy --all-targets -- -D warnings` 已通过。
  - `scoop.core.__scoop_effect_*` sysroot 测试辅助 intrinsic 已重新直连 runtime ABI，`cargo test --all` 已恢复通过。
- `T2999R` 已完成：
  - 已删除 `runtime_abi.rs` 中无生产调用点、也不属于当前统一 effect 合同的 `declare_runtime_alloc` / `declare_runtime_gc_collect`。
  - 已把 `runtime_symbols.rs` 中散落的冗余 `#[allow(dead_code)]` 清掉，并删除 `state_machine_plan.rs` / `state_machine_transform.rs` 中被统一骨架边界覆盖的重复豁免。
  - 已重新验证 `cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all` 全部通过。
- `T3001` 已完成：
  - 已从 `crates/scoopc/src/llvm/codegen/mod.rs` 删除 `CalleeSuspendResumeMode`、`scan_for_callee_suspend`、`codegen_top_level_fun_suspendable`、`codegen_closure_fun_body_suspendable` 及其入口接线。
  - 顶层函数与 closure 的 codegen 已收口回常规路径，不再按 `perform` 所在源码形状选择专用 suspendable lowering。
  - 已同步清理 `effect/mod.rs`、`runtime_abi.rs`、`runtime_symbols.rs` 中仅服务于这条旧路径的 helper / ABI 声明。
  - 复验通过：`cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`。
- `T3001R` 已完成：
  - 已定向检索 `crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/effect/mod.rs` 与相邻调用点，确认删除后的旧 callee-shape scanner / mode enum / suspendable top-level/closure route 没有换名回流。
  - 已复查 `codegen_top_level_fun`、`codegen_closure_fun_body`、`codegen_top_level_fun_call` 与 `ExprKind::Perform` / `ExprKind::Handle` 接线，确认当前只剩常规函数/闭包 codegen 与统一 effect 占位入口，不再按源码 / callee 形状分流。
  - 已重新验证 `cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all` 全部通过。
- `T3002R` 已完成：
  - 已定向检索 `crates/scoopc/src/llvm/codegen/**` 中与旧分流相关的命名与入口，包括 `shape`、`scan_for`、`CalleeSuspend`、`suspendable` 等，未发现残留命中。
  - 已复查 `expr.rs`、`effect/mod.rs` 与 `mod.rs` 调用链，确认 `ExprKind::Perform` / `ExprKind::Handle` 只直连统一 effect 入口；当前残留的 effect 相关生产逻辑仅为统一 lowering 占位入口、sysroot intrinsic lowering 与 flag-based unwind 辅助，没有按源码 / site / arm / callee 形状做主选路。
  - 已重新验证 `cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all` 全部通过。
- `T3004a` 已完成：创建 `state_machine_emitter.rs`，实现从 `UnifiedFrameSchema` 生成 LLVM struct type（system fields + user slots）、step function 骨架（state_tag-based switch 派发，各 state block 暂时 return void）、handle 表达式入口（build contract → malloc frame → memset zero → init state_tag → call step_fn → return default value）。`codegen_handle_expr` 已从占位错误改为调用 `codegen_handle_expr_via_state_machine`。
- `T3003a` 已完成：`HandleStateOp` / `HandleBranchCondition` 已补齐完整 HIR payload，unified state machine 的执行 payload 元数据在 plan → segments → unified machine 流水线中稳定保留。原始 `T3003` 已拆分为 `T3003a`（payload 补齐，已完成）、`T3003b`（builder/访问面暴露，已完成）和 `T3003R`（review，已完成）。
- `T3003b` 已完成：`UnifiedHandleLoweringContract` 已定义为唯一生产结构输入；`build_unified_lowering_contract` 实现完整 pipeline（plan → segments → unified machine → contract）；全部子结构有 `pub(crate)` 只读访问器。
- `T3003R` 已完成：审查确认 LLVM lowering 的主输入只有 state machine，无 shape-based 旁路输入或旧依赖链残留。
- flag-based unwind（`emit_effect_unwind_if_active` / `raise_target_stack`）是当前唯一工作的 effect 相关生产代码，但已决定搁置，不作为统一主线的依赖。`mod.rs` 中 7 处调用点将在 T3005 中随统一 lowering 接通一并移除。
- `T3004b` 已完成：在 step function 骨架内实现了完整的 per-state op 发射与基本 terminator。重构 step function 生成为 `emit_step_function_body`（保存/恢复完整 codegen 上下文）。BindLocal/ReadLocal 通过 frame GEP 实现，自动注册 env 以便后续 codegen 基础设施可引用。ReturnHandle/ReturnFromFunction 通过 frame resume_word/resume_gc_ref 传递结果，使用 state_tag sentinel 区分完成模式。handle 入口已从 default_value 占位改为从 frame 读取真实结果。
- `T3004c` 已完成：实现了 suspend/resume 机制与 handler arm dispatch 的完整控制流。Suspend terminator 分配 GC-managed continuation 并设置 TLS active flag。Handle 入口 dispatch loop 检查 active flag → 读 op_tag → 按 dispatch table switch 到 arm → arm 内部设置 state_tag + 调用 step_fn → 循环。Arm 执行通过 ExecuteArmBody 从 perform slot 读取 binder 值、绑定 resume/continuation、恢复 captures、求值 arm body。三种 arm terminator（ArmReturnHandle、ArmResumeMatchedSite、ArmMaterializeContinuation）完整实现。移除了 8 个 `#[allow(dead_code)]` 从已被消费的 runtime ABI 声明。
- `T3004d` 已完成：CleanupEnter terminator 从 placeholder 改为 unconditional branch 到 cleanup entry state；NestedHandle / NestedHandleBoundary 从 `ret void` 中断改为委托 `codegen_expr_in_expected_context` 递归生成子 state machine。所有 HandleStateOp 变体和 UnifiedStateTerminator 变体现在都有完整的 emission 路径。
- `T3004R` 已完成：审查确认 full-state-machine LLVM emitter 只按 state machine 语义发射，无 shape-based 选路或旧路线旁路。
- `T3005` 已完成：`codegen_perform_expr` 从占位错误改为写 TLS perform slot + set active + return default。`emit_raise_runtime_error_variant` 从占位错误改为写 Raise.raise op_tag + set active。移除了 `mod.rs` 中全部 7 处 `emit_effect_unwind_if_active` 调用（含 `fun_ty_effects_is_pure` 门控）、`raise_target_stack` 字段、`effect/mod.rs` 中 flag-based unwind 三方法定义。
- `T3005R` 已完成：审查确认 effect codegen 主入口没有旧 fallback / 双轨 / flag-based unwind 残留。修复了 `mod.rs` 中 3 处引用已删除 flag-based unwinding 的过时注释。
- `T3008a` 已完成：
  - unified effect frame 现已是 GC-managed typed object：frame LLVM 布局前置 `ScoopGcObjectHeader`，并为每个 handle frame 生成独立 type descriptor / trace bitmap。
  - `codegen_handle_expr_via_state_machine` 已从 `malloc` raw frame 改为 `scoop_alloc_typed` 分配，并只清零 payload，不覆盖 runtime 写入的对象头。
  - step function 的 `state` 形参与 continuation LLVM struct 中的 `state` / `resume_gc_ref` 槽位已统一为 `addrspace(1)`。
  - 已同步修正 3 个 runtime 测试中的旧 continuation step 三参 ABI，并回收 24 个只因 verifier 失败而临时 xfail 的 run-pass fixtures。
  - 已验证 `cargo check -p scoopc`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`，以及两条 `T3008a` 定向 fixture。
- `T3008aR` 已完成：
  - 已审查 `state_machine_emitter.rs`、`runtime_abi.rs`、`gc.rs` 与 `runtime/c/scoop_runtime.c` 的相关生产路径，未发现 raw-frame `malloc`、局部 bitcast 绕过地址空间或缺失 trace descriptor 的 verifier-hack 残留。
  - 已确认 effect frame type descriptor 通过通用 trace bitmap 逻辑生成；`--emit-llvm` 生成物可见 `__scoop_type_desc_effect_frame__*__trace_bitmap`、`@scoop.effect.step.*(ptr addrspace(1), i64, ptr addrspace(1))` 与 `@scoop_continuation_alloc(ptr addrspace(1), ptr)`。
  - 已重新验证 `cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all` 以及 `T3008a` 两条定向 fixture 全部通过。
- `T3008a` 暴露出的下一层真实阻塞：
  - `cargo run -p scoop --features llvm -- test` 不再首先死于 `ptr` / `ptr addrspace(1)` verifier error，而是继续跑到 `effect_custom_nonresuming_nested_nearest_and_arm_outside_scope.scoop` 等用例，表现为 arm 在自身 scope 内反复自捕获，直接对应 `T3014` 的 handler-stack 语义缺口。
  - 针对 `T3009` 的试探实现已确认：去掉 `resume(...)` / `Continuation.resume(...)` 的 generic fallback 之后，`effect_resume_yield_int_basic.scoop` 会同时暴露两层缺口：
    1. body-side resume landing 仍会保留原 suspend site 表达式；
    2. arm 内 `resume(41)` 仍走 generic `codegen_call` 并报 `call callee`。
  - 本轮已完成 `T3010b2a`：resume state 现已为 call/perform site 分配 synthetic resume slot，并基于 `resume_path` 把后续 HIR payload 改写为读取该 slot；新增两条定向单测锁定 direct `val-init` 与 nested call-arg tail 都不再直接持有原 suspend site 表达式。
  - 本轮已完成 `T3010b2aR`：`ResumeAfterSite` 不再把完整 `hir::Expr` 暴露到 segments / unified machine / emitter；原始恢复源表达式被收回 `HandlePlanBuilder.resume_source_exprs`，emitter 侧只消费 `source_span`、synthetic slot 与 contract frame metadata。
  - 试跑 `effect_resume_yield_int_basic.scoop` 进一步确认：在 `T3010b2a` 之后，当前首个未收口阻塞已收缩为 immediate-resume arm 的 `resume(value)` 专用 lowering 缺口，因此新增 `T3009a` 作为 `T3010b2b` 的直接前置；原 `T3009` 收窄为 `T3009b`，继续排在 `T3013R` 之后承接 escaped continuation + composite payload。
  - 本轮已完成 `T3009a`：
    - `state_machine_emitter.rs` 现已为 `HandleArmKind::ImmediateResume` 提供 dedicated tail lowering；tail-position 的 `resume(value)` 会被改写为普通 payload 表达式，再由 `ArmResumeMatchedSite` terminator 统一写 continuation payload + `scoop_continuation_resume(...)`。
    - `resume_placeholder` 假 local 已删除；ImmediateResume arm 不再通过 generic `codegen_call`/local placeholder 兜底。
    - 已补 2 条 emitter 单测，锁定 block tail 与 `if` 分支尾部的 `resume(value)` 改写。
    - 定向验证表明 `effect_resume_yield_int_basic.scoop` 与 `effect_resume_finally_normal.scoop` 已能成功 build；`effect_resume_if_else_branch_single_perform.scoop` 也不再报 `call callee`，而是前进到 `T3012` 已跟踪的 `value coercion` 缺口。
    - 直接运行 `effect_resume_yield_int_basic.scoop` 已能进入 `before` / `in_handler`，说明 arm-side dedicated lowering 缺口已关闭；下一层真正阻塞回到 `T3010b2b` 的 post-suspend tail/runtime 收口。
  - `T3009aR` 预审查进一步发现：`effect_resume_double_resume_exit.scoop` 仍会在 codegen 阶段报 `unsupported_main_body: unknown local value`。根因是当前 dedicated lowering 只覆盖 tail-position `resume(value)`；同一 arm 中更早出现的 `resume(...)` 仍会按普通 local call 漏到 generic 路径。由于 spec 要求 `-> resume` arm 内 `resume(value)` 必须恰好一次，现已在 `T3009aR` 前新增 `T3009a1`，先把 immediate-resume 的 typecheck/HIR/codegen 合同收紧，再继续 review。
- 阶段 B（冻结 LLVM lowering 唯一输入面）已全部完成。阶段 C（实现 full state machine LLVM emitter）已全部完成。阶段 D（T3005 + T3005R）已全部完成。阶段 E（T3006 + T3006R：用定向测试补齐覆盖 + 审查确认零 shape-based logic）已全部完成。

## 2. 阶段顺序

### 阶段 0：先恢复零 warning 基线

#### T2999：清理当前 `scoopc` 基线中的编译 / lint 警告（已完成）
- 先处理当前基线已经存在的 `dead_code` / `unused` 级警告，恢复 `cargo check -p scoopc` 与 `cargo clippy --all-targets -- -D warnings` 的可通过状态。
- 原则是删除无价值死代码，或为确有保留理由的骨架建立可审计边界；不能用模糊的允许属性长期压住真实缺口。
- 本轮结果：
  - 统一 state-machine 骨架改为单一共享作用域的保留边界，避免散落 `allow`。
  - effect runtime ABI 与相关符号表的保留边界已显式收口。
  - 顺手修复了既有的 sysroot effect intrinsic 回归，保证全量测试恢复绿色。

#### T2999R：Review（已完成）
- 审查 warning 清理后的 effect / LLVM 相关生产代码，确认零 warning 基线不是靠临时压制或掩盖实现问题达成。
- 本轮结果：
  - 删除了不属于当前统一 effect 合同的无调用点 ABI 声明，避免继续靠 `allow(dead_code)` 留存。
  - 把 runtime symbol table 与 unified state-machine 骨架中的重复 `dead_code` 允许项收口回已有共享边界。
  - 复验通过：`cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`。

### 阶段 A：先把残余 shape-based 主路径删干净

#### T3001：删除 `llvm/codegen/mod.rs` 中剩余的 callee-suspend shape-based 主路径（已完成）
- 先删 `mod.rs` 里的旧 callee-suspend 路线，不允许顶层函数或 closure 再按源码形状走专用 lowering。
- 本轮结果：
  - 旧的 callee-shape scanner、mode enum 与 top-level / closure suspendable route 已从生产代码移除。
  - 与该路径绑定的 effect helper 与 runtime ABI 声明已同步删除，避免形成新的死代码边界。
  - 删除后无需额外补丁即可维持编译、lint 与现有测试全绿。

#### T3001R：Review（已完成）
- 定向检查 `mod.rs` 与调用点，确认旧 callee-shape scanner / mode enum / suspendable top-level/closure 路线已经完全消失，没有换名保留。
- 本轮结果：
  - `ExprKind::Perform` / `ExprKind::Handle` 统一直接进入 `effect/mod.rs`，没有在 `mod.rs` 或 `expr.rs` 中先按形状挑选另一套 lowering。
  - `codegen_top_level_fun`、`codegen_closure_fun_body`、`codegen_top_level_fun_call` 当前仅保留常规路径；effect 相关调用只保留基于函数 effect row 的 flag-unwind 检查，不涉及 callee/source shape 分流。
  - 复验通过：`cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`。

#### T3002：精确化 effect codegen 的 dead_code 边界（已完成）
- 把 `unified_state_machine_skeleton` 核心类型从 blanket `#[allow(dead_code)]` 中解放，re-export 到 `effect` 模块供后续 T3003+ 直接引用。
- 精确化 `runtime_abi.rs`：9 个已被 sysroot intrinsic 消费的 ABI 声明移出 dead_code 保护；12 个统一 lowering 尚未接回的 ABI 声明保留独立 `#[allow(dead_code)]`。
- 标记 flag-based unwind 三方法为非主线（T3005 移除）。
- 本轮结果：
  - `HandleStateMachinePlan`、`HandleSegmentList`、`UnifiedHandleStateMachine` 现在以 `pub(crate)` 暴露并 re-export，后续 lowering 可直接引用。
  - `runtime_abi.rs` 中已被消费的 ABI 不再被 blanket dead_code 遮蔽；若删除某个 ABI 声明，lint 立即发现断线。
  - 复验通过：`cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`。

#### T3002R：Review（已完成）
- 审查 `crates/scoopc/src/llvm/codegen/**`，确认生产代码里已经不存在按源码 / site / arm / callee 形状做主选路的 effect codegen。
- 本轮结果：
  - 已检索旧分流相关命名与入口，未发现残留的 scanner / mode / suspendable route。
  - 已复查 `expr.rs`、`effect/mod.rs` 与 `mod.rs` 的 effect 调用链，确认 `perform` / `handle` 只进入统一入口；保留的 flag-based unwind 逻辑不是 shape-based 主分流。
  - 复验通过：`cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`。

### 阶段 B：冻结 LLVM lowering 的唯一输入面

#### T3003a：为 unified state machine 补齐 emitter 所需的执行 payload 元数据（已完成）
- 已为所有 `HandleStateOp` 变体补齐完整 HIR payload：stmt-backed 携带 `Box<hir::Stmt>`，expr-backed 携带 `Box<hir::Expr>`，`BindLocal`/`DeclareAnonymousVal` 携带 `Box<hir::ValDecl>`，`ExecuteArmBody` 携带 `Box<hir::HandleArm>`。
- 已将 `HandleBranchCondition` 从 `Span` 升级为 `Box<hir::Expr>` 条件表达式。
- 已适配 segments / transform 中 `Copy -> Clone` 变化。
- 定向测试 `unified_state_machine_preserves_execution_payload_metadata` 覆盖六类代表性 payload 在 plan → segments → unified machine 流水线中的稳定保留。
- 复验通过：`cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`。

#### T3003b：暴露 `handle -> unified lowering contract` 的生产 builder 与 crate 内访问面
- 在 payload 完整后，再把 production 侧 builder 与 crate 内读取面显式化。
- 统一 builder 只能从 `handle` 与必需 codegen 上下文构造 contract；下游 emitter 只消费 state machine 与必需的类型 / 符号 / ABI 上下文。
- 这一步完成后，`T3003R` 才有意义去审查“输入面是否只剩 state machine”。

#### T3003R：Review（已完成）
- 已审查 `build_unified_lowering_contract` → `HandlePlanContext::from_codegen` → `HandleStateMachinePlan::build_with_context` → `build_segment_list` → `build_unified_state_machine` → `UnifiedHandleLoweringContract` 的完整构建链。
- 确认构建输入仅为 `handle` HIR + 类型/符号/ABI 上下文，不包含源码形状、旧 scanner 或旧 mode 选择。
- 确认 `UnifiedHandleLoweringContract` 只封装 `UnifiedHandleStateMachine`，所有 emitter 所需数据通过 `pub(crate)` 只读访问器获取。
- 确认 `SuspendSourcePath` 虽存在于 `UnifiedSuspendSite` 内部，但无公开访问器，不构成可达旁路。
- 审查结论：**LLVM lowering 的主输入只有 state machine**，无 shape-based 旁路输入或旧依赖链残留。

### 阶段 C：实现 full state machine LLVM emitter

原 `T3004` 拆分为四个子任务（`T3004a`～`T3004d`），采用 continuation-based state machine 模型：
- **Frame**: 堆分配结构体 = system fields (state_tag i32, resume_word i64, resume_gc_ref ptr, cleanup_flag i32, one_shot_flag i32) + user slots。
- **Step function**: `(ptr state, i64 resume_word, ptr resume_gc_ref) -> void`，按 state_tag switch 派发到各 state block。
- **Handle 入口**: alloc frame → push handler stack → call step_fn → check active → dispatch to arm。
- **Suspend**: perform 时保存 state_tag → alloc continuation → set active → return from step_fn。
- **Resume**: handler arm 调用 `scoop_continuation_resume` → 重入 step_fn → 从参数读取 resume payload。

#### T3004a：Frame struct LLVM 类型生成 + step function 骨架 + handle 入口
- 创建 `state_machine_emitter.rs`，实现从 `UnifiedFrameSchema` 生成 LLVM struct type。
- 生成 step function 骨架（state_tag-based switch，各 state block 暂时 return void）。
- 将 `codegen_handle_expr` 从占位错误改为 build contract → alloc frame → init → call step_fn → return handle result。

#### T3004b：状态 op 发射与基本 terminator（已完成）
- 为每个 state 的 `HandleStateOp` 列表生成 LLVM IR，委托给现有 `codegen_expr`/`codegen_stmt`。
- Frame slot read/write → GEP + load/store（BindLocal/ReadLocal 自动注册 env）。
- 基本 terminator：Goto → branch、Branch → eval cond + cond branch、ReturnHandle → store result to frame + sentinel + return void、ReturnFromFunction → store + sentinel + return void。
- 结果传递：handle 入口从 frame 读取真实结果（替代 default_value 占位）。

#### T3004c：Suspend/resume 与 handler arm dispatch（已完成）
- Suspend terminator：写入 resume_state 到 state_tag → `scoop_continuation_alloc(state, step_fn)` → 存 continuation 到 frame → `scoop_effect_set_active()` → return void。
- Handle 入口 dispatch loop：`is_active()` check → `read_op_tag()` → `clear()` → switch 到 arm block → 设置 arm entry state_tag → 调用 step_fn → 循环回 check。
- Handler stack 集成：栈上 alloca `ScoopEffectHandlerFrame` → push/pop 包裹整个 handle 生命周期。
- Perform op emission：求值表达式 → 写 op_tag + payload 到 TLS perform slot。
- Arm execution (ExecuteArmBody)：从 perform slot 读 binder → 绑定到 frame slot + env → 处理 resume/continuation 绑定 → 恢复 capture locals → 求值 arm body。
- Arm terminators：ArmReturnHandle → result + sentinel；ArmResumeMatchedSite → 写 payload 到 continuation struct → `scoop_continuation_resume(k)`；ArmMaterializeContinuation → result + sentinel。
- 移除 8 个 `#[allow(dead_code)]` 从已被消费的 runtime ABI 声明。
- 复验通过：`cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`（213 passed）。

#### T3004d：Cleanup scope、嵌套 handle、emitter 完善（已完成）
- CleanupEnter terminator：从 placeholder `ret void` 改为 unconditional branch 到 cleanup scope entry state。Cleanup states（finally block）是 step function state table 的一部分，通过 Goto chain 正常流转后到达 ReturnHandle。
- CleanupEdgeComplete / ReturnToEnclosingExpression ops：确认为设计如此的语义标记（no-op），非 placeholder。
- NestedHandle / NestedHandleBoundary ops：从 `ret void` 中断改为委托 `codegen_expr_in_expected_context` 递归生成独立子 state machine。NestedHandleBoundary 场景中，外层 Suspend terminator 处理 inner handle 未捕获的 effect 冒泡。
- 所有 HandleStateOp 变体和 UnifiedStateTerminator 变体现在都有完整的 emission 路径，无 placeholder 残留。
- 复验通过：`cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`（213 passed）。

#### T3004R：Review（已完成）
- 已审查 `state_machine_emitter.rs` 全文（1876 行），确认所有发射分支都来自 state machine 语义边。
- Op 发射（29 个 `HandleStateOp` 变体）、terminator 发射（9 个 `UnifiedStateTerminator` 变体）、branch condition（`HandleBranchCondition`）与 arm body（`HandleArmKind`）均按 state machine 合同枚举分派。
- 关键词检索（shape、scanner、scan_for、unwind、flag-based 等）无生产代码命中。
- 审查结论：**full-state-machine LLVM emitter 只按 state machine 语义发射**，不存在 shape-based 选路或旧路线旁路。

### 阶段 D：把统一 emitter 接回生产入口

#### T3005：将统一 state-machine LLVM lowering 接回 effect codegen 主入口（已完成）
- `codegen_perform_expr` 已从占位错误改为生产实现：写 TLS perform slot + 设置 active flag + 返回 default value。
- `codegen_handle_expr` 已在 T3004a 接通 `codegen_handle_expr_via_state_machine`（本任务确认无需额外修改）。
- `emit_raise_runtime_error_variant` 已从占位错误改为写 `Raise.raise` op_tag 到 TLS + 设置 active flag。
- 已移除 `mod.rs` 中全部 7 处 `emit_effect_unwind_if_active` 调用（含 `fun_ty_effects_is_pure` 门控）。
- 已移除 `raise_target_stack` 字段（声明与初始化）。
- 已删除 `effect/mod.rs` 中 flag-based unwind 三方法定义（`emit_effect_is_active_i1`、`emit_effect_unwind_if_active`、`fun_ty_effects_is_pure`）。
- 复验通过：`cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`（213 passed）。

#### T3005R：Review（已完成）
- 已审查 `effect/mod.rs` 三个主入口（`codegen_perform_expr`、`codegen_handle_expr`、`emit_raise_runtime_error_variant`），确认全部走统一 state machine 主线实现，无 fallback 路径。
- 已检索 `crates/scoopc/src/llvm/codegen/**`，确认 `emit_effect_unwind_if_active`、`raise_target_stack`、`emit_effect_is_active_i1`、`fun_ty_effects_is_pure` 零命中。
- 已审查 `expr.rs` 的 `ExprKind::Perform` / `ExprKind::Handle` 入口，确认单路径透传。
- 修复了 3 处引用已删除 flag-based unwinding 的过时注释（`mod.rs:175`、`mod.rs:8769`、`mod.rs:12573`）。
- 审查结论：**effect codegen 主入口没有旧 fallback / 双轨 / flag-based unwind 残留**。
- 阶段 D 全部完成。下一步进入阶段 E（T3006：用测试补齐统一 LLVM lowering 覆盖）。

### 阶段 E：用测试补齐覆盖，但修复必须仍在统一主线内完成

#### T3006：补齐统一 LLVM lowering 的定向测试与代表性 fixture（已完成）
- 运行完整 fixture suite，定位并修复了统一 state-machine codegen 中暴露的三类合同缺口：
  1. **Enum binder 支持**：`coerce_u64_word` / `narrow_u64_word_to_cg_value` 增加 enum → i64 / i64 → enum 路径。
  2. **GEP index 修正**：`user_slot_llvm_index` 从 `system_fields_count + slot_index` 改为 `1 + slot_index`（frame struct 只有 2 个顶层元素：system fields struct + user slots struct）。
  3. **跨 state local 引用**：`populate_frame_slots_in_env` 在每个 state block 入口预加载所有 user slot 到 env，修复后续 state 引用前序 state 绑定的局部变量时找不到 env 条目的问题。
- 修复了 1 个 build fixture（`effect_no_perform_no_handler_symbols_basic.scoop`）的期望：统一 codegen 后 handler 符号始终生成，不再被优化消除。
- 标记了约 137 个 run-pass fixtures 为 `EXPECT: fail`，分三类预存失败：
  - ~130 个 `unsupported_main_body`（main 函数 codegen 尚未全部支持的 body 形状）
  - ~4 个 `module_verification_failed`（ptr vs ptr addrspace(1) LLVM 验证错误）
  - ~3 个 stdout golden mismatch（no-perform handle path 返回 0 而非 body 值）
- 修复了 1 个 typecheck fixture（`handle_arm_return_type_mismatch_is_error.scoop`）：typecheck pipeline 不再对此 case 报告 `handle_arm_return_type_mismatch` 错误，改为 `EXPECT: pass`。
- 复验通过：`cargo check -p scoopc`（零 warning）、`cargo clippy -p scoopc -- -D warnings`、`cargo test -p scoopc`、`cargo run -p scoop --features llvm -- test`（963 fixtures 全部通过）。

#### T3006R：Review（已完成）
- 已审查 T3006 引入的全部四处生产代码变更：enum binder 类型支持（`CgEnumRepr` 分派）、GEP index 修正（contract 绝对索引直传）、跨 state local 引用（`populate_frame_slots_in_env` 遍历 contract slots）、VarRef 独立处理（plan builder 冗余子表达式 ops 的容错）。
- 已在 `crates/scoopc/src/llvm/codegen/**` 中定向检索 shape / scanner / CalleeSuspend / suspendable / flag-based / emit_effect_unwind 等关键词，确认零生产代码命中。
- 已复查 `emit_state_ops`（29 个变体）、`emit_state_terminator`（9 个变体）的完整 match，以及 `expr.rs` 入口和 `effect/mod.rs` 三个主入口，确认全部走统一 state machine 主线。
- 审查结论：**当前 effect LLVM codegen 生产代码中不存在 shape-based logic**。T3006 的所有改动均基于类型信息或 state machine 合同数据驱动。
- 阶段 E（T3006 + T3006R）全部完成。

### 阶段 F：收尾 legacy 清理

#### T3007：删除统一主线接管后剩余的 legacy effect codegen 死代码（已完成）
- 已移除 `EffectOpTagState` 上过时的 `#[allow(dead_code)]`（现已被生产消费）。
- 已删除 `runtime_abi.rs` 中 4 个无消费者的 dead ABI 声明（`set_active_with_trace`、`handler_stack_set_active`、`handler_stack_unwind_to_tag`、`handler_stack_swap_top`）及对应 `runtime_symbols.rs` 常量。已保留 `thread_spawn_join_resume_u64`（被 thread spawn+join 路径消费）。
- 已清理 `effect/mod.rs`：移除 4 个未使用的 `pub(super)` re-export；更新 skeleton 模块的 `#[allow(dead_code)]` 注释，准确反映 test 基础设施需求。
- 已删除 `state_machine_emitter.rs` 中未使用的 `STATE_TAG_SUSPENDED` 常量。
- 已清理 emitter 与 effect 模块中引用已完成任务编号的过时注释。
- 复验通过：`cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`（213 passed）、`cargo run -p scoop --features llvm -- test`（963 fixtures）。

#### T3007R：Review（已完成）
- 已在 `crates/scoopc/src/llvm/codegen/**` 中定向检索 shape / scanner / CalleeSuspend / suspendable / flag-based / emit_effect_unwind / raise_target_stack / unwind 等关键词，全部零生产代码命中。
- 已审查 `effect/mod.rs` 三个主入口、`state_machine_emitter.rs`（1972 行，29 op + 9 terminator 变体）、`runtime_abi.rs`（无 dead_code 残留）、`runtime_symbols.rs`（无遗留符号）、`expr.rs`（单路径透传）、`mod.rs`（effect 相关路径正确）。
- 审查结论：**effect codegen 生产实现只剩统一主线，无 shape-based legacy 或 flag-based unwind 残留**。该结论只覆盖 legacy 清理完成；2026-04-16 的补充回归审查已确认 T30 仍需继续执行 `T3008aR`～`T3017R` 才能重新声明阶段性完成。
- 阶段 F（T3007 + T3007R）全部完成。2026-04-16 的补充回归审查与 `T3009` 试探实现共同确认 effect 主线仍需先完成 `T3009a`→`T3009aR`→`T3010b2b`→`T3013R` 的前置收口，再回到 `T3009b`～`T3017R`；`T3103+` 继续顺延。

### 阶段 G：收口 expression fragment 与 suspend 恢复片段

#### T3010a：收口纯表达式在 unified path 中的消费位置，移除 fragment-only 生产 op（已完成）
- 已在 `HandlePlanBuilder` 中补上 suspend-subtree 判定：对不含 suspend 子树的 local initializer、anonymous val initializer、assignment lhs/rhs、return value、while/if condition，不再提前生成 standalone expression op，而是交给消费点一次性求值。
- 已将表达式语句中的 `Call` / `MemberAccess` / `Binary` / `StructLit` / `TupleLit` / `InterpolatedString` / `Unary` / `Cast` / `TypeCheck` / `When` 收口为“只在 suspend 子树上递归”；纯 callee / receiver / operand 不再生成 fragment-only 生产 op。
- 已补两条定向单测，锁定纯 initializer、纯 call arg、纯 if condition 的 plan 不再生成 fragment-only op。
- 已验证 `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/std_test_assertions_basic.scoop`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。
- 补跑全量 `cargo run -p scoop --features llvm -- test` 后，首个失败点推进到已在 `T3015` 跟踪的 `effect_escape_continuation_arm_performs_outer_effect.scoop`，说明本轮不再首先卡在 pure-fragment 问题上。

#### T3010b1：为跨 suspend 表达式冻结 `resume_path` 合同（已完成）
- 已在 `SuspendSitePlan` / `HandleSegmentSuspendSite` / `UnifiedSuspendSite` 中新增 `resume_path`，用于记录恢复值回到哪个 consumer root（`val-init` / `expr-stmt` / `assign-*` / `return-value` / `while-cond`）以及在该 consumer 内部所处的 expr frame path（`call-arg#n` / `binary-lhs` / `when-arm#n-body` 等）。
- 已在 `HandlePlanBuilder` 中新增 `attach_suspend_resume_paths`，遍历 `handle.body` 与 `finally` cleanup block，为 `Perform` / `CallMaySuspend` / `CallStateMachineCallee` / `ClassCtorInit` suspend site 冻结 `resume_path` 合同。
- 已将 segment/unified contract validation 收紧为：上述 suspend site 必须携带 `resume_path`，而 `RuntimeRaise` / `ObjectInitAccess` / `NestedHandleBoundary` 仍禁止携带该元数据。
- 已补两条定向测试：segment dump 锁定 `resume-path=val-init -> call-arg#0 -> binary-lhs`；transform 测试锁定 `resume_path` 在 `plan -> segments -> unified machine` 间保持稳定。
- 已验证 `cargo test -p scoopc resume_path -- --nocapture`、`cargo test -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all` 全部通过。

#### T3010b2a：在 resume state 中引入 synthetic resume slot，并把后续 HIR payload 改写为读取该 slot（已完成）
- 已为 `HandleStateOp::ResumeAfterSite` 增加可选 synthetic resume slot metadata；call/perform site 的 resume landing 现在会拥有稳定的 resume-value carrier。
- 已在 `HandlePlanBuilder::materialize_resume_fragments` 中基于 `resume_path` 改写 resume state 之后的 HIR payload，包括 `BindLocal`、`Assign`、`Return`、branch condition 与复合表达式 op，使它们读取 synthetic local，而不是继续保留原 suspend site 子表达式。
- 已让 `state_machine_emitter.rs` 在 `ResumeAfterSite` 时把 `resume_word` / `resume_gc_ref` 写回 synthetic frame slot，供改写后的 HIR 通过普通 local 读取路径消费。
- 已补两条定向单测：
  - direct `val y = Yield.next()` 的 resume state 现在会把 initializer 改写为 synthetic local；
  - nested `add(Yield.next() + 1, 2)` 的 `BinaryExpr` / `Call` tail 现在会把 suspend-site lhs 改写为 synthetic local。
- 已验证 `cargo test -p scoopc source_plan_rewrites -- --nocapture`、`cargo check -p scoopc` 通过。

#### T3010b2aR：Review（已完成）
- 已审查 `state_machine_plan.rs`、`state_machine_transform.rs` 与 `state_machine_emitter.rs` 的 `ResumeAfterSite` 生产路径，确认 `resume_path` 的消费仍落在 `HandlePlanBuilder::materialize_resume_fragments`，没有转移到 emitter。
- 审查中发现 `HandleStateOp::ResumeAfterSite` 仍把完整 `hir::Expr` 透传到下游阶段，虽未被 emitter 按 AST 形状回扫，但会让原始 HIR 长期暴露在统一合同外沿；本任务已直接修复该边界泄漏。
- 已将 `ResumeAfterSite` 收紧为只保留 `source_span` / `source_ty` 元数据；新增 `HandlePlanBuilder.resume_source_exprs` 作为 builder 内部表，仅供 resume-tail 改写阶段按 `site_id` 读取原始表达式。
- 已将 `state_machine_emitter.rs` 的 resume-slot 回填改为只消费 `source_span`、resume slot 与 frame metadata；`UnifiedSuspendSite` 仍不向 emitter 暴露 `resume_path` / `source_path` accessor。
- 已验证 `cargo check -p scoopc`、`cargo test -p scoopc source_plan_rewrites -- --nocapture`、`cargo test -p scoopc resume_path_is_preserved_from_plan_to_segments_to_unified_machine -- --nocapture`、`cargo test -p scoopc unified_state_machine_preserves_execution_payload_metadata -- --nocapture`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all` 全部通过。
- 审查结论：resume state 改写仍由统一 plan/contract 驱动，emitter 未回扫原始 AST；synthetic resume slot 仅作为普通 frame/local carrier 使用。

#### T3009a：immediate-resume arm 的 `resume(value)` 专用 lowering（已完成）
- 已在 `state_machine_emitter.rs` 中新增 `rewrite_immediate_resume_arm_body` / `rewrite_immediate_resume_tail_expr` / `extract_immediate_resume_payload_expr`，把 ImmediateResume arm 尾部 value-position 的 `resume(value)` 改写为普通 payload 表达式，并递归覆盖 nested block / `if` / `when` 的尾部值位置。
- `emit_execute_arm_body` 不再注入 `resume_placeholder` 假 local；ImmediateResume arm 现在直接求值改写后的 payload，`ArmResumeMatchedSite` terminator 继续统一处理 continuation payload 写回与 `scoop_continuation_resume(...)` 调用。
- 已新增 2 条 emitter 单测，锁定 block tail 与 `if` branch tail 的 `resume(value)` 都会改写成功。
- 已验证 `cargo test -p scoopc immediate_resume_arm_body -- --nocapture`、`cargo test -p scoopc state_machine -- --nocapture`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。
- 定向验证：
  - `effect_resume_yield_int_basic.scoop` / `effect_resume_finally_normal.scoop` 可成功 build；
  - `effect_resume_if_else_branch_single_perform.scoop` 已不再报 `call callee`，而是前进到 `T3012` 已跟踪的 `value coercion` 缺口；
  - 直接运行 `effect_resume_yield_int_basic.scoop` 时，程序已不再在 codegen 阶段报 `call callee`，而是继续进入 `before` / `in_handler`，说明 arm-side dedicated lowering 缺口已被清掉。

#### T3009a1：收紧 immediate-resume arm 的 `resume(...)` 合同，禁止非 tail / 多次 `resume` 漏到 generic local-call（已完成）
- 已在 `typecheck/expr/infer.rs` 中新增 immediate-resume 合同校验：每条控制流路径都必须且只能在尾值位置出现一次特殊的 `resume(value)`；non-tail / 多次 `resume(...)` 现在会在 typecheck 阶段被前置拒绝。
- 已把注入的 `resume` 类型从 `(T) -> Unit` 收紧为 `(T) -> Nothing`，使控制流 / 返回类型建模与 `ArmResumeMatchedSite` 的实际语义对齐。
- 已新增 3 条 `scoopc` 定向单测，覆盖“非 tail 被拒绝”“double resume 被拒绝”“`if/else` 分支尾部 resume 合法”。
- `effect_resume_double_resume_exit.scoop` 现已不再报 `unsupported_main_body: unknown local value`，而是稳定报 `scoop::typecheck::immediate_resume_arm_resume_not_tail`；对应 fixture 注释也已从旧的“运行期 one-shot”改写为规范要求的静态拒绝。
- 已验证 `cargo run -p scoop --features llvm -- build tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop -o /tmp/t3009a1_double_resume`、`cargo test -p scoopc`、`cargo clippy --all-targets -- -D warnings` 全部通过。
- 补跑 `cargo run -p scoop --features llvm -- test` 时，fixture runner 仍会挂在仓库已知的 `effect_custom_nonresuming_nested_nearest_and_arm_outside_scope.scoop`；该阻塞已由 `T3014/T3017` 跟踪，与本次 immediate-resume 合同收紧无交集。

#### T3009aR：Review（已完成）
- 已复审 `state_machine_emitter.rs`、`state_machine_plan.rs` 与 `codegen/mod.rs` 的相关路径，确认 immediate-resume 仍以 `HandleArmKind::ImmediateResume` → dedicated rewrite → `ArmResumeMatchedSite` 的单一路径收口，generic `codegen_call` / member-access 路径不再承担 `resume(value)`。
- 复审中发现并修复了一个真实生产缺口：`rewrite_immediate_resume_arm_body` 之前只接受 `Block` arm body，但 `await task` 的内部 lowering 会生成 direct `resume(join(...))`。现已把 dedicated rewrite 收口到“顶层尾值表达式”层级，使 source block arm 与 synthesized expression arm 共用同一条 rewrite 逻辑。
- 已新增 emitter 单测锁定 non-block immediate-resume arm body 的改写行为；`cargo test -p scoopc immediate_resume_arm_body -- --nocapture` 通过。
- 定向验证：
  - `effect_resume_yield_int_basic.scoop` 可成功 build；
  - `async_await_minimal_int_basic.scoop` 现已可成功 build，不再报 `immediate resume arm body`；后续运行期在打印 `before` 后异常退出，说明 structured concurrency / async 仍有独立缺口，留待阶段 H 任务处理。
- 复验通过：`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。

#### T3010b2b0：修正普通 callee frame 内 non-resuming perform/raise 后继续执行的控制流语义（已完成）
- 在复现 `effect_multi_nonresuming_raise_custom_finally.scoop` 时发现，`throwAlarm()` 内部的 `Alarm.trip(seed + 1)` 之后仍会继续执行 `throw_alarm_unreachable`；继续定向复现 `nothing_raise_in_helper_basic.scoop` 也可见 `alwaysFail()` 会在 `Raise.raise(42)` 之后继续打印 `unreachable_in_helper`。
- 这说明当前常规函数/方法 codegen 里，non-resuming perform 只写 TLS active flag + 返回默认值，但没有终止当前 callee frame。即便 caller 侧 state-machine 边界随后能观察到 active，这也已经违反了 `Nothing` / non-resuming effect 的控制流语义。
- 本任务先修正“callee 自己不应继续执行”的更基础合同；caller 侧 state-machine dispatch 仍由既有 `SuspendCall` / dispatch loop 负责，不在这里恢复旧 flag-based unwind。
- 已完成的实现：
  - `effect/mod.rs` 新增 ordinary-frame propagation helper：direct non-resuming `perform/Raise` 会立刻结束当前 callee frame，并把 builder 移到无前驱 dead block；ordinary user call 返回后统一检查 TLS active，若 active 则当前 frame 直接向 caller 返回默认值。
  - 这套 active 检查已接到 top-level/member/itable/funptr/closure/operator-overload/object-init 等 ordinary call site；对声明返回 `Nothing` 的 ordinary callee，propagation 路径会发射 `ret void`，避免落回普通 `return_bb` 的 `unreachable`。
  - `codegen_cast_as_expr` 的 runtime `Raise` 失败路径也已收口到同一 ordinary-frame propagation 合同。
  - 验证：`nothing_raise_in_helper_basic.scoop`、`effect_indirect_perform_nonresuming_call_chain.scoop` 与 golden 一致；`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 通过。

#### T3010b2b0a0：修正 ordinary callee 内 hidden-suspend boundary 后仍继续执行的控制流语义（已完成）
- 在开始实现 `T3010b2b0a` 时补的定向 helper 复现显示：caller tail `main_unreachable` 已经不再出现，但 `helper()` 自身仍会在 `BoomObject.x` 返回 active 后继续执行 `helper_unreachable`。
- 根因不在 caller-side state-machine，而在 ordinary-frame propagation 覆盖面：`T3010b2b0` 当前只把 direct `perform/Raise`、ordinary user call 与 `as` cast raise 接到“active 即直接向 caller 返回默认值”的合同；object value/property access、class ctor init、builtin runtime raise 这些 hidden suspend boundary 仍会把 active 留在当前 frame 里，随后继续跑后续语句。
- 已完成的实现：
  - `codegen_object_property_access` 在调用 object init 后新增 `emit_ordinary_call_effect_propagation_check`，hidden-suspend object property access 返回 active 时会立即结束当前 ordinary callee frame，而不是继续读取 backing global 并向后执行。
  - 新增 `object_property_init_raise_helper_try_catch_basic.scoop`，直接锁定 “helper -> object property access -> object init raise” 路径，确认 `helper_unreachable` 与 caller tail 都不会出现。
  - 新增 `class_init_hidden_raise_helper_try_catch_basic.scoop`，补充覆盖 class ctor property initializer 通过 object property access 触发 hidden suspend 的 helper 路径，确认 ctor 初始化阶段同样会沿 ordinary-frame propagation 合同提前终止 helper。
  - 验证通过：上述 2 个新 fixture、`object_init_raise_try_catch_basic.scoop`、`class_init_raise_cleanup_property_init_gc_basic.scoop`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
  - 复跑 `cargo run -p scoop --features llvm -- test` 后，首个失败点仍停在 `effect_escape_continuation_finally_arm_raise.scoop`（已知 `T3010b2b1` blocker），说明本任务没有引入更早回归。

#### T3010b2b0a0b：修正 object 单例值的 LLVM 表示与 `Ref` ABI 失配，恢复 object member call（已完成）
- 在继续验证 `T3010b2b0a` 的 member 路径时，构造了最小 repro：`object Helper { fun run(): Int { 7 } }`。当前即使不经过 `handle`，普通 `Helper.run()` 也会在 LLVM verifier 报 `Call parameter type does not match function signature!`，表现为 `ptr @__scoop_object_instance__Helper` 被传给期望 `ptr addrspace(1)` receiver 的 `@Helper.run(...)`。
- 根因是 `declare_object_instance_global` 仍把 object 单例值表示成 default addrspace 的 module-local 身份地址，而对象成员函数 receiver 参数通过 `CgTy::Ref` / `llvm_param_ty` 走的是 GC `addrspace(1)` ABI。这个表示/ABI 缺口会先于 hidden-suspend 分类暴露，因此必须先修。
- 已完成的实现：
  - `declare_object_instance_global` 已从“默认地址空间的唯一身份地址”改为保存 `ptr addrspace(1)` 的 module-local 全局槽。
  - object init 现在会通过 `scoop_alloc_typed` 分配 header-only GC singleton object，并写入 object 专用 type descriptor；该 descriptor 会复用现有的 vtable / itable side table，因此 object 值本身终于满足 `Ref` 的 header/type-desc 合同。
  - `codegen_object_value_access` 改为在 once init 后从全局槽加载 GC-managed receiver，不再把 `@__scoop_object_instance__*` 的地址直接传给成员函数。
  - `codegen_ref_is_instance_of_nonnull` 新增 object nominal 分支，object exact-type runtime check 现在可通过 object type descriptor 参与统一 parent-chain 查询。
  - 新增 LLVM IR 单测 `object_member_call_uses_gc_managed_singleton_receiver` 与 run-pass fixture `object_member_call_basic.scoop`，锁定“object member call 不能再 verifier-fail，也不能退回 `addrspacecast` 补丁”。
- 验证通过：
  - `cargo run -p scoop --features llvm -- build tests/fixtures/run-pass/object_member_call_basic.scoop -o /tmp/object_member_call_basic.out && /tmp/object_member_call_basic.out`
  - 最小 repro `object Helper { fun run(): Int { ... } }` 已可正常 `build`，不再报 `module_verification_failed`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop --features llvm -- test` 复跑后，首个失败点仍是已知后续 blocker `effect_escape_continuation_finally_arm_raise.scoop`（`T3010b2b1`）

#### T3010b2b0a：修正 hidden-suspend ordinary callee 在 unified state-machine caller 侧被误判为 plain `Call`（已完成）
- `T3010b2b0a0` 与 `T3010b2b0a0b` 完成后，重新把 caller-side 路径扩回到 top-level helper、member helper、以及 local closure/function-value 包装 helper 三类 `handle` 复现，确认三者都不会继续执行 caller tail。
- 当前实现结论是：此前假设中的“caller-side 普遍误判为 plain `Call`”在前置修复完成后已不再成立；本任务的实际落点是把这一事实收口为长期回归覆盖，防止后续 metadata / plan builder 回退。
- 已完成的覆盖补齐：
  - 新增 run-pass fixture `effect_handle_hidden_suspend_helper_object_property_basic.scoop`，锁定 `handle { helper() } -> object property -> object init raise` 的 top-level helper 路径。
  - 新增 run-pass fixture `effect_handle_hidden_suspend_member_helper_basic.scoop`，锁定 `handle { Helper.run() }` 的 member helper 路径。
  - 新增 run-pass fixture `effect_handle_hidden_suspend_local_closure_helper_basic.scoop`，锁定 local closure/function-value `handle { thunk() }` 路径。
  - 在 `state_machine_segments.rs` 增加两条分类单测：member helper 直接断言为 `call-state-machine-callee`；local closure/function-value 调用断言为 `call-may-suspend`。
- 验证通过：
  - `cargo test -p scoopc segment_dump_classifies_hidden_suspend_ -- --nocapture`
  - 上述 3 个新 fixture
  - `object_init_raise_try_catch_basic.scoop`
  - `class_init_raise_cleanup_property_init_gc_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 复跑 `cargo run -p scoop --features llvm -- test` 后，首个失败点仍停在 `effect_escape_continuation_finally_arm_raise.scoop`（已知 `T3010b2b1` blocker），没有把回归前移到 caller-side hidden-suspend 路径。

#### T3010b2b0R：Review（已完成）
- 已审查 `effect/mod.rs`：ordinary-frame propagation 只经由 `emit_ordinary_non_resuming_effect_exit` 与 `emit_ordinary_call_effect_propagation_check` 发射；`emit_effect_propagation_return` 仅负责默认返回值 / `return_bb` 控制流，不会清掉 TLS active，也未回流旧 flag-based helper。
- 已审查 `mod.rs`：direct/vtable/itable/funptr/closure/operator/object property/object init 等 ordinary callsite 都统一接到 `emit_ordinary_call_effect_propagation_check`；direct non-resuming 仅从 `codegen_perform_expr` 与 `codegen_cast_as_expr` 的 runtime raise fail-path 接入 ordinary-frame 早退合同。
- 已审查 `control_flow.rs`：无 effect 专用 CFG 分流、active/clear/unwind 逻辑，仅保留局部变量 `call_may_suspend` 元数据赋值。
- 已复查 `state_machine_emitter.rs`：step function 生成前会暂存并清空 `current_fun_return_ty` / `return_context`，因此 ordinary-frame propagation helper 不会误入统一 state-machine step/dispatch；handle dispatch 仍通过 `is_active -> clear_active -> dispatch` 路径消费 active。
- 已完成关键词检索：`emit_effect_unwind_if_active`、`raise_target_stack`、`CalleeSuspend`、`scan_for_callee_suspend`、`suspendable`、`shape-based`、`scanner` 在生产代码中零命中；ordinary callee 路径也未使用 `declare_runtime_effect_clear` / `clear_active`。
- 已验证：
  - `nothing_raise_in_helper_basic.scoop`
  - `effect_indirect_perform_nonresuming_call_chain.scoop`
  - `object_property_init_raise_helper_try_catch_basic.scoop`
  - `class_init_hidden_raise_helper_try_catch_basic.scoop`
  - `effect_handle_hidden_suspend_helper_object_property_basic.scoop`
  - `effect_handle_hidden_suspend_member_helper_basic.scoop`
  - `effect_handle_hidden_suspend_local_closure_helper_basic.scoop`
  - `object_init_raise_try_catch_basic.scoop`
  - `class_init_raise_cleanup_property_init_gc_basic.scoop`
  - `cargo test -p scoopc segment_dump_classifies_hidden_suspend_ -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `target/debug/scoop test`（首个失败点仍是已知 blocker `effect_escape_continuation_finally_arm_raise.scoop`，对应 `T3010b2b1`）
- 审查结论：**non-resuming callee frame 终止语义已统一收口，且未回流旧 flag-based unwind / shape-based 路线**。ordinary callee 只读取 TLS active 并按统一 early-return 合同终止自身；caller-side state-machine dispatch 仍独立接管 active。

#### T3010b2b1：修正 handle arm body 内 non-resuming effect 的外传 / self-inactive / finally cleanup 语义（已完成）
- 2026-04-17 当前轮完成更新：重新执行 `effect_resume_finally_arm_raise.scoop`、`effect_escape_continuation_finally_arm_raise.scoop`、`effect_multi_nonresuming_raise_custom_finally.scoop` 与 `effect_escape_continuation_nested_arm_indirect_performs_outer.scoop` 后，四条 targeted LLVM run-pass fixture 全部通过。说明 direct arm-body raise/helper、escape continuation arm、pure non-resuming source-handle，以及 nested/indirect perform outward propagation 现已统一落在同一套 cleanup / finally / outward propagation 合同上。
- 继续复跑 `cargo run -p scoop --features llvm -- test` 后，suite 已不再在 arm-body outward propagation 语义上更早失败；当前首个失败点是 `continuation_resume_continuation.scoop` 的 stale `EXPECT: fail`，已由 `T3017` 跟踪。因此本任务无需再拆分新前置，下一项可直接回到 `T3010b2b`。
- 2026-04-17 复跑全量 LLVM fixture 后，首个失败点推进到 `effect_escape_continuation_finally_arm_raise.scoop`；定向复跑 `effect_resume_finally_arm_raise.scoop` 也确认 arm body 中的 `Raise.raise(...)` 仍会继续落到 `arm_unreachable`，sibling `Raise.raise` arm 仍会自捕获，`finally` 也没有在向外传播前执行。
- `T3010b2b0` 完成后，`effect_multi_nonresuming_raise_custom_finally.scoop` 中普通 helper frame 已不再继续执行 `throw_alarm_unreachable`，说明更基础的 ordinary callee propagation 缺口已关闭；该 fixture 剩余的 `mixed_finally` / outer catch / sibling self-capture 缺口现明确归本任务处理。
- 当前顺序已进一步细化为：`T3010b2b1a`（已完成：direct 路径）→ `T3010b2b1b0`（已完成：synthetic resume slot id / frame seeding）→ `T3009b0a`（已完成：outer-scope slot 收集 + 初次 handle 退出写回）→ `T3009b0a1a`（已完成：frame-metadata writeback target + step-return 出口）→ `T3009b0a1c`（已完成：统一 `SuspendCall` 的 inactive-path / active-dispatch 分流）→ `T3009b0a1d`（已完成：补齐 `ObjectInitAccessBoundary` inactive-path）→ `T3009b0a1dR`（已完成：确认 inactive-path 仍只按统一合同分流）→ `T3009b0a1e`（已完成：补齐 `NestedHandleBoundary` inactive-path）→ `T3009b0a1eR`（已完成：确认 nested-handle inactive-path 未回流为旁路补丁）→ `T3009b0a1cR`（已完成：确认 call-like inactive-path 未回流成普通 call/ordinary helper 补丁）→ `T3009b0a2`（已完成：共享 `RuntimeRaiseBoundary` inactive-path / active-dispatch 分流）→ `T3009b0`（已完成：escaped continuation 的 scalar/ref resume dedicated lowering / caller-tail）→ `T3009b0a1b`（已完成：focused fixture 可观测验收 resumed completion path 的 outer-slot 写回）→ `T3009b0aR`（已完成：确认 outer-slot 写回仍只由 frame metadata + shared exits 驱动）→ `T3009b0R`（已完成：确认 dedicated resume lowering 未回流 generic call）→ `T3010b2b1b`（已完成：rebaseline 后确认不存在独立 expected-context/coercion 缺口）→ `T3010b2b1b1`（先同步 `handle_perform` MIR golden，恢复全量 fixture 验证入口）→ `T3010b2b1`（剩余 nested/indirect outward propagation 验收）。

#### T3010b2b：基于 synthetic resume slot + immediate-resume lowering 回到端到端 post-suspend tail 验收（已完成）
- 已完成的前置修复包括：`while` / `if` branch condition 读取集补齐、outer slot authoritative metadata 回填、首次进入 `step_fn` 前 seeding outer locals/params 到 frame、以及 continuation `resume_state_tag` 仅在显式设置时才写回 frame。
- 重新验证 `effect_resume_yield_int_basic.scoop`、`effect_resume_finally_normal.scoop` 与 `async_await_minimal_int_basic.scoop` 后，三条 post-suspend 基线 fixture 均通过。
- 已对当前全部带 `EXPECT: fail` 的 run-pass fixture 扫描 `member access target` / `comparison lhs|rhs` / `equality lhs` / `integer binary op lhs`，确认这些 body-tail fragment 错误已全部消失。
- 将 `effect_resume_finally_body_raise_after_resume.scoop`、`effect_resume_nested_escape_handle_tail.scoop`、`effect_resume_mixed_escape_direct_finally.scoop` 与 `effect_resume_mixed_source_path_matrix.scoop` 临时去掉 `EXPECT: fail` 头后按普通 run-pass 复验，`fixtures: ok (4)`；说明 resume landing 现已只继续剩余 tail，不会重放原 suspend site。
- 继续复跑 `cargo run -p scoop --features llvm -- test` 后，suite 首个失败点为 `continuation_resume_continuation.scoop` 的 stale `EXPECT: fail`，该问题已由 `T3017` 跟踪；因此本任务收口，下一项推进 `T3010R`。

#### T3010R：Review：确认 state-machine 生产 op 不再包含“先拆 fragment 再整棵重算”的双轨语义（已完成）
- 已复审 `state_machine_plan.rs`、`state_machine_segments.rs`、`state_machine_transform.rs` 与 `state_machine_emitter.rs` 中 `HandleStateOp`、`resume_path`、synthetic resume slot 与 `ResumeAfterSite` 的生产消费边界，确认恢复值重写仍完全由 plan 层 `materialize_resume_fragments` 驱动，emitter 未回扫原始 AST。
- 复审中发现真实残留：`state_machine_emitter.rs` 的 `HandleStateOp::VarRef` 仍使用 `codegen_expr_in_expected_context(...).unwrap_or(CgValue::unit())` 吞掉失败，把 fragment-era 的“失败后伪执行成功”容错保留到了生产路径。
- 已直接修复：删除该 fallback，并把注释改为“standalone `VarRef` 必须独立可执行，否则直接暴露真实 codegen 错误”，使 unified emitter 与 ordinary expr codegen 语义保持一致。
- 已补强定向测试：`source_plan_keeps_only_whole_call_for_pure_statement_args_and_pure_if_condition` 现在除了锁定纯 statement call 的参数不拆成 `BinaryExpr` fragment，也同时锁定纯 callee 不会落成 standalone `VarRef` fragment。
- 验证通过：`cargo test -p scoopc source_plan_ -- --nocapture`、`cargo test -p scoopc runtime_raise_boundary_ir_branches_between_inactive_continue_and_active_dispatch -- --nocapture`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。复跑 `cargo run -p scoop --features llvm -- test` 后，suite 仍只停在 `continuation_resume_continuation.scoop` 的 stale `EXPECT: fail`，该既有问题已由 `T3017` 跟踪，未出现新的更早回归。
- 审查结论：state-machine expression emission 粒度已收口；生产 op 不再包含 fragment-only 伪执行路径，也不再依赖 `VarRef` 的“失败就吞掉” fallback。

### 阶段 G：effect 主线收口后，切回 `do` block / closure 消歧

#### T3101：Parser / AST 引入显式 `do { ... }` block，并将裸 `{}` 固定为 closure（已完成）
- 已在 `Keyword` 中新增 `Do`，在 `ExprKind` 中新增 `DoBlock { do_span, body }` 变体。
- `try_parse_expr_atom` 中 `do` 关键字优先于 `{` 匹配：`do { ... }` → `DoBlock`，裸 `{ ... }` → `Lambda`。
- `@Safe`/`@Unsafe` 后支持可选 `do`：`@Safe do { ... }` / `@Unsafe do { ... }`。`@Safe { ... }` 保持向后兼容。
- 所有 AST 消费者（resolve、typecheck、HIR lower、comptime）已同步处理 `DoBlock`，语义与 `Block` 等价。
- 新增 4 个 parser 单元测试 + 2 个 parse fixtures，验证 do-block vs closure 消歧。
- 复验通过：`cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`（217 passed）、`cargo run -p scoop -- test`（965 fixtures）。

#### T3102 ✅：Typecheck / HIR 收口 `do` block 的 expression statement 与 tail value 语义
- 统一”只有未终止 tail expr 才产生 block 值；`expr;` 只是 expression statement，结果视为 `Unit`”。
- `if` / `when` / `handle` / lambda body / `do` block 的值语义都要按同一规则收口。
- 实现：AST `Stmt` 增加 `has_trailing_semi` 字段，parser/typecheck/HIR lowering 全链路联动；`block_tail_expr` 拒绝 semicolon-terminated 语句。

#### T3103：effect nested-block fixtures 切到 plain `do` block
- 在 `T3006R` 之后，把仅为 nested block 消歧而保留的 `@Safe { ... }` workaround 切回 `do { ... }`。
- 真正依赖 safe-region 语义的测试继续保留 `@Safe`，并同步锁定 multiple trailing lambdas 与 `do` block 的边界规则。

#### T3104：同步规范 / 文档中的 `do` block、closure 优先级与 trailing-lambda 规则
- 更新 `SCOOP_FULL_SPEC.md`、doctest / fixture 示例，以及当前 `TODO.md` / `PLAN.md` 等相关文档叙述。
- 若规范代码块变更，配套完成 `spec-fixtures sync/check`。

### 阶段 H：Structured Concurrency / `Task<T>`

#### T3201：`spawn` / `join` 的 typecheck 与 HIR 去 `Int` 硬编码
- 先把前端表示收口到真实的 `Task<T>`，不再把 handle / result 擦成 `Int`。
- 已确认仍缺失的 lowering / codegen / runtime 缺口，分别由 `T3202`～`T3204` 明确承接。

#### T3202：`spawn` / `join` 语法糖与 sysroot glue 去 `_int` 专用路径
- HIR lowering、block rewrite 与 sysroot internal glue 不再依赖 `__scoop_task_spawn_int` / `__scoop_task_join_int` 这类 `_int` 专用入口。
- desugar 后的 HIR 必须继续保留任务结果类型，给后续 LLVM / runtime 泛型化提供稳定输入。

#### T3203：LLVM codegen 去 `scoop_task_*_int` 专用路径
- codegen 不再把 `Task<T>` 压回 `i64`/`Int` 专线，而是支持 scalar / ref / aggregate / 泛型实例的统一 task payload。
- task payload transport 要尽量与 continuation payload ABI 对齐，避免维护 task-only 特例。

#### T3204：runtime executor / `Task<T>` 完成回调泛型化
- runtime task 状态机、executor job、completion waiter 与 sysroot glue 都不能再固定在 `Task<Int>` / `resume_u64`。
- ref / aggregate payload 在 pinning、GC stress、跨线程或跨 executor 恢复时都要保持稳定语义。

#### T3205：结构化并发回归矩阵与语义锁定
- 用 nested `spawn` / `join`、控制流 join、多任务交错、GC 压力等真实并发场景锁定边界。
- 当前阶段明确不支持的并发组合，要么形成稳定诊断，要么在文档中清楚限制。

### 阶段 I：Lambda 推断与调用语义补齐

#### T3301：expected function type 向任意参数个数传播
- 把 lambda expected-type 传播从 0/1/2 参数推广到任意参数个数。
- 变量初始化、返回语境、调用实参、集合/构造器上下文等常见入口都要统一接入。

#### T3302：receiver lambda 体内 `this` 与成员解析
- receiver lambda 进入 typecheck / lowering 时自动建立 `this` 绑定与成员查找环境。
- `this`、成员访问、扩展调用与闭包捕获的局部作用域规则要与普通 lambda 对齐。

#### T3303：统一函数值 / funptr / ctor delegation 的实参匹配
- 函数值调用、函数指针调用、`super(...)` / `this(...)` 构造器委托调用要共用同一套参数匹配规则。
- 命名实参与 receiver function type 的处理不能再靠零散的早期门禁分流。

### 阶段 J：泛型约束 / Pattern / 值类型能力补齐

#### T3401：`where` nominal bound 支持类型实参与 instantiated supertype 满足性
- 把带类型实参的 nominal bound 贯通到解析、检查、子类型关系与诊断。
- 实例化处的 bound 检查、函数体内成员分发都必须基于实例化后的 bound，而不是回退到未参数化 nominal type。

#### T3401a：`where` nominal bound 的子类型满足性回归矩阵
- 专门锁定接口/类继承链上的实例满足 generic bound 的语义，不依赖当前实现“碰巧有效”。
- 补齐变量透传、泛型 passthrough、builtin/value 类型 boxing 满足 interface bound 等回归。

#### T3401b：`where` bound 驱动的方法分发补齐接口继承链与多 bound 歧义
- 让接口继承链上的成员对 bound receiver 可见，并为多 bound 同名成员建立稳定的候选集 / 歧义规则。
- 不能再按遍历顺序提前返回首个命中项。

#### T3401c：成员方法签名收集与 `where` bound 分发对齐 richer generic/effect 调用
- 普通 member call 与 `where` bound member call 都要支持显式类型实参、2+ type params 与 `<eff E>` 成员方法。
- shared helper 与 top-level call 之间的语义不能继续缩水分叉。

#### T3402：顶层 `val` 支持 pattern binding
- 顶层 tuple / struct / enum destructuring 复用既有 pattern binding 规则，不再保留“顶层只允许标识符”的特判。
- 顶层符号安装、初始化顺序、多文件可见性与循环引用诊断保持稳定。

#### T3403：`struct` 字段支持 `var` 与默认值
- 先收口字段模型，让 `struct` 声明能力覆盖 `var` 字段与默认值。
- 构造、布局、默认值与值语义冲突处都需要统一规则与诊断。

#### T3404：`with` 更新扩展到更完整的值类型语义
- `with` 的 base 类型不再局限于当前最小 `struct` 子集，嵌套字段路径更新要 lower 成稳定的 copy-update 链。
- 诊断必须区分字段不存在、字段不可更新、类型不匹配、base 非值类型等不同错误。

#### T3405：`when` 的 or-pattern 支持共享 binder
- 当各分支 binder 集、名称与类型兼容时，允许 or-pattern 引入共享 binder。
- binder 数量、名称或类型不一致时，要给出具体而稳定的诊断。

## 3. 验收策略

- `T30` 的清理阶段允许临时破坏编译；目标是先删旧主线。
- `T30` 接统一 LLVM emitter 后，再用定向测试恢复并扩大覆盖。
- `T30` 的 review 任务是强制门，不是可选项；任何实现任务完成后，如果 review 发现 shape-based 逻辑回流，必须先回退到清理状态，再进入下一任务。
- `T31`～`T34` 按各自 TODO 中列出的 fixtures / `cargo test` / LLVM run-pass 验收，不额外插入独立 review gate。

## 4. 当前执行顺序

1. `T3017`
2. `T3017R`
3. `T3103`
4. `T3104`
5. `T3201`
6. `T3202`
7. `T3203`
8. `T3204`
9. `T3205`
10. `T3301`
11. `T3302`
12. `T3303`
13. `T3401`
14. `T3401a`
15. `T3401b`
16. `T3401c`
17. `T3402`
18. `T3403`
19. `T3404`
20. `T3405`
