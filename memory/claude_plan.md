# 本轮执行计划

## 约束说明

- 按要求，本轮只处理 `TODO.md` 中第一个未完成任务，完成后即停止。

## 当前任务：T3004 — 实现 heap-allocated full state machine 的 LLVM lowering 主体

### 分析

T3004 过于复杂，需要拆分为子任务：

1. **T3004a**: Frame struct LLVM 类型生成 + step function 骨架 + handle 表达式入口
2. **T3004b**: 状态 op 发射 + 基本 terminator（Goto/Branch/Return）
3. **T3004c**: Suspend/resume 机制 + handler arm dispatch
4. **T3004d**: Cleanup scope、嵌套 handle、边界完善

### 架构设计

采用 continuation-based state machine 模型：

- **Frame**: 堆分配结构体 = system fields (state_tag i32, resume_word i64, resume_gc_ref ptr, cleanup_flag i32, one_shot_flag i32) + user slots
- **Step function**: `(ptr state, i64 resume_word, ptr resume_gc_ref) -> void`，按 state_tag switch 派发
- **Handle 入口**: 分配 frame → push handler stack → 调用 step_fn → 检查 active → dispatch to arm
- **Suspend**: perform 时保存 state_tag → alloc continuation → set active → return from step_fn
- **Resume**: handler arm 调用 continuation_resume → 重入 step_fn

### 本轮执行：T3004a

创建 `state_machine_emitter.rs`：
1. `emit_effect_frame_llvm_type()` — 从 UnifiedFrameSchema 生成 LLVM struct type
2. `emit_effect_step_function()` — 生成含 state dispatch 的 step function（各 state block 暂时 return void）
3. 更新 `codegen_handle_expr()` — build contract → alloc frame → init → call step_fn → return result

### 进展
- [DONE] 分析代码库现状
- [DONE] 确定子任务分解方案
- [TODO] 更新 TODO.md / PLAN.md
- [TODO] 实现 T3004a
- [TODO] 验证编译和测试通过
- [TODO] 提交
