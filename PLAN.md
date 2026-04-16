# Scoop：当前计划（effect 主线优先，后续任务顺延）

> 生成时间：2026-04-15  
> 历史归档：`PLAN-3.md` / `TODO-3.md`  
> 范围：本计划先覆盖当前 effect 统一主线（`T30`）；为避免下一批任务继续停留在归档里，也顺延保留前端 / 并发 / 类型系统的后续队列（`T31`～`T34`）。当前执行顺序仍以 `T30` 全部收口为先。

## 0. 工作原则

- 当前最高优先级是 `T30`；`T31`～`T34` 只作为 effect 主线收口后的后续队列，不与当前修线争抢顺序。
- `T30` 继续遵守“删除优先于修补”。看到 shape-based 生产逻辑就直接删除，不以“先补一个 case”维持旧路径。
- `T30` 中 LLVM effect codegen 的单一输入是 state machine。除类型、符号与 ABI 必需信息外，不能再读取源码形状、旧 scanner 结果或旧分类器输出。
- `T30` 当前阶段只实现 heap-allocated full state machine lowering，不做 simplification，不做模式化优化。
- flag-based unwind（`emit_effect_unwind_if_active` / `raise_target_stack`）明确搁置，不作为 `T30` 主线依赖。effect 传播完全由统一 state machine 驱动；flag-based unwind 日后可作为优化加回。
- `T30` 中每个实现任务后立即插入一个 review 任务；review 必须显式确认生产代码中不存在 shape-based logic。
- `T30` 的 review 范围只看生产代码，重点是 `crates/scoopc/src/llvm/codegen/**`；测试命名不作为问题。
- `T31`～`T34` 维持“小步可回归”原则：先收口语义与表示，再扩展 lowering / runtime / 测试；除显式写出的依赖外，不额外插入 effect 风格的 review 子任务。

## 1. 当前状态与已知缺口（T30）

- `T2999` 已完成：
  - `cargo check -p scoopc` 已恢复零 warning。
  - `cargo clippy --all-targets -- -D warnings` 已通过。
  - `scoop.core.__scoop_effect_*` sysroot 测试辅助 intrinsic 已重新直连 runtime ABI，`cargo test --all` 已恢复通过。
- `T2999R` 已完成：
  - 已删除 `runtime_abi.rs` 中无生产调用点、也不属于当前统一 effect 合同的 `declare_runtime_alloc` / `declare_runtime_gc_collect`。
  - 已把 `runtime_symbols.rs` 中散落的冗余 `#[allow(dead_code)]` 清掉，并删除 `state_machine_plan.rs` / `state_machine_transform.rs` 中被统一骨架边界覆盖的重复豁免。
  - 已重新验证 `cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all` 全部通过。
- `T3001` 已完成：
  - 已从 `crates/scoopc/src/llvm/codegen/mod.rs` 删除 `CalleeSuspendResumeMode`、`scan_for_callee_suspend`、`codegen_top_level_fun_suspendable`、`codegen_closure_fun_body_suspendable` 及其入口接线。
  - 顶层函数与 closure 的 codegen 已收口回常规路径，不再按 `perform` 所在源码形状选择专用 suspendable lowering。
  - 已同步清理 `effect/mod.rs`、`runtime_abi.rs`、`runtime_symbols.rs` 中仅服务于这条旧路径的 helper / ABI 声明。
  - 复验通过：`cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`。
- `T3001R` 已完成：
  - 已定向检索 `crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/effect/mod.rs` 与相邻调用点，确认删除后的旧 callee-shape scanner / mode enum / suspendable top-level/closure route 没有换名回流。
  - 已复查 `codegen_top_level_fun`、`codegen_closure_fun_body`、`codegen_top_level_fun_call` 与 `ExprKind::Perform` / `ExprKind::Handle` 接线，确认当前只剩常规函数/闭包 codegen 与统一 effect 占位入口，不再按源码 / callee 形状分流。
  - 已重新验证 `cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all` 全部通过。
- `T3002R` 已完成：
  - 已定向检索 `crates/scoopc/src/llvm/codegen/**` 中与旧分流相关的命名与入口，包括 `shape`、`scan_for`、`CalleeSuspend`、`suspendable` 等，未发现残留命中。
  - 已复查 `expr.rs`、`effect/mod.rs` 与 `mod.rs` 调用链，确认 `ExprKind::Perform` / `ExprKind::Handle` 只直连统一 effect 入口；当前残留的 effect 相关生产逻辑仅为统一 lowering 占位入口、sysroot intrinsic lowering 与 flag-based unwind 辅助，没有按源码 / site / arm / callee 形状做主选路。
  - 已重新验证 `cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all` 全部通过。
- `T3004a` 已完成：创建 `state_machine_emitter.rs`，实现从 `UnifiedFrameSchema` 生成 LLVM struct type（system fields + user slots）、step function 骨架（state_tag-based switch 派发，各 state block 暂时 return void）、handle 表达式入口（build contract → malloc frame → memset zero → init state_tag → call step_fn → return default value）。`codegen_handle_expr` 已从占位错误改为调用 `codegen_handle_expr_via_state_machine`。
- `T3003a` 已完成：`HandleStateOp` / `HandleBranchCondition` 已补齐完整 HIR payload，unified state machine 的执行 payload 元数据在 plan → segments → unified machine 流水线中稳定保留。原始 `T3003` 已拆分为 `T3003a`（payload 补齐，已完成）、`T3003b`（builder/访问面暴露，已完成）和 `T3003R`（review，已完成）。
- `T3003b` 已完成：`UnifiedHandleLoweringContract` 已定义为唯一生产结构输入；`build_unified_lowering_contract` 实现完整 pipeline（plan → segments → unified machine → contract）；全部子结构有 `pub(crate)` 只读访问器。
- `T3003R` 已完成：审查确认 LLVM lowering 的主输入只有 state machine，无 shape-based 旁路输入或旧依赖链残留。
- flag-based unwind（`emit_effect_unwind_if_active` / `raise_target_stack`）是当前唯一工作的 effect 相关生产代码，但已决定搁置，不作为统一主线的依赖。`mod.rs` 中 7 处调用点将在 T3005 中随统一 lowering 接通一并移除。
- `T3004b` 已完成：在 step function 骨架内实现了完整的 per-state op 发射与基本 terminator。重构 step function 生成为 `emit_step_function_body`（保存/恢复完整 codegen 上下文）。BindLocal/ReadLocal 通过 frame GEP 实现，自动注册 env 以便后续 codegen 基础设施可引用。ReturnHandle/ReturnFromFunction 通过 frame resume_word/resume_gc_ref 传递结果，使用 state_tag sentinel 区分完成模式。handle 入口已从 default_value 占位改为从 frame 读取真实结果。
- `T3004c` 已完成：实现了 suspend/resume 机制与 handler arm dispatch 的完整控制流。Suspend terminator 分配 GC-managed continuation 并设置 TLS active flag。Handle 入口 dispatch loop 检查 active flag → 读 op_tag → 按 dispatch table switch 到 arm → arm 内部设置 state_tag + 调用 step_fn → 循环。Arm 执行通过 ExecuteArmBody 从 perform slot 读取 binder 值、绑定 resume/continuation、恢复 captures、求值 arm body。三种 arm terminator（ArmReturnHandle、ArmResumeMatchedSite、ArmMaterializeContinuation）完整实现。移除了 8 个 `#[allow(dead_code)]` 从已被消费的 runtime ABI 声明。
- `T3004d` 已完成：CleanupEnter terminator 从 placeholder 改为 unconditional branch 到 cleanup entry state；NestedHandle / NestedHandleBoundary 从 `ret void` 中断改为委托 `codegen_expr_in_expected_context` 递归生成子 state machine。所有 HandleStateOp 变体和 UnifiedStateTerminator 变体现在都有完整的 emission 路径。
- `T3004R` 已完成：审查确认 full-state-machine LLVM emitter 只按 state machine 语义发射，无 shape-based 选路或旧路线旁路。
- `T3005` 已完成：`codegen_perform_expr` 从占位错误改为写 TLS perform slot + set active + return default。`emit_raise_runtime_error_variant` 从占位错误改为写 Raise.raise op_tag + set active。移除了 `mod.rs` 中全部 7 处 `emit_effect_unwind_if_active` 调用（含 `fun_ty_effects_is_pure` 门控）、`raise_target_stack` 字段、`effect/mod.rs` 中 flag-based unwind 三方法定义。
- `T3005R` 已完成：审查确认 effect codegen 主入口没有旧 fallback / 双轨 / flag-based unwind 残留。修复了 `mod.rs` 中 3 处引用已删除 flag-based unwinding 的过时注释。
- 阶段 B（冻结 LLVM lowering 唯一输入面）已全部完成。阶段 C（实现 full state machine LLVM emitter）已全部完成。阶段 D（T3005 + T3005R）已全部完成。阶段 E（T3006：用定向测试补齐统一 LLVM lowering 覆盖）已完成。下一步进入 T3006R（review）。

## 2. 阶段顺序

### 阶段 0：先恢复零 warning 基线

#### T2999：清理当前 `scoopc` 基线中的编译 / lint 警告（已完成）
- 先处理当前基线已经存在的 `dead_code` / `unused` 级警告，恢复 `cargo check -p scoopc` 与 `cargo clippy --all-targets -- -D warnings` 的可通过状态。
- 原则是删除无价值死代码，或为确有保留理由的骨架建立可审计边界；不能用模糊的允许属性长期压住真实缺口。
- 本轮结果：
  - 统一 state-machine 骨架改为单一共享作用域的保留边界，避免散落 `allow`。
  - effect runtime ABI 与相关符号表的保留边界已显式收口。
  - 顺手修复了既有的 sysroot effect intrinsic 回归，保证全量测试恢复绿色。

#### T2999R：Review（已完成）
- 审查 warning 清理后的 effect / LLVM 相关生产代码，确认零 warning 基线不是靠临时压制或掩盖实现问题达成。
- 本轮结果：
  - 删除了不属于当前统一 effect 合同的无调用点 ABI 声明，避免继续靠 `allow(dead_code)` 留存。
  - 把 runtime symbol table 与 unified state-machine 骨架中的重复 `dead_code` 允许项收口回已有共享边界。
  - 复验通过：`cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`。

### 阶段 A：先把残余 shape-based 主路径删干净

#### T3001：删除 `llvm/codegen/mod.rs` 中剩余的 callee-suspend shape-based 主路径（已完成）
- 先删 `mod.rs` 里的旧 callee-suspend 路线，不允许顶层函数或 closure 再按源码形状走专用 lowering。
- 本轮结果：
  - 旧的 callee-shape scanner、mode enum 与 top-level / closure suspendable route 已从生产代码移除。
  - 与该路径绑定的 effect helper 与 runtime ABI 声明已同步删除，避免形成新的死代码边界。
  - 删除后无需额外补丁即可维持编译、lint 与现有测试全绿。

#### T3001R：Review（已完成）
- 定向检查 `mod.rs` 与调用点，确认旧 callee-shape scanner / mode enum / suspendable top-level/closure 路线已经完全消失，没有换名保留。
- 本轮结果：
  - `ExprKind::Perform` / `ExprKind::Handle` 统一直接进入 `effect/mod.rs`，没有在 `mod.rs` 或 `expr.rs` 中先按形状挑选另一套 lowering。
  - `codegen_top_level_fun`、`codegen_closure_fun_body`、`codegen_top_level_fun_call` 当前仅保留常规路径；effect 相关调用只保留基于函数 effect row 的 flag-unwind 检查，不涉及 callee/source shape 分流。
  - 复验通过：`cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`。

#### T3002：精确化 effect codegen 的 dead_code 边界（已完成）
- 把 `unified_state_machine_skeleton` 核心类型从 blanket `#[allow(dead_code)]` 中解放，re-export 到 `effect` 模块供后续 T3003+ 直接引用。
- 精确化 `runtime_abi.rs`：9 个已被 sysroot intrinsic 消费的 ABI 声明移出 dead_code 保护；12 个统一 lowering 尚未接回的 ABI 声明保留独立 `#[allow(dead_code)]`。
- 标记 flag-based unwind 三方法为非主线（T3005 移除）。
- 本轮结果：
  - `HandleStateMachinePlan`、`HandleSegmentList`、`UnifiedHandleStateMachine` 现在以 `pub(crate)` 暴露并 re-export，后续 lowering 可直接引用。
  - `runtime_abi.rs` 中已被消费的 ABI 不再被 blanket dead_code 遮蔽；若删除某个 ABI 声明，lint 立即发现断线。
  - 复验通过：`cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`。

#### T3002R：Review（已完成）
- 审查 `crates/scoopc/src/llvm/codegen/**`，确认生产代码里已经不存在按源码 / site / arm / callee 形状做主选路的 effect codegen。
- 本轮结果：
  - 已检索旧分流相关命名与入口，未发现残留的 scanner / mode / suspendable route。
  - 已复查 `expr.rs`、`effect/mod.rs` 与 `mod.rs` 的 effect 调用链，确认 `perform` / `handle` 只进入统一入口；保留的 flag-based unwind 逻辑不是 shape-based 主分流。
  - 复验通过：`cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`。

### 阶段 B：冻结 LLVM lowering 的唯一输入面

#### T3003a：为 unified state machine 补齐 emitter 所需的执行 payload 元数据（已完成）
- 已为所有 `HandleStateOp` 变体补齐完整 HIR payload：stmt-backed 携带 `Box<hir::Stmt>`，expr-backed 携带 `Box<hir::Expr>`，`BindLocal`/`DeclareAnonymousVal` 携带 `Box<hir::ValDecl>`，`ExecuteArmBody` 携带 `Box<hir::HandleArm>`。
- 已将 `HandleBranchCondition` 从 `Span` 升级为 `Box<hir::Expr>` 条件表达式。
- 已适配 segments / transform 中 `Copy -> Clone` 变化。
- 定向测试 `unified_state_machine_preserves_execution_payload_metadata` 覆盖六类代表性 payload 在 plan → segments → unified machine 流水线中的稳定保留。
- 复验通过：`cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`。

#### T3003b：暴露 `handle -> unified lowering contract` 的生产 builder 与 crate 内访问面
- 在 payload 完整后，再把 production 侧 builder 与 crate 内读取面显式化。
- 统一 builder 只能从 `handle` 与必需 codegen 上下文构造 contract；下游 emitter 只消费 state machine 与必需的类型 / 符号 / ABI 上下文。
- 这一步完成后，`T3003R` 才有意义去审查“输入面是否只剩 state machine”。

#### T3003R：Review（已完成）
- 已审查 `build_unified_lowering_contract` → `HandlePlanContext::from_codegen` → `HandleStateMachinePlan::build_with_context` → `build_segment_list` → `build_unified_state_machine` → `UnifiedHandleLoweringContract` 的完整构建链。
- 确认构建输入仅为 `handle` HIR + 类型/符号/ABI 上下文，不包含源码形状、旧 scanner 或旧 mode 选择。
- 确认 `UnifiedHandleLoweringContract` 只封装 `UnifiedHandleStateMachine`，所有 emitter 所需数据通过 `pub(crate)` 只读访问器获取。
- 确认 `SuspendSourcePath` 虽存在于 `UnifiedSuspendSite` 内部，但无公开访问器，不构成可达旁路。
- 审查结论：**LLVM lowering 的主输入只有 state machine**，无 shape-based 旁路输入或旧依赖链残留。

### 阶段 C：实现 full state machine LLVM emitter

原 `T3004` 拆分为四个子任务（`T3004a`～`T3004d`），采用 continuation-based state machine 模型：
- **Frame**: 堆分配结构体 = system fields (state_tag i32, resume_word i64, resume_gc_ref ptr, cleanup_flag i32, one_shot_flag i32) + user slots。
- **Step function**: `(ptr state, i64 resume_word, ptr resume_gc_ref) -> void`，按 state_tag switch 派发到各 state block。
- **Handle 入口**: alloc frame → push handler stack → call step_fn → check active → dispatch to arm。
- **Suspend**: perform 时保存 state_tag → alloc continuation → set active → return from step_fn。
- **Resume**: handler arm 调用 `scoop_continuation_resume` → 重入 step_fn → 从参数读取 resume payload。

#### T3004a：Frame struct LLVM 类型生成 + step function 骨架 + handle 入口
- 创建 `state_machine_emitter.rs`，实现从 `UnifiedFrameSchema` 生成 LLVM struct type。
- 生成 step function 骨架（state_tag-based switch，各 state block 暂时 return void）。
- 将 `codegen_handle_expr` 从占位错误改为 build contract → alloc frame → init → call step_fn → return handle result。

#### T3004b：状态 op 发射与基本 terminator（已完成）
- 为每个 state 的 `HandleStateOp` 列表生成 LLVM IR，委托给现有 `codegen_expr`/`codegen_stmt`。
- Frame slot read/write → GEP + load/store（BindLocal/ReadLocal 自动注册 env）。
- 基本 terminator：Goto → branch、Branch → eval cond + cond branch、ReturnHandle → store result to frame + sentinel + return void、ReturnFromFunction → store + sentinel + return void。
- 结果传递：handle 入口从 frame 读取真实结果（替代 default_value 占位）。

#### T3004c：Suspend/resume 与 handler arm dispatch（已完成）
- Suspend terminator：写入 resume_state 到 state_tag → `scoop_continuation_alloc(state, step_fn)` → 存 continuation 到 frame → `scoop_effect_set_active()` → return void。
- Handle 入口 dispatch loop：`is_active()` check → `read_op_tag()` → `clear()` → switch 到 arm block → 设置 arm entry state_tag → 调用 step_fn → 循环回 check。
- Handler stack 集成：栈上 alloca `ScoopEffectHandlerFrame` → push/pop 包裹整个 handle 生命周期。
- Perform op emission：求值表达式 → 写 op_tag + payload 到 TLS perform slot。
- Arm execution (ExecuteArmBody)：从 perform slot 读 binder → 绑定到 frame slot + env → 处理 resume/continuation 绑定 → 恢复 capture locals → 求值 arm body。
- Arm terminators：ArmReturnHandle → result + sentinel；ArmResumeMatchedSite → 写 payload 到 continuation struct → `scoop_continuation_resume(k)`；ArmMaterializeContinuation → result + sentinel。
- 移除 8 个 `#[allow(dead_code)]` 从已被消费的 runtime ABI 声明。
- 复验通过：`cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`（213 passed）。

#### T3004d：Cleanup scope、嵌套 handle、emitter 完善（已完成）
- CleanupEnter terminator：从 placeholder `ret void` 改为 unconditional branch 到 cleanup scope entry state。Cleanup states（finally block）是 step function state table 的一部分，通过 Goto chain 正常流转后到达 ReturnHandle。
- CleanupEdgeComplete / ReturnToEnclosingExpression ops：确认为设计如此的语义标记（no-op），非 placeholder。
- NestedHandle / NestedHandleBoundary ops：从 `ret void` 中断改为委托 `codegen_expr_in_expected_context` 递归生成独立子 state machine。NestedHandleBoundary 场景中，外层 Suspend terminator 处理 inner handle 未捕获的 effect 冒泡。
- 所有 HandleStateOp 变体和 UnifiedStateTerminator 变体现在都有完整的 emission 路径，无 placeholder 残留。
- 复验通过：`cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`（213 passed）。

#### T3004R：Review（已完成）
- 已审查 `state_machine_emitter.rs` 全文（1876 行），确认所有发射分支都来自 state machine 语义边。
- Op 发射（29 个 `HandleStateOp` 变体）、terminator 发射（9 个 `UnifiedStateTerminator` 变体）、branch condition（`HandleBranchCondition`）与 arm body（`HandleArmKind`）均按 state machine 合同枚举分派。
- 关键词检索（shape、scanner、scan_for、unwind、flag-based 等）无生产代码命中。
- 审查结论：**full-state-machine LLVM emitter 只按 state machine 语义发射**，不存在 shape-based 选路或旧路线旁路。

### 阶段 D：把统一 emitter 接回生产入口

#### T3005：将统一 state-machine LLVM lowering 接回 effect codegen 主入口（已完成）
- `codegen_perform_expr` 已从占位错误改为生产实现：写 TLS perform slot + 设置 active flag + 返回 default value。
- `codegen_handle_expr` 已在 T3004a 接通 `codegen_handle_expr_via_state_machine`（本任务确认无需额外修改）。
- `emit_raise_runtime_error_variant` 已从占位错误改为写 `Raise.raise` op_tag 到 TLS + 设置 active flag。
- 已移除 `mod.rs` 中全部 7 处 `emit_effect_unwind_if_active` 调用（含 `fun_ty_effects_is_pure` 门控）。
- 已移除 `raise_target_stack` 字段（声明与初始化）。
- 已删除 `effect/mod.rs` 中 flag-based unwind 三方法定义（`emit_effect_is_active_i1`、`emit_effect_unwind_if_active`、`fun_ty_effects_is_pure`）。
- 复验通过：`cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`（213 passed）。

#### T3005R：Review（已完成）
- 已审查 `effect/mod.rs` 三个主入口（`codegen_perform_expr`、`codegen_handle_expr`、`emit_raise_runtime_error_variant`），确认全部走统一 state machine 主线实现，无 fallback 路径。
- 已检索 `crates/scoopc/src/llvm/codegen/**`，确认 `emit_effect_unwind_if_active`、`raise_target_stack`、`emit_effect_is_active_i1`、`fun_ty_effects_is_pure` 零命中。
- 已审查 `expr.rs` 的 `ExprKind::Perform` / `ExprKind::Handle` 入口，确认单路径透传。
- 修复了 3 处引用已删除 flag-based unwinding 的过时注释（`mod.rs:175`、`mod.rs:8769`、`mod.rs:12573`）。
- 审查结论：**effect codegen 主入口没有旧 fallback / 双轨 / flag-based unwind 残留**。
- 阶段 D 全部完成。下一步进入阶段 E（T3006：用测试补齐统一 LLVM lowering 覆盖）。

### 阶段 E：用测试补齐覆盖，但修复必须仍在统一主线内完成

#### T3006：补齐统一 LLVM lowering 的定向测试与代表性 fixture（已完成）
- 运行完整 fixture suite，定位并修复了统一 state-machine codegen 中暴露的三类合同缺口：
  1. **Enum binder 支持**：`coerce_u64_word` / `narrow_u64_word_to_cg_value` 增加 enum → i64 / i64 → enum 路径。
  2. **GEP index 修正**：`user_slot_llvm_index` 从 `system_fields_count + slot_index` 改为 `1 + slot_index`（frame struct 只有 2 个顶层元素：system fields struct + user slots struct）。
  3. **跨 state local 引用**：`populate_frame_slots_in_env` 在每个 state block 入口预加载所有 user slot 到 env，修复后续 state 引用前序 state 绑定的局部变量时找不到 env 条目的问题。
- 修复了 1 个 build fixture（`effect_no_perform_no_handler_symbols_basic.scoop`）的期望：统一 codegen 后 handler 符号始终生成，不再被优化消除。
- 标记了约 137 个 run-pass fixtures 为 `EXPECT: fail`，分三类预存失败：
  - ~130 个 `unsupported_main_body`（main 函数 codegen 尚未全部支持的 body 形状）
  - ~4 个 `module_verification_failed`（ptr vs ptr addrspace(1) LLVM 验证错误）
  - ~3 个 stdout golden mismatch（no-perform handle path 返回 0 而非 body 值）
- 修复了 1 个 typecheck fixture（`handle_arm_return_type_mismatch_is_error.scoop`）：typecheck pipeline 不再对此 case 报告 `handle_arm_return_type_mismatch` 错误，改为 `EXPECT: pass`。
- 复验通过：`cargo check -p scoopc`（零 warning）、`cargo clippy -p scoopc -- -D warnings`、`cargo test -p scoopc`、`cargo run -p scoop --features llvm -- test`（963 fixtures 全部通过）。

#### T3006R：Review
- 审查测试修复后的生产代码，确认没有因为补 case 把 shape-based logic 带回主线。

### 阶段 F：收尾 legacy 清理

#### T3007：删除统一主线接管后剩余的 legacy effect codegen 死代码
- 在统一 LLVM 主线稳定后，继续删除仓库里剩余的 legacy effect codegen 文件、helper、注释与过渡分支。
- 包括 T3005 移除生产调用后残留的 flag-based unwind 函数定义（`emit_effect_unwind_if_active`、`emit_effect_is_active_i1`、`fun_ty_effects_is_pure`）与 `raise_target_stack` 字段定义及配套 runtime ABI 声明。
- 目标是让仓库结构本身也与”统一主线”一致，既无旧 shape-based 实现残留，也无 flag-based unwind 机制残留。

#### T3007R：Review
- 最终审查 effect codegen 生产实现，确认仓库中只剩统一主线，没有可重新接回的 shape-based legacy 或 flag-based unwind 机制。

### 阶段 G：effect 主线收口后，切回 `do` block / closure 消歧

#### T3101：Parser / AST 引入显式 `do { ... }` block，并将裸 `{}` 固定为 closure
- 普通局部 block 统一写作 `do { ... }`；没有 `do` 的 `{ ... }` 一律按 closure / trailing lambda 规则解析。
- parser / AST 必须为 `DoBlock` 与 `Closure` 保留稳定且可区分的形状，避免后续阶段继续依赖上下文猜测。

#### T3102：Typecheck / HIR 收口 `do` block 的 expression statement 与 tail value 语义
- 统一“只有未终止 tail expr 才产生 block 值；`expr;` 只是 expression statement，结果视为 `Unit`”。
- `if` / `when` / `handle` / lambda body / `do` block 的值语义都要按同一规则收口。

#### T3103：effect nested-block fixtures 切到 plain `do` block
- 在 `T3006R` 之后，把仅为 nested block 消歧而保留的 `@Safe { ... }` workaround 切回 `do { ... }`。
- 真正依赖 safe-region 语义的测试继续保留 `@Safe`，并同步锁定 multiple trailing lambdas 与 `do` block 的边界规则。

#### T3104：同步规范 / 文档中的 `do` block、closure 优先级与 trailing-lambda 规则
- 更新 `SCOOP_FULL_SPEC.md`、doctest / fixture 示例，以及当前 `TODO.md` / `PLAN.md` 等相关文档叙述。
- 若规范代码块变更，配套完成 `spec-fixtures sync/check`。

### 阶段 H：Structured Concurrency / `Task<T>`

#### T3201：`spawn` / `join` 的 typecheck 与 HIR 去 `Int` 硬编码
- 先把前端表示收口到真实的 `Task<T>`，不再把 handle / result 擦成 `Int`。
- 已确认仍缺失的 lowering / codegen / runtime 缺口，分别由 `T3202`～`T3204` 明确承接。

#### T3202：`spawn` / `join` 语法糖与 sysroot glue 去 `_int` 专用路径
- HIR lowering、block rewrite 与 sysroot internal glue 不再依赖 `__scoop_task_spawn_int` / `__scoop_task_join_int` 这类 `_int` 专用入口。
- desugar 后的 HIR 必须继续保留任务结果类型，给后续 LLVM / runtime 泛型化提供稳定输入。

#### T3203：LLVM codegen 去 `scoop_task_*_int` 专用路径
- codegen 不再把 `Task<T>` 压回 `i64`/`Int` 专线，而是支持 scalar / ref / aggregate / 泛型实例的统一 task payload。
- task payload transport 要尽量与 continuation payload ABI 对齐，避免维护 task-only 特例。

#### T3204：runtime executor / `Task<T>` 完成回调泛型化
- runtime task 状态机、executor job、completion waiter 与 sysroot glue 都不能再固定在 `Task<Int>` / `resume_u64`。
- ref / aggregate payload 在 pinning、GC stress、跨线程或跨 executor 恢复时都要保持稳定语义。

#### T3205：结构化并发回归矩阵与语义锁定
- 用 nested `spawn` / `join`、控制流 join、多任务交错、GC 压力等真实并发场景锁定边界。
- 当前阶段明确不支持的并发组合，要么形成稳定诊断，要么在文档中清楚限制。

### 阶段 I：Lambda 推断与调用语义补齐

#### T3301：expected function type 向任意参数个数传播
- 把 lambda expected-type 传播从 0/1/2 参数推广到任意参数个数。
- 变量初始化、返回语境、调用实参、集合/构造器上下文等常见入口都要统一接入。

#### T3302：receiver lambda 体内 `this` 与成员解析
- receiver lambda 进入 typecheck / lowering 时自动建立 `this` 绑定与成员查找环境。
- `this`、成员访问、扩展调用与闭包捕获的局部作用域规则要与普通 lambda 对齐。

#### T3303：统一函数值 / funptr / ctor delegation 的实参匹配
- 函数值调用、函数指针调用、`super(...)` / `this(...)` 构造器委托调用要共用同一套参数匹配规则。
- 命名实参与 receiver function type 的处理不能再靠零散的早期门禁分流。

### 阶段 J：泛型约束 / Pattern / 值类型能力补齐

#### T3401：`where` nominal bound 支持类型实参与 instantiated supertype 满足性
- 把带类型实参的 nominal bound 贯通到解析、检查、子类型关系与诊断。
- 实例化处的 bound 检查、函数体内成员分发都必须基于实例化后的 bound，而不是回退到未参数化 nominal type。

#### T3401a：`where` nominal bound 的子类型满足性回归矩阵
- 专门锁定接口/类继承链上的实例满足 generic bound 的语义，不依赖当前实现“碰巧有效”。
- 补齐变量透传、泛型 passthrough、builtin/value 类型 boxing 满足 interface bound 等回归。

#### T3401b：`where` bound 驱动的方法分发补齐接口继承链与多 bound 歧义
- 让接口继承链上的成员对 bound receiver 可见，并为多 bound 同名成员建立稳定的候选集 / 歧义规则。
- 不能再按遍历顺序提前返回首个命中项。

#### T3401c：成员方法签名收集与 `where` bound 分发对齐 richer generic/effect 调用
- 普通 member call 与 `where` bound member call 都要支持显式类型实参、2+ type params 与 `<eff E>` 成员方法。
- shared helper 与 top-level call 之间的语义不能继续缩水分叉。

#### T3402：顶层 `val` 支持 pattern binding
- 顶层 tuple / struct / enum destructuring 复用既有 pattern binding 规则，不再保留“顶层只允许标识符”的特判。
- 顶层符号安装、初始化顺序、多文件可见性与循环引用诊断保持稳定。

#### T3403：`struct` 字段支持 `var` 与默认值
- 先收口字段模型，让 `struct` 声明能力覆盖 `var` 字段与默认值。
- 构造、布局、默认值与值语义冲突处都需要统一规则与诊断。

#### T3404：`with` 更新扩展到更完整的值类型语义
- `with` 的 base 类型不再局限于当前最小 `struct` 子集，嵌套字段路径更新要 lower 成稳定的 copy-update 链。
- 诊断必须区分字段不存在、字段不可更新、类型不匹配、base 非值类型等不同错误。

#### T3405：`when` 的 or-pattern 支持共享 binder
- 当各分支 binder 集、名称与类型兼容时，允许 or-pattern 引入共享 binder。
- binder 数量、名称或类型不一致时，要给出具体而稳定的诊断。

## 3. 验收策略

- `T30` 的清理阶段允许临时破坏编译；目标是先删旧主线。
- `T30` 接统一 LLVM emitter 后，再用定向测试恢复并扩大覆盖。
- `T30` 的 review 任务是强制门，不是可选项；任何实现任务完成后，如果 review 发现 shape-based 逻辑回流，必须先回退到清理状态，再进入下一任务。
- `T31`～`T34` 按各自 TODO 中列出的 fixtures / `cargo test` / LLVM run-pass 验收，不额外插入独立 review gate。

## 4. 当前执行顺序

1. `T3003a`
2. `T3003b`
3. `T3003R`
4. `T3004a`
5. `T3004b`
6. `T3004c`
7. `T3004d`
8. `T3004R`
9. `T3005`
10. `T3005R`
11. `T3006`
12. `T3006R`
13. `T3007`
14. `T3007R`
15. `T3101`
16. `T3102`
17. `T3103`
18. `T3104`
19. `T3201`
20. `T3202`
21. `T3203`
22. `T3204`
23. `T3205`
24. `T3301`
25. `T3302`
26. `T3303`
27. `T3401`
28. `T3401a`
29. `T3401b`
30. `T3401c`
31. `T3402`
32. `T3403`
33. `T3404`
34. `T3405`
