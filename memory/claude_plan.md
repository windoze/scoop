# Claude Plan

## 目标
- 按 `TODO.md` 的顺序完成第一个未完成任务，并在完成后停止。

## 约束说明
- 这里记录的是可执行计划、检查项与进度摘要，不包含内部推理细节。
- 在执行过程中，如计划变更、发现阻塞问题、完成关键步骤，会及时更新本文件。

## 初始执行计划
1. 检查最新提交信息，确认是否提到需要优先修复的既有问题；如果有，先处理该问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 评估该任务是否过大：
   - 如果可直接完成，继续实现。
   - 如果过大，先细化到 `PLAN.md` 和 `TODO.md`，然后只执行拆分后的第一个子任务。
4. 在开始实现前，阅读相关代码、测试和规范上下文，确认现状与依赖。
5. 实现该任务，避免引入变通方案或偏离规范的实现。
6. 运行相关测试；如发现既有缺陷、回归、规范不匹配或阻塞问题，优先修复或将其作为前置任务插入 `TODO.md`。
7. 更新 `TODO.md` 和 `PLAN.md`，标记本次完成情况或阻塞依赖调整。
8. 按仓库约定创建一次 git 提交，然后停止。

## 进度
- 已写入初始计划。
- 已检查最新提交：`[T5002a] Record state-machine flush-back completion`，提交信息未提到需要先修复的既有问题。
- 已阅读 `TODO.md` / `PLAN.md`，当前首个未完成任务为 `T5002aR`（review：确认 state-machine flush-back 真正取代了 block-local write-through 偶然正确性）。

## 当前任务：T5002aR

### 目标
- 确认 flush-back 合同确实覆盖 `suspend / return / arm-exit / cleanup` 四类边界。
- 确认 `outer mutable local`、`arm binder`、`capture local`、`escape continuation binder` 共享同一持久化合同。
- 在三项 GC 环境全开条件下重跑相关 direct/indirect fixtures，确认 `T5002b` 不再被该问题阻塞。

### 执行步骤
1. 阅读与 `T5002a` 直接相关的 codegen / 测试实现，确认 flush-back 入口点与覆盖范围。
2. 运行相关 LLVM / 单元 / fixture 回归，优先覆盖 direct/indirect mixed 路径与 cleanup 相关窗口。
3. 如果发现既有缺陷或规范不匹配，立即修复；若无法在本轮直接修复，则按要求先更新 `TODO.md` / `PLAN.md` 记录前置任务并停止。
4. 如果 review 通过，更新 `TODO.md` / `PLAN.md`，将 `T5002aR` 标记完成并记录验证结果。
5. 提交本轮变更并停止。

### 当前结论
- `write_back_outer_scope_frame_slots(...)` 的调用点已覆盖：
  - step-function return
  - `ReturnHandle`
  - `ReturnFromFunction`
  - `Suspend`
  - `ArmReturnHandle`
  - `ArmResumeMatchedSite`
  - `ArmMaterializeContinuation`
  - 外层 handle `handle_propagate` / `handle_done`
- `populate_frame_slots_in_env(...)`、arm binder materialization、escape continuation binder materialization、capture local restore 都采用“entry alloca exec home + frame slot backing”合同；对 mutable local，赋值路径会同时写回 `frame_backing_ptr`。
- 暂未发现新的既有 blocker，也未发现需要在 `T5002b` 前追加的前置任务。

### 已完成验证
- LLVM 回归：
  - `llvm::codegen::effect::state_machine_emitter::tests::escaped_continuation_resume_ir_records_outer_slot_storage_and_writeback`
  - `llvm::codegen::effect::state_machine_emitter::tests::state_machine_frame_slots_materialize_stable_exec_local_homes`
  - `llvm::codegen::effect::state_machine_emitter::tests::cleanup_enter_ir_checks_cleanup_flag_before_reentering_finally`
  - `llvm::codegen::effect::state_machine_emitter::tests::cleanup_propagate_ir_restores_propagating_state_after_shared_finally_exit`
  - `llvm::codegen::effect::state_machine_emitter::tests::escape_arm_gc_roots_use_frame_slot_or_entry_spill_contract`
- 默认环境 fixture：
  - `tests/fixtures/run-pass/effect_escape_continuation_outer_mutable_writeback_basic.scoop`
  - `tests/fixtures/run-pass/continuation_resume_enum.scoop`
  - `tests/fixtures/run-pass/effect_multi_escape_direct_indirect_while.scoop`
  - `tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  - `tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_binder_string_use.scoop`
  - `tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_locals.scoop`
- GC env 全开 fixture：
  - `tests/fixtures/run-pass/effect_escape_continuation_outer_mutable_writeback_basic.scoop`
  - `tests/fixtures/run-pass/continuation_resume_enum.scoop`
  - `tests/fixtures/run-pass/effect_multi_escape_direct_indirect_while.scoop`
  - `tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  - `tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_binder_string_use.scoop`
  - `tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_locals.scoop`
- lint：`cargo clippy --all-targets -- -D warnings`

### 收尾状态
- 已更新 `TODO.md`：`T5002aR` 标记为完成，并补充 review 完成记录。
- 已更新 `PLAN.md`：记录 `T5002aR` 的复核结论与进入 `T5002b` 的前置状态。
- 下一步：提交本轮变更并停止。
