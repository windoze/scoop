# 本轮执行计划

## 约束说明

- 按要求，本轮只处理 `TODO.md` 中第一个未完成任务，完成后即停止。

## 当前任务：T3003b [已完成]

### 任务描述

暴露 `handle -> unified lowering contract` 的生产 builder 与 crate 内访问面。

### 完成内容

1. [DONE] 在 `state_machine_transform.rs` 中为 `UnifiedHandleStateMachine` 和全部子结构添加 pub(crate) 只读访问器
2. [DONE] 在 `state_machine_transform.rs` 中定义 `UnifiedHandleLoweringContract` 并提供便利委托访问接口
3. [DONE] 升级 `build_handle_state_machine_plan` 为完整 pipeline builder `build_unified_lowering_contract`
4. [DONE] 更新 `effect/mod.rs` 的 re-export
5. [DONE] 添加定向测试 `unified_lowering_contract_provides_complete_read_access`
6. [DONE] cargo check / clippy / test 全绿（213 passed）
7. [DONE] 更新 TODO.md / PLAN.md，提交 commit

### 下一步

下一个未完成任务为 **T3003R** — Review：确认 LLVM lowering 输入面只剩 state machine。
