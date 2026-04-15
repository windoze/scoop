# 本轮执行计划

## 当前任务：T3004b — 状态 op 发射与基本 terminator [已完成]

### 实现摘要

1. 重构 `emit_effect_step_function`：将 step function body 拆分为 `emit_step_function_body`，保存/恢复完整 codegen 上下文（env、return_context、current_fun_return_ty、loop_context_stack）。

2. 实现 `emit_state_ops`：遍历每个 state 的 ops 列表，按 HandleStateOp 变体分派：
   - BindLocal → 求值初始化 → frame GEP + store + env 注册
   - ReadLocal → frame GEP + load + env 注册
   - Literal/VarRef/Expr/Call 等 → 委托 codegen_expr_in_expected_context
   - Assign → 委托 codegen_assign_stmt
   - Return → 求值返回值并追踪为 last_value
   - Suspend/Arm 等 T3004c/d op → placeholder return void

3. 实现 `emit_state_terminator`：
   - Goto → unconditional branch
   - Branch → eval condition + conditional branch
   - ReturnHandle → store result to frame + sentinel + return void
   - ReturnFromFunction → store value + sentinel + return void
   - T3004c/d terminators → placeholder return void

4. 实现结果传递：
   - `store_result_to_frame`：CgTy 分流 → resume_word / resume_gc_ref
   - `read_result_from_frame`：从 frame 读取结果
   - `narrow_u64_word_to_cg_value`：u64 → CgTy 窄化
   - 更新 handle 入口：从 frame 读取真实结果

### 验证结果
- cargo check -p scoopc：零 warning
- cargo clippy --all-targets -- -D warnings：通过
- cargo test --all：213 passed
