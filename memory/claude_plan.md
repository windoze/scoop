# 执行计划与进度记录

## 说明

本文件记录本次执行的高层计划、关键决策、阻塞原因与完成进度，便于审计与续接。  
不记录逐字的内部思维链路，但会持续更新可验证的执行步骤、发现与结论。

## 当前任务：T3002 [已完成]

精确化 effect codegen 的 dead_code 边界，把统一骨架从 blanket allow 中解放。

### 分析结论

#### 1. `effect/mod.rs` - unified_state_machine_skeleton（line 11-18）
- 整个模块被 `#[allow(dead_code)]` 包裹
- 内含三个 include 文件，所有类型被 blanket dead_code 遮蔽
- `state_machine_plan.rs` 底部有 `impl MainCodegen` 的 `build_handle_state_machine_plan` 方法

#### 2. `runtime_abi.rs` - 两个 dead_code 边界
- Line 15: `llvm_effect_handler_frame_type` 单方法（已精确）
- Line 1256: blanket `#[allow(dead_code)]` impl 块，~30 个 ABI 声明

#### 3. 已被 sysroot intrinsic 生产路径消费的 ABI（9 个）
- `declare_runtime_effect_is_active`, `set_active`, `clear`
- `perform_slot_write_u64`, `write_u64_2`
- `perform_slot_read_op_tag`, `read_len_words`, `read_u64`, `read_u64_at`

#### 4. 仅供统一 lowering 的 dead ABI（12 个）
- handler_stack_push/pop/set_active/unwind_to_tag/swap_top
- set_active_with_trace, continuation_alloc/resume, llvm_continuation_struct_type
- thread_spawn_join_resume_u64, perform_slot_write_u64_with_gc_ref, perform_slot_read_gc_ref

### 执行步骤

1. **重构 skeleton 模块结构**：将核心类型从 `pub(super)` 改为 `pub(crate)`，从模块中 re-export，使 T3003+ 可直接引用。
2. **精确化 runtime_abi.rs**：拆分 blanket impl 块，9 个已被消费的 ABI 移出 dead_code 保护；12 个 dead ABI 保留独立 `#[allow(dead_code)]` 注解。
3. **标记 flag-based unwind**：为 `emit_effect_is_active_i1`、`emit_effect_unwind_if_active`、`fun_ty_effects_is_pure` 添加非主线标记。
4. **验证**：cargo check / clippy / test 全通过。

### 进度
- [x] 代码分析完成
- [x] Step 1: 核心类型 pub(crate) + re-export
- [x] Step 2: 精确化 runtime_abi.rs
- [x] Step 3: 标记 flag-based unwind 非主线
- [x] Step 4: 验证（零 warning / clippy / test all pass）
- [x] 更新 TODO.md
- [ ] 提交
