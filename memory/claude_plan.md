# T3005 执行计划

## 任务描述
将统一 state-machine LLVM lowering 接回 effect codegen 主入口，替换 `codegen_perform_expr` / `codegen_handle_expr` 的占位错误。同时移除 `mod.rs` 中 flag-based unwind 调用点。

## 步骤

### 1. 实现 codegen_perform_expr（替换占位错误）
- 查找 op_tag（`effect_op_tag(&op.fqn)`）
- 评估参数得到 payload
- 写 op_tag + payload 到 TLS perform slot（复用 emit_perform_op 的逻辑模式）
- 设置 active flag（`scoop_effect_set_active()`）
- 返回 expected type 的 default_value

### 2. 实现 emit_raise_runtime_error_variant（替换占位错误）
- 写 Raise.raise op_tag 到 TLS perform slot
- 设置 active flag
- 不终止 basic block（caller 处理后续控制流）

### 3. 移除 mod.rs 中 7 处 emit_effect_unwind_if_active 调用
- line ~8788-8793：codegen_top_level_fun_call 中的 fun_ty_effects_is_pure 门控 + emit_effect_unwind_if_active
- line ~9059：vtable call
- line ~9305：itable call
- line ~9750：funptr/closure call
- line ~9921：closure call
- line ~12502：object init
- line ~12725：object init

### 4. 移除 raise_target_stack 字段
- 移除字段声明（mod.rs:188）
- 移除初始化（mod.rs:344）

### 5. 移除 effect/mod.rs 中 flag-based unwind 方法定义
- emit_effect_is_active_i1
- emit_effect_unwind_if_active
- fun_ty_effects_is_pure

### 6. 验证 & 提交
- cargo check -p scoopc
- cargo clippy --all-targets -- -D warnings
- cargo test --all
- 更新 TODO.md / PLAN.md
- git commit

## 执行状态
- [ ] 实现 codegen_perform_expr
- [ ] 实现 emit_raise_runtime_error_variant
- [ ] 移除 7 处 flag-based unwind 调用
- [ ] 移除 raise_target_stack
- [ ] 移除 flag-based unwind 方法定义
- [ ] 验证通过
- [ ] 提交
