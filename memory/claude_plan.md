# 本轮执行计划（摘要版）

说明：这里记录的是可执行的思路摘要与步骤计划，用于跟踪本轮任务进展，不展开内部推理细节。

## 目标

完成 `TODO.md` 中第一个未完成任务；如果在执行前发现最新提交提到的既有问题，先修复这些问题；完成后更新计划与任务状态，运行相关测试，并提交 git commit，然后停止。

## 预定步骤

1. 检查最新一次 git commit 的提交信息与变更，确认是否提到尚未修复的问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务上下文、依赖与可能已有拆分计划。
4. 如果首个未完成任务过大或存在前置缺口：
   - 拆分为更小子任务；
   - 更新 `PLAN.md`；
   - 调整 `TODO.md` 中的任务顺序与依赖；
   - 本轮只执行拆分后的第一个子任务。
5. 阅读相关代码、测试和规范材料，确认正确实现路径，避免引入规避性方案。
6. 实现任务所需改动，并在关键步骤完成后同步更新本文件。
7. 运行与改动相关的测试，并补充必要测试；同时确保 `cargo clippy --all-targets -- -D warnings` 不产生告警（若本轮改动范围允许，需要验证）。
8. 更新 `TODO.md` 与 `PLAN.md`，记录完成状态或阻塞原因。
9. 检查工作区差异，整理提交内容，使用清晰的提交信息创建 commit。
10. 停止，不继续处理下一个任务。

## 进度记录

- 已创建本轮计划文件。
- 已检查最新一次 git commit：`[T3010b2b0a] Lock hidden-suspend caller coverage`。提交信息本身未额外声明新的未入账历史问题；`PLAN.md` / `TODO.md` 中记录的后续 blocker 仍为已排队的 `T3010b2b1`。
- 已读取 `TODO.md` / `PLAN.md`，确认当前第一个未完成任务是 `T3010b2b0R`：复审 ordinary callee frame 在 non-resuming perform / hidden-suspend 返回 active 后的终止语义，确认未回流旧 flag-based unwind / shape-based 路线。
- 已更新 `TODO.md` / `PLAN.md`：`T3010b2b0R` 已标记完成，后续首个未完成任务变为 `T3010b2b1`。
- 已检查工作区差异：当前仅保留 `PLAN.md`、`TODO.md` 与 `memory/claude_plan.md` 三处本轮更新。
- 下一步：
  1. 创建本轮 git commit，然后停止。

## 已完成的复审与验证

- 已审阅 `crates/scoopc/src/llvm/codegen/effect/mod.rs`：
  - ordinary-frame propagation 只通过 `emit_ordinary_non_resuming_effect_exit` 与 `emit_ordinary_call_effect_propagation_check` 两个 helper 发射；
  - `emit_effect_propagation_return` 只负责“默认返回值/return_bb”控制流，不会清掉 TLS active；
  - direct non-resuming 路径只从 `codegen_perform_expr` 与 `codegen_cast_as_expr` 的 runtime raise fail-path 接入。
- 已审阅 `crates/scoopc/src/llvm/codegen/mod.rs`：
  - ordinary call propagation check 已接到 direct/vtable/itable/funptr/closure/operator/object property/object init 等调用面；
  - hidden-suspend object/property/class-init 路径继续沿统一 active-check 合同传播；
  - 未发现旧 `emit_effect_unwind_if_active`、`raise_target_stack` 或 callee/source shape 分流回流。
- 已审阅 `crates/scoopc/src/llvm/codegen/control_flow.rs`：
  - 未发现 effect 专用 CFG 分流或 active/clear/unwind 逻辑；只有局部变量元数据里的 `call_may_suspend` 赋值，与本任务无旧路径回流关系。
- 已审阅 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 的关键边界：
  - step function 生成前会暂存并清空 `current_fun_return_ty` / `return_context`，说明 ordinary-frame propagation helper 不会误闯统一 state-machine step/dispatch；
  - handle dispatch 仍通过 state machine 的 `is_active -> clear_active -> dispatch` 路径工作，caller-side 语义未被普通 callee 早退逻辑破坏。
- 已完成关键词检索：
  - `emit_effect_unwind_if_active` / `raise_target_stack` / `CalleeSuspend` / `scan_for_callee_suspend` / `suspendable` 在生产代码中无命中；
  - ordinary callee 路径没有使用 `declare_runtime_effect_clear` / `clear_active`，说明不会吞掉 active。
- 已完成定向验证：
  - `target/debug/scoop run tests/fixtures/run-pass/nothing_raise_in_helper_basic.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/effect_indirect_perform_nonresuming_call_chain.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/object_property_init_raise_helper_try_catch_basic.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/class_init_hidden_raise_helper_try_catch_basic.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/effect_handle_hidden_suspend_helper_object_property_basic.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/effect_handle_hidden_suspend_member_helper_basic.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/effect_handle_hidden_suspend_local_closure_helper_basic.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/object_init_raise_try_catch_basic.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/class_init_raise_cleanup_property_init_gc_basic.scoop`
  - `cargo test -p scoopc segment_dump_classifies_hidden_suspend_ -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已复跑全量 LLVM fixture：`target/debug/scoop test` 仍首先失败于已知后续 blocker `tests/fixtures/run-pass/effect_escape_continuation_finally_arm_raise.scoop`，与 `T3010b2b1` 一致，未把失败点拉回到本轮复审范围之前。
