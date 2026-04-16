# 当前执行计划：T3007 — 删除统一主线接管后剩余的 legacy effect codegen 死代码

## 状态：已完成

## 已执行步骤

1. [x] 移除 `mod.rs:229` 上 `EffectOpTagState` 的过时 `#[allow(dead_code)]`
2. [x] 删除 `runtime_abi.rs` 中 4 个 dead ABI 声明（set_active_with_trace, handler_stack_set_active, handler_stack_unwind_to_tag, handler_stack_swap_top）
3. [x] 删除对应的 `runtime_symbols.rs` 常量
4. [x] 保留 `thread_spawn_join_resume_u64`（被 mod.rs:8166 消费）并移除其 dead_code 注解
5. [x] 清理 `effect/mod.rs`：移除 4 个未使用的 re-export，更新 skeleton 模块注释
6. [x] 删除 `state_machine_emitter.rs` 中未使用的 `STATE_TAG_SUSPENDED` 常量
7. [x] 清理 emitter 中过时的 T-number 注释和 stale T3005 TODO
8. [x] 验证 cargo check + clippy + test（全部通过）
9. [x] 更新 TODO.md / PLAN.md 并提交
