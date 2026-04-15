# 本轮执行计划

## 当前任务：T3004c — Suspend/resume 机制与 handler arm dispatch [已完成]

### 实现摘要

1. **Suspend terminator**: 写入 resume_state 到 state_tag → `scoop_continuation_alloc(state, step_fn)` → 存 continuation 到 frame resume_gc_ref slot → `scoop_effect_set_active()` → return void。

2. **Handle entry dispatch loop**: `is_active()` check → `read_op_tag()` → `clear()` → switch on op_tag → arm block (设置 state_tag 到 arm entry state, 调用 step_fn) → loop back to check。Handler stack 生命周期由栈上 ScoopEffectHandlerFrame + push/pop 管理。

3. **Perform op emission**: 求值 perform 表达式 → 按 CgTy 分流写入 TLS perform slot (`write_u64` / `write_u64_with_gc_ref`)。

4. **SuspendCall/ObjectInitAccessBoundary/RuntimeRaiseBoundary**: 委托 codegen_expr 求值。

5. **ResumeAfterSite**: no-op — step_fn entry 已将 resume payload 写入 frame。

6. **ExecuteArmBody**: 从 perform slot 读 binder → frame slot + env → resume/continuation 绑定 → 恢复 captures → 求值 arm body。

7. **Arm terminators**:
   - ArmReturnHandle → store result + HANDLE_RETURNED sentinel
   - ArmResumeMatchedSite → write payload to continuation (field 6/7) + `scoop_continuation_resume(k)`
   - ArmMaterializeContinuation → store result + HANDLE_RETURNED sentinel

8. **Dead code cleanup**: 移除 8 个 `#[allow(dead_code)]` 从已消费的 runtime ABI + `effect_op_tag`。

### 验证结果
- cargo check -p scoopc：零 warning
- cargo clippy --all-targets -- -D warnings：通过
- cargo test --all：213 passed
