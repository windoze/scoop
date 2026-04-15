# 当前执行计划：T3005R（已完成）

## 任务
Review：确认 effect codegen 主入口只接统一 state-machine lowering，无 flag-based unwind 残留。

## 审查结论

effect codegen 主入口没有旧 fallback / 双轨 / flag-based unwind 残留：
- `codegen_perform_expr` 写 TLS perform slot + set active + 返回 default value
- `codegen_handle_expr` 直接委托 `codegen_handle_expr_via_state_machine`
- `emit_raise_runtime_error_variant` 写 Raise.raise op_tag + set active
- `emit_effect_unwind_if_active`、`raise_target_stack`、`emit_effect_is_active_i1`、`fun_ty_effects_is_pure` 在 codegen 目录零命中
- `expr.rs` Perform/Handle 入口单路径透传

## 修复
修复了 3 处引用已删除 flag-based unwinding 的过时注释：
- `mod.rs:175` — current_fun_return_ty 字段文档
- `mod.rs:8769` — extern call leave_native 注释
- `mod.rs:12573` — object init return type 注释

## 下一步
T3006：用定向测试补齐统一 LLVM lowering 覆盖
