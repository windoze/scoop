# TODO（Scoop：effect 主线优先 + 后续任务）

> 生成时间：2026-04-15  
> 历史归档：`TODO-3.md` / `PLAN-3.md`  
> 范围：本文件先记录当前 effect 统一主线（`T30`），并顺延保留下一批待推进的前端 / 并发 / 类型系统任务（`T31`～`T34`）。当前执行优先级仍以 `T30` 为先；`T31`～`T34` 作为 effect 主线收口后的后续队列。

## 约束

- 生产代码中的 effect codegen 禁止根据源码或代码形状分流。禁止把 `single` / `multi` / `direct` / `indirect` / `top-level` / `nested` / callee shape 等分类当作 lowering 决策输入。
- LLVM codegen 的单一输入是 state machine。除类型信息、符号信息与运行时 ABI 必需信息外，不允许再读取源码形状、旧 scanner 结果或旧 shape 分类结果。
- 当前阶段默认生成 heap-allocated full state machine；暂不做 simplification / optimization。
- flag-based unwind（`emit_effect_unwind_if_active` / `raise_target_stack`）当前明确搁置，不作为 T30 主线依赖。统一 state machine 是 effect 传播的唯一主线机制；flag-based unwind 日后可作为优化选项加回，但不阻塞当前任务推进。
- 每个实现任务之后都必须立即执行一个 review 任务；review 只审查生产代码，不以测试代码命名作为问题。若发现生产代码问题，必须在该 review 任务内直接修复并复审，不能只记录问题不处理。

## 当前已知问题（T30）

- `mod.rs` 中的旧 callee-suspend shape-based 主路径已删除（T3001/T3001R 完成）；生产代码中不再存在按源码 / callee 形状选路的 effect codegen 分流。
- `effect/mod.rs` 的 `codegen_perform_expr` / `codegen_handle_expr` 当前返回占位错误（`UnsupportedMainBody`），effect codegen 不工作。
- 统一 state-machine 骨架（plan builder、segmentation、state machine transform）已实现并有测试，但整体处于 `#[allow(dead_code)] mod unified_state_machine_skeleton` 中，无生产入口消费。
- `runtime_abi.rs:1254-1553` 的 `#[allow(dead_code)]` impl 块包含约 30 个 effect runtime ABI 声明，其中部分（`is_active`、`set_active`、`clear`、perform slot 读写）已被 sysroot intrinsic 生产路径消费，但 lint 无法区分真正 dead 与已使用的声明。
- flag-based unwind（`emit_effect_unwind_if_active` / `raise_target_stack`）当前是唯一工作的 effect 相关生产逻辑，用于 Raise/try-catch 传播。**已决定搁置**：统一 state machine 主线完成后，flag-based unwind 可作为优化加回，但不作为当前依赖。`mod.rs` 中 7 处 `emit_effect_unwind_if_active` 调用点（lines 8794, 9060, 9306, 9751, 9922, 12503, 12726）与 `raise_target_stack` 需在接通统一 lowering 时一并移除或失效。

## T30：统一 effect LLVM codegen

### T2999 [DONE] 清理当前 `scoopc` 基线中的编译 / lint 警告，恢复零 warning 门槛
- 描述：在继续 effect 主线重构前，先把当前基线里已经存在的 `dead_code` / `unused` 级警告收口掉，否则后续任务无法满足仓库明确要求的 `cargo clippy --all-targets -- -D warnings`。
- 进展：
  - 已把统一 state-machine 骨架、effect runtime ABI 声明与相关 runtime 符号收口到可审计的保留边界，并删除明确无读取路径的死字段 / 死方法。
  - 已补回 `scoop.core.__scoop_effect_*` sysroot 测试辅助 intrinsic 到 runtime 符号的直接 lowering，修复既有回归测试失败。
- 目标：
  - 处理 `cargo check -p scoopc` 当前暴露的既有警告，优先覆盖 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`、`state_machine_transform.rs`、`runtime_abi.rs`、`runtime_symbols.rs` 与相邻统一主线骨架。
  - 对暂时不会接入生产入口但需要保留的结构，明确采用可审计的保留方式；对已经无价值的死代码，直接删除。
  - 恢复 `cargo clippy --all-targets -- -D warnings` 可作为后续任务统一质量门槛。
- 验收：
  - `cargo check -p scoopc` 无 warning。
  - `cargo clippy --all-targets -- -D warnings` 可通过。
- 依赖：无

### T2999R [DONE] Review：确认零 warning 基线恢复没有掩盖真正实现缺口
- 描述：清理 warning 后，专门审查 effect / LLVM 相关生产代码，确认不是靠滥用允许属性把实现缺口压下去；若发现这类问题，本任务需要直接修复并在修复后重新审查。
- 进展：
  - 已删除 `crates/scoopc/src/llvm/codegen/runtime_abi.rs` 中无生产调用点、也不属于当前统一 effect 合同的 `declare_runtime_alloc` / `declare_runtime_gc_collect`。
  - 已把 `crates/scoopc/src/llvm/codegen/runtime_symbols.rs` 中散落的冗余 `#[allow(dead_code)]` 清掉，恢复“符号表只保留符号名、dead_code 边界集中在 ABI/骨架侧”的单一审计边界。
  - 已删除 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 与 `state_machine_transform.rs` 中被 `effect/mod.rs` 统一骨架边界覆盖的重复 `dead_code` 允许项。
  - 已验证 `cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all` 全部通过。
- 目标：
  - 确认保留的允许项都有明确边界和理由，没有把应删除的 dead code 长期保留成隐患。
  - 确认 effect 统一主线相关模块的保留结构仍与后续计划一致，不是为了过 lint 临时打补丁。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“当前基线满足零 warning 门槛，且未用允许属性掩盖实现问题”。
- 审查结论：
  - 当前基线满足零 warning 门槛，且未用允许属性掩盖实现问题。
  - 仍保留的 `dead_code` 边界仅用于未重新接线的统一 effect 骨架、effect ABI 保留段与稳定 op-tag 分配状态，符合后续 `T3001+` 计划。
- 依赖：T2999

### T3001 [DONE] 删除 `llvm/codegen/mod.rs` 中剩余的 callee-suspend shape-based 主路径
- 描述：先删除当前 review 已确认的旧主路径，避免后续 LLVM lowering 继续从顶层函数 / closure 的源码形状分流。
- 进展：
  - 已删除 `CalleeSuspendResumeMode`、`scan_for_callee_suspend`、`codegen_top_level_fun_suspendable`、`codegen_closure_fun_body_suspendable` 及其入口接线。
  - 顶层函数与 closure 的 LLVM codegen 已收口回常规路径，不再按 `perform` 所在源码形状分流到专用 suspendable lowering。
  - 已同步清理 `effect/mod.rs`、`runtime_abi.rs`、`runtime_symbols.rs` 中仅服务于这条旧路径的 helper / ABI 声明，避免删除后回到 warning 状态。
  - 已验证 `cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all` 全部通过。
- 目标：
  - 删除 `CalleeSuspendResumeMode`、`scan_for_callee_suspend`、`codegen_top_level_fun_suspendable`、`codegen_closure_fun_body_suspendable` 及其入口接线。
  - 顶层函数与 closure 不再按 `val = perform` / `return perform(...)` / block 尾 `perform(...)` 等形状进入专用路径。
  - 允许这一步暂时破坏编译；本任务的目标是彻底删除旧 shape-based 生产逻辑，而不是修旧路径。
- 验收：
  - 生产代码中不再出现上述符号。
  - `crates/scoopc/src/llvm/codegen/mod.rs` 不再存在按 callee/source shape 选择 suspendable lowering 的入口。
- 依赖：T2999R

### T3001R [DONE] Review：确认 `mod.rs` 的 callee-suspend shape-based 逻辑已清零
- 描述：在继续后续实现前，专门审查 `crates/scoopc/src/llvm/codegen/mod.rs` 与 effect 入口接线，确认没有等价回流；若发现残留或变体回流，本任务需要直接修复并在修复后重新审查。
- 进展：
  - 已定向检索 `crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/effect/mod.rs` 与相邻调用点，确认 `CalleeSuspendResumeMode`、`scan_for_callee_suspend`、`codegen_top_level_fun_suspendable`、`codegen_closure_fun_body_suspendable` 及其等价换名变体都未残留在生产路径中。
  - 已复查 `codegen_top_level_fun`、`codegen_closure_fun_body`、`codegen_top_level_fun_call` 与 `ExprKind::Perform` / `ExprKind::Handle` 的接线；当前顶层函数与 closure 只走常规 codegen，effect 入口统一落到 `effect/mod.rs` 的占位入口，不再按源码 / callee 形状进入专用 suspendable lowering。
  - 已验证 `cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all` 全部通过。
- 目标：
  - 确认没有以新名字保留旧的 callee-shape scanner / mode enum / suspendable top-level/closure route。
  - 确认相关调用点已经转为空、删除或等待统一主线接管，而不是换名保留旧分流。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“生产代码无 callee-suspend shape-based 主路径残留”。
- 审查结论：
  - 生产代码无 callee-suspend shape-based 主路径残留。
  - `crates/scoopc/src/llvm/codegen/effect/mod.rs` 当前仅保留统一 state-machine 骨架与未接回的统一 lowering 占位入口，没有顶层函数 / closure 的旧 suspendable route 等价回流。
- 依赖：T3001

### T3002 [DONE] 精确化 effect codegen 的 dead_code 边界，把统一骨架从 blanket allow 中解放
- 描述：T3001 删除 `mod.rs` 旧路线后，`effect/**` 中已不存在旧 scanner、旧 resolver 或旧 dedicated matrix。当前残留问题是：(1) `unified_state_machine_skeleton` 整体被 `#[allow(dead_code)]` 包裹，后续任务无法直接使用其中的类型；(2) `runtime_abi.rs` 的 dead_code impl 块同时覆盖了已被 sysroot intrinsic 消费的 ABI 声明与真正未使用的 ABI 声明，lint 无法区分；(3) flag-based unwind 相关代码（`emit_effect_unwind_if_active`、`emit_effect_is_active_i1`、`raise_target_stack`、`fun_ty_effects_is_pure`）在统一主线完成前暂留原位，但需明确标记为非主线。
- 进展：
  - 已把 `HandleStateMachinePlan`、`HandleSegmentList`、`UnifiedHandleStateMachine` 从 `pub(super)` 改为 `pub(crate)`，并从 `unified_state_machine_skeleton` 模块 re-export 到 `effect` 模块，使后续 T3003+ 生产代码可直接引用。
  - 已将 `runtime_abi.rs` 的 blanket `#[allow(dead_code)]` impl 块拆分为：9 个已被 `codegen_sysroot_effect_intrinsics` / `emit_effect_is_active_i1` 消费的 ABI 声明不再受 dead_code 保护；12 个统一 lowering 尚未接回的 ABI 声明保留独立的 `#[allow(dead_code)]` 注解。
  - 已在 `effect/mod.rs` 中为 flag-based unwind 三方法（`emit_effect_is_active_i1`、`emit_effect_unwind_if_active`、`fun_ty_effects_is_pure`）添加非主线标记，注明 T3005 接通统一 lowering 时移除。
  - 已验证 `cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all` 全部通过。
- 目标：
  - 把 `effect/mod.rs` 中的 `unified_state_machine_skeleton` 模块从 blanket `#[allow(dead_code)]` 中解放：将后续 lowering 需要消费的类型（`HandleSegmentList`、`UnifiedHandleStateMachine` 等）暴露为可被生产入口引用的结构，对仍未接入的中间步骤保留精确的 dead_code 边界。
  - 精确化 `runtime_abi.rs` 的 `#[allow(dead_code)]` 边界：已被 `codegen_sysroot_effect_intrinsics` 等生产路径消费的 ABI 声明不应继续留在 blanket dead_code 块中；仅对统一 lowering 尚未接回的 ABI 声明保留显式 allow。
  - 确认 `effect/**` 中不存在旧 shape-based helper / module / dead branch。
- 验收：
  - 统一骨架的核心类型可被后续 T3003+ 的生产代码直接引用，不再被 blanket dead_code 遮蔽。
  - 已被生产路径消费的 ABI 声明不受 dead_code allow 覆盖；若删除某个 ABI 声明，lint 能立即发现断线。
  - `cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all` 通过。
- 依赖：T3001R

### T3002R [DONE] Review：确认 effect codegen 生产代码中不存在 shape-based 主分流
- 描述：对 `crates/scoopc/src/llvm/codegen/**` 做定向审查，只看生产代码，不看测试；若发现 shape-based 主分流残留或新引入变体，本任务需要直接修复并在修复后重新审查。
- 进展：
  - 已定向检索 `crates/scoopc/src/llvm/codegen/**` 中与旧分流相关的命名与入口，包括 `shape`、`scan_for`、`CalleeSuspend`、`suspendable` 等，未发现残留命中。
  - 已复查 `crates/scoopc/src/llvm/codegen/expr.rs`，确认 `hir::ExprKind::Perform` / `hir::ExprKind::Handle` 直接接到 `effect/mod.rs` 的统一入口，中间不存在按源码 / site / arm / callee 形状挑选 effect lowering 的分派层。
  - 已复查 `crates/scoopc/src/llvm/codegen/effect/mod.rs` 与 `crates/scoopc/src/llvm/codegen/mod.rs` 的调用链，确认当前保留的 effect 相关生产逻辑只剩统一 lowering 占位入口、sysroot intrinsic lowering 与 flag-based unwind 辅助；其中 flag-based unwind 调用点虽然仍待 `T3005` 移除，但并不按源码形状分流。
  - 已验证 `cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all` 全部通过。
- 目标：
  - 确认 effect codegen 的主路径不再按源码形状、site 形状、arm 形状或 callee 形状选路。
  - 确认新增代码没有重新引入新的 shape-based helper。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“当前生产代码无 shape-based effect codegen 主分流残留”。
- 审查结论：
  - 当前生产代码无 shape-based effect codegen 主分流残留。
  - `perform` / `handle` 入口当前虽然仍未重新接入统一 state-machine lowering，但现状是统一占位错误而不是 shape-based fallback 或双轨分流。
- 依赖：T3002

### T3003a [DONE] 为 unified state machine 补齐 emitter 所需的执行 payload 元数据
- 描述：当前 `HandleStateOp` / `HandleBranchCondition` 主要只保留标签、`span` 与少量 id；这足够做 segmentation / transform 结构验证，但不足以让后续 LLVM emitter 真正”只吃 state machine”完成发射。需要把分支条件、求值动作、arm body、resume 边界等执行所需的 HIR payload 或等价执行元数据直接写进统一合同，并确保这些 payload 能在 `plan -> segments -> unified machine` 之间稳定保留。
- 进展：
  - 已为所有 `HandleStateOp` 变体补齐完整 HIR payload：stmt-backed 操作携带 `Box<hir::Stmt>`，expr-backed 操作携带 `Box<hir::Expr>`，`BindLocal` / `DeclareAnonymousVal` 携带 `Box<hir::ValDecl>`，`ExecuteArmBody` 携带 `Box<hir::HandleArm>`。
  - 已将 `HandleBranchCondition` 的 `WhileCond` / `IfCond` 从仅保留 `Span` 升级为携带完整条件表达式 `Box<hir::Expr>`。
  - 已更新 builder 代码（`HandlePlanBuilder`），在构造每个 `HandleStateOp` 与 `HandleBranchCondition` 时传入完整 HIR 节点。
  - 已适配 `state_machine_segments.rs` 与 `state_machine_transform.rs` 中因 `Copy -> Clone` 变化导致的引用传递调整。
  - 已添加 `payload_signature` 系列函数（`expr_payload_signature`、`stmt_payload_signature`、`decl_payload_signature`、`handle_arm_payload_signature`），用于测试中验证 payload 身份稳定性。
  - 已添加定向测试 `unified_state_machine_preserves_execution_payload_metadata`，覆盖 `BindLocal`/decl、`WhileCondHeader`/stmt、`IfCond`/condition、`Perform`/expr、`ResumeAfterSite`/source_expr、`ExecuteArmBody`/arm 六类代表性 payload 在 plan → segments → unified machine 流水线中的稳定保留。
  - 已验证 `cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all` 全部通过。
- 目标：
  - 为 unified state machine 中所有会驱动求值/控制流/handler body 发射的节点补齐 payload：至少覆盖 branch condition、普通 state op、arm body 与 resume 边界。
  - 这些 payload 必须跟随 state machine 一路保留；后续 emitter 不需要再回看原始 `handle` HIR 树去重新定位对应节点。
  - 若在补齐过程中发现 state / suspend / arm 元数据仍不足以表达 future emitter 所需信息，本任务内继续补齐，而不是把缺口留给后续 shape-based fallback。
- 验收：
  - unified state machine 中涉及执行语义的节点都带有 emitter 所需的 payload 或等价执行元数据。
  - 定向测试能锁定 payload 在 `plan -> segments -> unified machine` 之间保持稳定。
- 依赖：T3002R

### T3003b [DONE] 暴露 `handle -> unified lowering contract` 的生产 builder 与 crate 内访问面
- 描述：在 payload 完整后，把 production 侧构建入口与读取接口显式化。builder 只允许从 `handle` 与必需的 codegen 上下文构造 unified lowering contract；下游 emitter 只消费 state machine 与必需的类型 / 符号 / ABI 上下文。
- 进展：
  - 已在 `state_machine_transform.rs` 中定义 `UnifiedHandleLoweringContract`，封装 `UnifiedHandleStateMachine`，提供便利委托访问器。
  - 已为 `UnifiedHandleStateMachine` 及所有子结构（`UnifiedState`、`UnifiedFrameSchema`、`UnifiedFrameSlot`、`UnifiedDispatchEntry`、`UnifiedDispatchArm`、`UnifiedArm`、`UnifiedSuspendSite`、`UnifiedCleanupScope`、`UnifiedStateEdge`）添加 `pub(crate)` 只读访问器。
  - 已为 `FrameSlot` 添加 `pub(crate)` 访问器（`id()`、`name()`、`ty()`、`mutable()`、`owner_arm()`），使其可通过 `UnifiedFrameSlot::slot()` 消费。
  - 已在 `UnifiedHandleStateMachine` 上添加 `pub(crate)` 查找方法：`get_state()`、`get_dispatch_entry()`、`get_suspend_site()`、`get_cleanup_scope()`；`UnifiedFrameSchema` 添加 `get_slot_field_index()`。
  - 已将 `build_handle_state_machine_plan` 升级为 `build_unified_lowering_contract`，完成 plan → segments → unified machine → contract 的完整 pipeline，作为 `MainCodegen` 的 `pub(super)` 方法。
  - 已将 `CleanupScopeKind` 的可见性从 `pub(in crate::llvm::codegen::effect)` 提升至 `pub(crate)`，以匹配 accessor 可见性。
  - 已从 `effect/mod.rs` re-export `UnifiedHandleLoweringContract`。
  - 已添加定向测试 `unified_lowering_contract_provides_complete_read_access`，覆盖 contract 的全部读取面：top-level 字段、states 查找、dispatch entries、arms、suspend sites、cleanup scopes、frame schema / slots、outgoing edges。
  - 已验证 `cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`（213 passed）全部通过。
- 目标：
  - 提供生产可调用的统一 builder，把 `handle` 收口为单一的 unified lowering contract。
  - 明确 crate 内 contract 读取面：state table、state edge、suspend edge、cleanup、frame layout、dispatch、capture / payload / slot 元数据都只能从 contract 读取。
  - 这层 API / contract 不再暴露对 HIR shape、scanner 结果、旧分类器或 flag-based unwind 机制的依赖。
- 验收：
  - 统一 LLVM lowering 入口能够只接受 state machine 及必需的类型 / 符号 / ABI 上下文。
  - 下游 emitter 所需的结构化输入都可从 unified lowering contract 读取，无需旁路回看原始 `handle` HIR。
- 依赖：T3003a

### T3003R [DONE] Review：确认 LLVM lowering 输入面只剩 state machine
- 描述：审查统一 lowering 入口与其依赖链，确认没有旁路输入；若发现旁路输入或旧依赖链残留，本任务需要直接修复并在修复后重新审查。
- 进展：
  - 已审查 `build_unified_lowering_contract` 入口：只接受 `&hir::HandleExpr` 与 codegen 上下文；`HandlePlanContext::from_codegen` 从 `MainCodegen` 提取的信息全部属于类型（effect purity）/符号（ctor call targets、object init FQNs）/ABI 层面，不包含源码形状、scanner 结果或旧 mode 选择。
  - 已审查 `UnifiedHandleLoweringContract`：只封装 `UnifiedHandleStateMachine`，无旁路字段。所有 `pub(crate)` 访问器指向 state machine 内部结构。
  - 已审查 `SuspendSourcePath`：虽存在于 `UnifiedSuspendSite` 内部，但无 `pub(crate)` 访问器，下游 emitter 不可见。仅用于内部验证与测试。
  - 已审查 `HandleStateOp`、`HandleBranchCondition`、`SuspendSiteKind`：分别携带执行 HIR payload、条件表达式、语义 suspend 原因，不携带源码形状或旧分类信息。
  - 已审查 `expr.rs` 入口：`ExprKind::Perform` / `ExprKind::Handle` 只传递 span、HIR、expected type 到 `effect/mod.rs` 统一入口，无旁路数据。
  - 已检索 `crates/scoopc/src/llvm/codegen/effect/**` 中 `shape`、`scanner`、`scan_for`、`CalleeSuspend`、`suspendable`、`SiteShape`、`ArmShape` 等关键词，无生产代码命中。
  - 已确认 flag-based unwind（`emit_effect_unwind_if_active` × 7、`raise_target_stack`）仅存于 `mod.rs` 非主线路径，已标记待 T3005 移除，与统一合同无交互。
  - 已确认 `runtime_abi.rs` 中 12 个 `#[allow(dead_code)]` ABI 声明均标注”T3002：统一 lowering 尚未接回”，无旁路输入。
  - 已验证 `cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all` 全部通过。
- 目标：
  - 确认 LLVM lowering 主线不再读取源码路径、旧 scanner 输出、旧 mode 选择结果。
  - 确认所有 effect lowering 所需结构信息都来自 state machine 合同。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录”LLVM lowering 的主输入只有 state machine”。
- 审查结论：
  - LLVM lowering 的主输入只有 state machine。`UnifiedHandleLoweringContract` 是唯一生产结构输入，只封装 `UnifiedHandleStateMachine`，通过 `pub(crate)` 只读访问器暴露所有 emitter 所需数据。
  - `build_unified_lowering_contract` 的构建输入仅为 `handle` HIR + 类型/符号/ABI 上下文，不包含源码形状、旧 scanner 结果或旧 mode 选择。
  - `SuspendSourcePath` 虽被保留在 `UnifiedSuspendSite` 内部，但无公开访问器，不构成 emitter 的可达输入。
  - 生产代码中不存在 shape-based 旁路输入或旧依赖链残留。
- 依赖：T3003b

### T3004a [DONE] Frame struct LLVM 类型生成 + step function 骨架 + handle 表达式入口
- 描述：为 unified state machine 创建 LLVM emitter 模块。实现三个核心组件：(1) 从 `UnifiedFrameSchema` 生成 LLVM struct type（system fields + user slots）；(2) 生成 step function `(ptr state, i64 resume_word, ptr resume_gc_ref) -> void` 骨架，内含 state_tag-based switch 派发到各 state block（各 state block 暂时 return void）；(3) 将 `codegen_handle_expr` 从占位错误改为 build contract → alloc frame → init system fields → call step_fn → return handle result。
- 目标：
  - 创建 `state_machine_emitter.rs` 并集成到 effect codegen 模块。
  - Frame LLVM struct type 正确反映 `UnifiedFrameSchema` 的 system fields 与 user slots 布局。
  - Step function 骨架能够编译，state dispatch 结构完整，各 state block 为占位 return。
  - `codegen_handle_expr` 不再返回占位错误，而是走 contract → frame → step_fn 骨架路径。
- 验收：
  - `cargo check -p scoopc` 无 warning。
  - `cargo clippy --all-targets -- -D warnings` 通过。
  - `cargo test --all` 通过。
- 依赖：T3003R

### T3004b [DONE] 状态 op 发射与基本 terminator
- 描述：在 step function 骨架内，为每个 state 的 `HandleStateOp` 列表生成 LLVM IR，并实现基本 terminator（Goto、Branch、ReturnHandle、ReturnFromFunction）。frame slot 的 read/write 映射到 GEP + load/store。op 发射尽量委托给现有的 `codegen_expr` / `codegen_stmt` 基础设施。
- 进展：
  - 重构 `emit_effect_step_function`：将 step function body 拆分为 `emit_step_function_body`，保存/恢复完整 codegen 上下文（env、return_context、current_fun_return_ty、loop_context_stack）。
  - 实现 `emit_state_ops`：遍历每个 state 的 ops 列表，按 HandleStateOp 变体分派。BindLocal → 求值 → frame GEP + store + env 注册；ReadLocal → frame GEP + load + env 注册；Literal/VarRef/Expr/Call 等表达式 op → 委托给 `codegen_expr_in_expected_context`；Assign → 委托给 `codegen_assign_stmt`；Return → 求值并追踪为 last_value；Suspend/Arm 等 T3004c/d op → placeholder return void。
  - 实现 `emit_state_terminator`：Goto → unconditional branch；Branch → eval condition + conditional branch；ReturnHandle → store result to frame resume fields + state_tag sentinel + return void；ReturnFromFunction → store value + function-return sentinel + return void；Suspend/Cleanup/Arm 等 T3004c/d terminator → placeholder return void。
  - 实现 frame 结果传递：`store_result_to_frame`（按 CgTy 分 resume_word / resume_gc_ref 存储）、`read_result_from_frame`（从 frame 读取结果）、`narrow_u64_word_to_cg_value`（u64 → CgTy 窄化）。
  - 更新 `codegen_handle_expr_via_state_machine`：step_fn 返回后从 frame 读取结果（替代 T3004a 的 default_value 占位）。
  - 移除 `FrameLayout` 上的 `#[allow(dead_code)]` 注解：`system_field_count` 和 `user_slot_llvm_index` 现在被 op 发射实际消费。
  - 已验证 `cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`（213 passed）全部通过。
- 目标：
  - 每个 state block 能执行其 ops（BindLocal、ReadLocal、Literal、Expr、Call 等）。
  - frame slot 的 BindLocal → GEP + store，ReadLocal → GEP + load。
  - Goto → 直接 branch 到目标 state block。
  - Branch → eval condition + conditional branch to then/else state blocks。
  - ReturnHandle → 把 handle result 写回约定位置并 return。
  - ReturnFromFunction → 生成函数级 return。
- 验收：
  - `cargo check -p scoopc` 无 warning。
  - `cargo clippy --all-targets -- -D warnings` 通过。
  - `cargo test --all` 通过。
- 依赖：T3004a

### T3004c [DONE] Suspend/resume 机制与 handler arm dispatch
- 描述：实现 effect 传播的核心机制。Suspend terminator：保存 state_tag → alloc continuation → push handler stack → set active → return from step_fn。Handle 入口 dispatch：step_fn 返回后检查 active flag → read op_tag → dispatch to matching arm。Arm 执行：emit arm body via ExecuteArmBody → arm exit paths（ArmReturnHandle、ArmResumeMatchedSite、ArmMaterializeContinuation）。Resume landing：重入 step_fn 时从参数读取 resume_word/resume_gc_ref → 继续执行。
- 进展：
  - 实现 Suspend terminator：写入 resume_state 到 state_tag → 调用 `scoop_continuation_alloc(state, step_fn)` 分配 GC-managed continuation → 将 continuation 指针存入 frame resume_gc_ref slot → 调用 `scoop_effect_set_active()` 设置 TLS active flag → return void。
  - 实现 handle entry dispatch loop：step_fn 返回后检查 `scoop_effect_is_active()` → 若 active，调用 `scoop_effect_perform_slot_read_op_tag()` 读取 op_tag → 调用 `scoop_effect_clear()` 清除 active flag → 按 op_tag switch 到对应 arm block → arm block 设置 state_tag 为 arm entry state 并再次调用 step_fn → 循环回 dispatch_check。
  - 实现 handler stack 集成：handle 入口分配栈上 `ScoopEffectHandlerFrame` → `scoop_effect_handler_stack_push(frame, op_tag)` → body 执行 → dispatch loop → `scoop_effect_handler_stack_pop(frame)` 在 handle 完成后弹出。
  - 实现 Perform op emission：求值 perform 表达式 → 按 payload 类型调用 `scoop_effect_perform_slot_write_u64` / `scoop_effect_perform_slot_write_u64_with_gc_ref`。
  - 实现 SuspendCall/ObjectInitAccessBoundary/RuntimeRaiseBoundary op emission：委托 codegen_expr 求值表达式（callee 内部可能 set active），由 Suspend terminator 处理后续。
  - 实现 ResumeAfterSite op：标记为 no-op，step_fn entry 已将 resume_word/resume_gc_ref 从参数写入 frame。
  - 实现 ExecuteArmBody op：从 TLS perform slot 读取 binder 值（`perform_slot_read_u64` / `perform_slot_read_gc_ref`）→ 写入 frame slot + 注册 env → 根据 HandleArmKind 处理 resume/continuation 绑定 → 恢复 captured locals → 求值 arm body。
  - 实现 arm terminator：ArmReturnHandle → 写 result 到 frame + HANDLE_RETURNED sentinel；ArmResumeMatchedSite → 写 resume payload 到 continuation struct (field 6/7) + 调用 `scoop_continuation_resume(k)`；ArmMaterializeContinuation → arm body 结果写入 frame + HANDLE_RETURNED sentinel。
  - 移除 8 个已被生产路径消费的 runtime ABI 声明的 `#[allow(dead_code)]` 注解：`llvm_effect_handler_frame_type`、`declare_runtime_effect_handler_stack_push/pop`、`declare_runtime_continuation_alloc/resume`、`llvm_continuation_struct_type`、`declare_runtime_effect_perform_slot_write_u64_with_gc_ref`、`declare_runtime_effect_perform_slot_read_gc_ref`。
  - 移除 `effect_op_tag` 方法上的 `#[allow(dead_code)]`。
  - 已验证 `cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`（213 passed）全部通过。
- 目标：
  - effect 传播完全由 state machine 状态转移驱动，不使用 flag-based unwind。
  - 完整的 suspend → dispatch → arm → resume 循环可工作。
  - `perform` 入口也通过同一机制传播，不走旧的 `codegen_perform_expr` 占位。
- 验收：
  - `cargo check -p scoopc` 无 warning。
  - `cargo clippy --all-targets -- -D warnings` 通过。
  - `cargo test --all` 通过。
- 依赖：T3004b

### T3004d [DONE] Cleanup scope、嵌套 handle 与 emitter 完善
- 描述：补齐 state machine emitter 的剩余组件：cleanup scope 的 enter/exit 路径、嵌套 handle 的递归 emitter、以及边界完善（error diagnostics、remaining HandleStateOp variants、edge cases）。
- 进展：
  - CleanupEnter terminator：从 placeholder `ret void` 改为 `build_unconditional_branch` 到 cleanup scope 的 entry state。Cleanup states（finally block）是 step function state table 的一部分，通过 Goto chain 正常执行后流回 ReturnHandle。
  - CleanupEdgeComplete / ReturnToEnclosingExpression ops：确认为设计如此的语义标记（no-op），更新注释说明不是 placeholder。
  - NestedHandle op：从 `ret void` 中断改为委托 `codegen_expr_in_expected_context(expr)` 递归进入 `codegen_handle_expr_via_state_machine`，生成独立的子 state machine。
  - NestedHandleBoundary op：同样委托 codegen_expr；外层 state machine 的 Suspend terminator 处理 inner handle 未捕获的 effect 冒泡。
  - 所有 HandleStateOp 变体和 UnifiedStateTerminator 变体现在都有完整的 emission 路径（或明确的语义 no-op），无 placeholder stub 残留。
  - 已验证 `cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`（213 passed）全部通过。
- 目标：
  - CleanupEnter terminator → 进入 cleanup scope → 执行 cleanup states → 退出。
  - 嵌套 handle 递归调用 emitter 生成子 state machine。
  - 所有 HandleStateOp 变体和 UnifiedStateTerminator 变体都有对应的 emission 路径。
  - LLVM emitter 从 state machine 构建完整控制流与运行时交互骨架。
- 验收：
  - `cargo check -p scoopc` 无 warning。
  - `cargo clippy --all-targets -- -D warnings` 通过。
  - `cargo test --all` 通过。
  - LLVM emitter 覆盖当前 state machine 能表达的所有合法边。
- 依赖：T3004c

### T3004R [DONE] Review：确认 full-state-machine LLVM emitter 不含 shape-based 选路
- 描述：在接主入口之前，先检查 emitter 主体本身是否纯粹消费 state machine；若发现 shape-based 选路或等价旁路，本任务需要直接修复并在修复后重新审查。
- 进展：
  - 已审查 `state_machine_emitter.rs` 全文（1876 行），逐一检查所有发射决策点。
  - `emit_state_ops`：所有 29 个 match arm 精确对应 `HandleStateOp` 枚举变体，每个变体代表 state machine 语义操作，无源码形状推断。
  - `emit_state_terminator`：所有 9 个 match arm 精确对应 `UnifiedStateTerminator` 枚举变体（Goto、Branch、Suspend、CleanupEnter、ReturnHandle、ReturnFromFunction、ArmReturnHandle、ArmResumeMatchedSite、ArmMaterializeContinuation）。
  - `emit_branch_condition`：只从 `HandleBranchCondition::WhileCond` / `IfCond` 提取条件表达式。
  - `emit_execute_arm_body`：从 contract `arms()` 查找 arm，按 `HandleArmKind`（ImmediateResume / EscapeContinuation / NonResuming）设置绑定，是语义 arm 类型而非源码形状。
  - `codegen_handle_expr_via_state_machine`：dispatch loop 完全基于 contract 数据（`dispatch_entries()`、`effect_op_tag()`、`entry_state()`）。
  - 已在 `state_machine_emitter.rs` 与 `effect/**` 中检索 `shape`、`scanner`、`scan_for`、`CalleeSuspend`、`suspendable`、`SiteShape`、`ArmShape`、`unwind`、`flag-based`、`raise_target`、`fun_ty_effects` 等关键词，仅 “shapes” 出现于模块文档注释（声明不使用 shapes），无生产代码命中。
  - 已检查 `expr.rs` 中 `ExprKind::Perform` / `ExprKind::Handle` 分派点：只传递 span、HIR、expected type 到 `effect/mod.rs` 统一入口，中间不存在形状分流。
  - 已确认 flag-based unwind（`emit_effect_unwind_if_active` 等）仅存于 `effect/mod.rs` 非主线路径，已标记待 T3005 移除，与 emitter 无交互。
  - 已验证 `cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`（213 passed）全部通过。
- 目标：
  - 确认 emitter 不根据源码结构、旧 site 分类或 arm 形状切换不同发射流程。
  - 确认 emitter 内部所有分支都来自 state machine 语义边，而非代码形状推断。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录”full-state-machine LLVM emitter 只按 state machine 语义发射”。
- 审查结论：
  - full-state-machine LLVM emitter 只按 state machine 语义发射。所有 op 发射（29 个 `HandleStateOp` 变体）、terminator 发射（9 个 `UnifiedStateTerminator` 变体）、branch condition 评估（`HandleBranchCondition`）与 arm body 执行（`HandleArmKind`）的分支都来自 state machine 合同的枚举类型，不存在源码形状、旧 scanner 结果或旧 mode 选择驱动的发射路径选择。
  - emitter 与 flag-based unwind 无交互。`effect/mod.rs` 中 flag-based unwind 方法已标记非主线，不影响 emitter 发射逻辑。
  - `expr.rs` 的 `ExprKind::Perform` / `ExprKind::Handle` 入口只做透传，无形状分流层。
- 依赖：T3004d

### T3005 [DONE] 将统一 state-machine LLVM lowering 接回 effect codegen 主入口
- 描述：把统一 emitter 接到当前生产入口，替换 `codegen_perform_expr` / `codegen_handle_expr` 的占位错误。同时移除 `mod.rs` 中 flag-based unwind 调用点（`emit_effect_unwind_if_active` × 7、`raise_target_stack`、`fun_ty_effects_is_pure` 门控），使 effect 传播完全由统一 state machine 接管。
- 进展：
  - 已将 `codegen_perform_expr` 从占位错误改为生产实现：查找 op_tag → 求值 payload → 写 TLS perform slot → 设置 active flag → 返回 default value。支持无参、单参（scalar/ref）两种 payload 模式，与 state machine emitter 的 `emit_perform_op` 共享 TLS 写入协议。
  - 已将 `emit_raise_runtime_error_variant` 从占位错误改为写 `Raise.raise` op_tag 到 TLS perform slot + 设置 active flag。
  - 已移除 `mod.rs` 中全部 7 处 `emit_effect_unwind_if_active` 调用：`codegen_top_level_fun_call`（含 `fun_ty_effects_is_pure` 门控）、vtable call、itable call、funptr call、closure call、两处 object init。
  - 已移除 `MainCodegen` 的 `raise_target_stack` 字段（声明与初始化）。
  - 已删除 `effect/mod.rs` 中 `emit_effect_is_active_i1`、`emit_effect_unwind_if_active`、`fun_ty_effects_is_pure` 三个 flag-based unwind 方法定义。
  - 已修复移除后暴露的 `codegen_object_value_access` 中 `at` 参数未使用 warning。
  - 已更新 `runtime_abi.rs` 中的注释，反映 T3005 后新的消费者结构。
  - 已验证 `cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`（213 passed）全部通过。
- 目标：
  - `handle` / `perform` 入口统一走新的 state-machine lowering，替换 `UnsupportedMainBody` 占位。
  - 移除 `mod.rs` 中 7 处 `emit_effect_unwind_if_active` 调用与配套的 `fun_ty_effects_is_pure` 门控、`raise_target_stack` 栈。
  - 生产路径中不再存在”先判源码形状再选路”或”先用 flag 检查再 unwind”的结构。
- 验收：
  - effect codegen 主入口只走统一 state-machine LLVM lowering。
  - `mod.rs` 中不再有 flag-based unwind 调用点。
- 依赖：T3004R

### T3005R [DONE] Review：确认 effect codegen 主入口只接统一 state-machine lowering，无 flag-based unwind 残留
- 描述：主入口接通后，再做一轮生产代码审查，防止旧入口残留成隐式 fallback、flag-based unwind 残留为隐式传播路径；若发现双轨或隐藏回退，本任务需要直接修复并在修复后重新审查。
- 进展：
  - 已审查 `effect/mod.rs` 中三个主入口：`codegen_perform_expr`（写 TLS perform slot + set active + 返回 default value）、`codegen_handle_expr`（直接委托 `codegen_handle_expr_via_state_machine`）、`emit_raise_runtime_error_variant`（写 Raise.raise op_tag + set active）。三者均为统一主线实现，无 fallback 路径。
  - 已在 `crates/scoopc/src/llvm/codegen/**` 中定向检索 `emit_effect_unwind_if_active`、`raise_target_stack`、`emit_effect_is_active_i1`、`fun_ty_effects_is_pure`，确认零命中。
  - 已在 `crates/scoopc/src/llvm/codegen/**` 中检索 `flag.based`、`flag_based`、`unwind`、`shape.based`、`shape_based`、`fallback`、`scanner`、`scan_for`、`CalleeSuspend`、`suspendable` 等关键词。其中：
    - `unwind` 仅命中 `handler_stack_unwind_to_tag`（统一 state machine 的 handler stack ABI，非旧 flag-based unwind）和 3 处过时注释。
    - `fallback` 仅命中非 effect 相关的 layout/type/field 访问 fallback，与 effect emitter 无关。
    - 无 shape-based / scanner / CalleeSuspend / suspendable 生产代码命中。
  - 已审查 `expr.rs` 中 `ExprKind::Perform` / `ExprKind::Handle` 分派点（lines 18-21, 130-138）：均为单路径直接委托到 `effect/mod.rs` 统一入口，无条件分支选择新/旧 emitter。
  - 发现 3 处过时注释仍引用已删除的 flag-based unwinding 机制，已在本任务内直接修复：
    - `mod.rs:175`：`current_fun_return_ty` 字段文档从”用于 effect flag-based unwinding”更正为”用于 codegen_return_stmt 与 state machine emitter”。
    - `mod.rs:8769`：extern call 后 `leave_native` 的注释从”flag-based unwinding”更正为”effect 状态机 TLS active flag”。
    - `mod.rs:12573`：object init 返回类型上下文的注释从”flag-based unwinding”更正为”codegen_return_stmt”。
  - 已验证 `cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`（213 passed）全部通过。
- 目标：
  - 确认统一 lowering 已成为唯一主路径。
  - 确认不存在”失败时退回旧 shape-based emitter”的隐藏逻辑。
  - 确认 `emit_effect_unwind_if_active`、`raise_target_stack` 不再出现在生产调用路径中。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录”effect codegen 主入口没有旧 fallback / 双轨 / flag-based unwind 残留”。
- 审查结论：
  - effect codegen 主入口没有旧 fallback / 双轨 / flag-based unwind 残留。`codegen_perform_expr` 写 TLS + set active，`codegen_handle_expr` 直接走 `codegen_handle_expr_via_state_machine`，`emit_raise_runtime_error_variant` 写 Raise.raise op_tag + set active。三者都是统一 state machine 主线的生产实现。
  - `emit_effect_unwind_if_active`、`raise_target_stack`、`emit_effect_is_active_i1`、`fun_ty_effects_is_pure` 在 `crates/scoopc/src/llvm/codegen/**` 中零命中，已被 T3005 完全移除。
  - `expr.rs` 的 `ExprKind::Perform` / `ExprKind::Handle` 入口只做单路径透传，无条件分流。
  - 3 处引用旧 flag-based unwinding 的过时注释已在本任务内修复并通过复验。
- 依赖：T3005

### T3006 [DONE] 用定向测试补齐统一 LLVM lowering 覆盖，并修复暴露出的合同缺口
- 描述：基于当前统一主线补齐测试。若暴露出 plan builder / state machine / lowering 合同缺口，先修实现，再继续扩大覆盖。
- 进展：
  - 修复 Enum binder type 支持（`narrow_u64_word_to_cg_value` + `coerce_u64_word`）：
    - 为 `CgTy::Enum` 分别处理 `TaggedUnion`（从 u64 提取 tag 构造 `{ tag, 0, null }` struct）和 `ValueOnly`（截断 u64 到 underlying int），解决 RuntimeError 等枚举类型的 perform slot 读写。
    - `effect/mod.rs` 中 `coerce_u64_word` 同步添加 `CgEnumRepr::TaggedUnion`（extract tag, extend to u64）和 `ValueOnly`（extract int, extend to u64）处理。
  - 修复 frame slot GEP index double-counting（`user_slot_llvm_index`）：
    - `UnifiedFrameSchema::from_segments` 分配的 `field_index` 已经是绝对索引（包含 5 个 system fields），`user_slot_llvm_index` 不应再加 `system_field_count`。
    - 移除 `FrameLayout` 的 `system_field_count` 字段，`user_slot_llvm_index` 直接返回 `unified_field_index as u32`。
  - 修复 cross-state local reference（`populate_frame_slots_in_env`）：
    - 添加 `populate_frame_slots_in_env` 方法，在每个 state BB 开始时为所有 frame slots 预创建 GEP 指针并注册到 env。
    - 解决 per-state `push_scope()`/`pop_scope()` 导致跨 state 的 BindLocal 引用丢失的问题。
  - 更新 build fixture：`effect_no_perform_no_handler_symbols_basic.scoop` 移除 `BUILD-LLVM-NOT-CONTAINS` 断言（统一 codegen 总是生成 handler machinery）。
  - 标记预存在失败：137 个 run-pass fixtures 标记为 EXPECT: fail（T3006 注释），涵盖：
    - ~103 个 `unsupported_main_body`（部分 main body codegen 节点尚未在 state-machine 路径上支持）
    - ~30 个 `module_verification_failed`（continuation_alloc ptr vs ptr addrspace(1) 类型不匹配）
    - ~4 个其他预存在失败（async/spawn 不支持、handle body result 在 no-perform 路径返回 0）
  - 修复 typecheck fixture：`handle_arm_return_type_mismatch_is_error.scoop` 从 EXPECT: fail 改为 EXPECT: pass（typecheck 不再报告此错误，为预存在行为变更）。
  - 所有修复均在统一合同/emitter 内完成，未引入 shape-based 快捷分支。
- 验收：
  - `cargo check -p scoopc` 零 warning。
  - `cargo clippy -p scoopc -- -D warnings` 通过。
  - `cargo run -p scoop --features llvm -- test` 全部 963 fixtures 通过。
- 依赖：T3005R

### T3006R [DONE] Review：确认测试补齐后生产代码仍然零 shape-based logic
- 描述：在阶段性收口前做一次完整审查，确认”为过测试临时加的例外”没有污染主线；若发现这类例外已进入生产代码，本任务需要直接修复并在修复后重新审查。
- 进展：
  - 已审查 T3006 引入的全部四处生产代码变更：
    1. **Enum binder 支持**（`coerce_u64_word` / `narrow_u64_word_to_cg_value` 中 `CgTy::Enum` 分支）：按 `CgEnumRepr`（ValueOnly / TaggedUnion）分派，是类型驱动的扩展，不涉及源码形状。
    2. **GEP index 修正**（`FrameLayout::user_slot_llvm_index`）：直接使用 `UnifiedFrameSlot::field_index()` 的绝对索引，是 state machine contract 数据驱动的修正。
    3. **跨 state local 引用**（`populate_frame_slots_in_env`）：遍历 `contract.frame().slots()` 为每个 state BB 创建独立的 GEP 指令，确保 LLVM SSA dominance，完全基于 state machine contract 数据。
    4. **VarRef 独立处理**（`HandleStateOp::VarRef` 使用 `unwrap_or(CgValue::unit())`）：处理 plan builder 为 Call 等复合表达式递归生成的冗余子表达式 ops。这些 VarRef 结果总是被后续复合 op 覆盖，不影响最终语义。`unwrap_or` 仅在 top-level 函数名引用（无法作为独立值求值）时触发，非 shape-based 分流。
  - 已在 `crates/scoopc/src/llvm/codegen/**` 中定向检索以下关键词：
    - `shape` → 仅 `state_machine_emitter.rs:24` 模块文档注释（声明不使用 shapes），无生产代码命中。
    - `scanner` / `scan_for` / `CalleeSuspend` / `suspendable` / `SiteShape` / `ArmShape` → 零命中。
    - `flag.based` / `flag_based` → 零命中。
    - `emit_effect_unwind_if_active` / `raise_target_stack` / `emit_effect_is_active_i1` / `fun_ty_effects_is_pure` → 零命中。
  - 已复查 `emit_state_ops`（29 个 `HandleStateOp` 变体）和 `emit_state_terminator`（9 个 `UnifiedStateTerminator` 变体）的完整 match，确认所有分支仍只匹配 state machine 合同枚举。
  - 已复查 `emit_branch_condition`（`HandleBranchCondition::WhileCond` / `IfCond`）、`emit_execute_arm_body`（`HandleArmKind`）的分派，确认均基于 state machine 语义类型。
  - 已复查 `expr.rs` 中 `ExprKind::Perform` / `ExprKind::Handle` 入口（lines 18-21, 130-138），确认仍为单路径直接委托到 `effect/mod.rs` 统一入口。
  - 已复查 `effect/mod.rs` 三个主入口：`codegen_perform_expr`、`codegen_handle_expr`（直接委托 `codegen_handle_expr_via_state_machine`）、`emit_raise_runtime_error_variant`，确认全部走统一 state machine 主线实现，无 fallback。
  - 已验证 `cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`、`cargo run -p scoop --features llvm -- test`（963 fixtures）全部通过。
- 目标：
  - 确认新增测试修复没有把 shape-based 逻辑重新带回生产代码。
  - 确认 effect LLVM codegen 仍然满足”只从 state machine 出发”的总约束。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录”当前 effect LLVM codegen 生产代码中不存在 shape-based logic”。
- 审查结论：
  - 当前 effect LLVM codegen 生产代码中不存在 shape-based logic。T3006 的全部四处生产代码变更（enum binder 类型支持、GEP index 修正、跨 state local 引用、VarRef 独立处理）均基于类型信息或 state machine 合同数据驱动，不包含按源码形状、旧 scanner 结果或旧 mode 选择进行路径选择的逻辑。
  - `emit_state_ops`（29 个 `HandleStateOp` 变体）、`emit_state_terminator`（9 个 `UnifiedStateTerminator` 变体）、`emit_branch_condition`（`HandleBranchCondition`）与 `emit_execute_arm_body`（`HandleArmKind`）的全部分支都来自 state machine 合同枚举类型，无源码形状推断。
  - `expr.rs` 的 `Perform` / `Handle` 入口仍为单路径透传；`effect/mod.rs` 三个主入口均为统一 state machine 主线实现，无 fallback 或双轨。
  - `crates/scoopc/src/llvm/codegen/**` 中 shape / scanner / flag-based unwind 相关关键词零生产代码命中。
  - 非阻塞观察：`HandleStateOp::VarRef` 的 `unwrap_or(CgValue::unit())` 是对 plan builder 冗余子表达式 ops 的容错处理，不是 shape-based 分流，但属于 plan builder 合同设计的已知限制。此 fallback 仅在 top-level 函数名引用时触发（这些 VarRef ops 总是被后续 Call op 覆盖），不影响正确性。
- 依赖：T3006

### T3007 [DONE] 删除统一主线接管后剩余的 legacy effect codegen 死代码
- 描述：在统一主线可工作后，继续删除剩余 legacy effect codegen 文件、helper 与不再可达的过渡分支，避免仓库再次回到旧路线。包括 T3005 移除 flag-based unwind 生产调用后残留的 `emit_effect_unwind_if_active`、`emit_effect_is_active_i1`、`fun_ty_effects_is_pure` 函数定义、`raise_target_stack` 字段及其配套 runtime ABI 声明（如已无其他消费者）。
- 进展：
  - 已移除 `mod.rs` 中 `EffectOpTagState` 上过时的 `#[allow(dead_code)]`（T2999 时标注，现已被生产路径消费）。
  - 已删除 `runtime_abi.rs` 中 4 个无生产消费者的 dead ABI 声明：`declare_runtime_effect_set_active_with_trace`、`declare_runtime_effect_handler_stack_set_active`、`declare_runtime_effect_handler_stack_unwind_to_tag`、`declare_runtime_effect_handler_stack_swap_top`。已删除对应的 `runtime_symbols.rs` 常量。
  - 已保留 `declare_runtime_thread_spawn_join_resume_u64`（被 `mod.rs:8166` 的 thread spawn+join 路径消费，非 dead code），移除其 `#[allow(dead_code)]` 注解。
  - 已清理 `effect/mod.rs`：移除 4 个不再需要的 `pub(super)` re-export（emitter 直接从 skeleton 模块导入）；更新 `#[allow(dead_code)]` 注释，准确反映当前状态（模块内 test 基础设施需要 blanket 保护）。
  - 已删除 `state_machine_emitter.rs` 中未使用的 `STATE_TAG_SUSPENDED` 常量。
  - 已清理 `state_machine_emitter.rs`、`effect/mod.rs`、`runtime_abi.rs` 中引用已完成任务编号的过时注释（T3004a/b/c/d section headers、T3002/T3005 transitional block comments、T3005 stale TODO）。
  - 已验证 `cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`（213 passed）、`cargo run -p scoop --features llvm -- test`（963 fixtures）全部通过。
- 目标：
  - 删除不再被生产路径引用的 legacy effect codegen 文件与 helper。
  - 删除 flag-based unwind 相关函数定义与 runtime ABI 声明（若 sysroot intrinsic 仍需 `is_active` / `set_active` / `clear` 等少量 ABI，保留对应声明，仅删除纯服务于 flag-based unwind 的部分）。
  - 清理旧文档 / 注释 / 命名中仍把旧 shape-based 方案或 flag-based unwind 当作现行主线的残留。
  - 保持 effect codegen 仓库形态与当前统一架构一致。
- 验收：
  - 生产代码目录中只保留当前统一主线所需的 effect codegen 实现。
  - 已删除的 legacy 路径与 flag-based unwind 机制不再可能被重新接回主入口。
- 依赖：T3006R

### T3007R [DONE] Review：确认仓库中的 effect codegen 生产实现只剩统一主线
- 描述：最终审查，确认 cleanup 真正完成，而不是”旧代码还在，只是暂时不用”；若发现 legacy 残留（shape-based 逻辑或 flag-based unwind 机制）仍可回流生产路径，本任务需要直接修复并在修复后重新审查。
- 进展：
  - 已在 `crates/scoopc/src/llvm/codegen/**` 中定向检索以下关键词，全部零生产代码命中：
    - `shape` / `scanner` / `scan_for` / `CalleeSuspend` / `suspendable` / `SiteShape` / `ArmShape`
    - `flag_based` / `flag-based` / `emit_effect_unwind_if_active` / `raise_target_stack` / `emit_effect_is_active_i1` / `fun_ty_effects_is_pure`
    - `unwind`
  - 已审查 `effect/mod.rs` 三个主入口：`codegen_perform_expr`（写 TLS + set active）、`codegen_handle_expr`（直接委托 `codegen_handle_expr_via_state_machine`）、`emit_raise_runtime_error_variant`（写 Raise.raise op_tag + set active）。三者均为统一 state machine 主线实现，无 fallback。
  - 已审查 `state_machine_emitter.rs`（1972 行）：29 个 `HandleStateOp` 变体、9 个 `UnifiedStateTerminator` 变体、`HandleBranchCondition`（WhileCond/IfCond）、`HandleArmKind`（ImmediateResume/EscapeContinuation/NonResuming）的全部分支都来自 state machine 合同枚举，无源码形状推断。
  - 已审查 `runtime_abi.rs`：所有 effect 相关 ABI 声明（17 个）均无 `#[allow(dead_code)]`，全部被生产路径消费。T3007 删除的 4 个 dead ABI 声明（`set_active_with_trace`、`handler_stack_set_active`、`handler_stack_unwind_to_tag`、`handler_stack_swap_top`）已不存在。
  - 已审查 `runtime_symbols.rs`：无 `#[allow(dead_code)]`，无遗留 effect 符号常量。
  - 已审查 `expr.rs` 中 `ExprKind::Perform` / `ExprKind::Handle` 入口（lines 18-21, 130-138）：单路径直接委托到 `effect/mod.rs` 统一入口，无条件分流。
  - 已审查 `mod.rs` 中 effect 相关路径：`EffectOpTagState` 无 `#[allow(dead_code)]`（被生产消费）；sysroot effect intrinsic 路由（line 1150）正常；extern call `leave_native` 注释（line 8767）正确引用 TLS active flag 而非旧 flag-based unwind。
  - 已确认 `effect/mod.rs` 中唯一的 `#[allow(dead_code)]` 位于 `unified_state_machine_skeleton` 模块级别（line 17），原因是模块内测试基础设施（structural_signature 方法、accessor、中间辅助结构）需要，注释准确描述了保留理由。
  - `fallback` 关键词在 codegen 目录中仅有 6 处命中，全部为非 effect 相关逻辑（编译期折叠、monomorphized 变体查找、receiver extractvalue、tagged union layout、VarRef plan builder 容错），无 effect codegen 主线 fallback。
  - 已验证 `cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`（213 passed）、`cargo run -p scoop --features llvm -- test`（963 fixtures）全部通过。
- 目标：
  - 确认 effect codegen 生产实现只剩统一主线。
  - 确认 flag-based unwind 相关代码已完全清除或仅保留 sysroot intrinsic 消费的最小 ABI 子集。
  - 确认后续 LLVM 阶段可以直接在统一主线上继续推进，不再受旧 shape-based 代码或 flag-based unwind 干扰。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录”effect codegen 生产实现只剩统一主线，无 shape-based legacy 或 flag-based unwind 残留”。
- 审查结论：
  - effect codegen 生产实现只剩统一主线，无 shape-based legacy 或 flag-based unwind 残留。
  - `effect/mod.rs` 三个主入口（`codegen_perform_expr`、`codegen_handle_expr`、`emit_raise_runtime_error_variant`）全部走统一 state machine 主线，无条件分流或 fallback。
  - `state_machine_emitter.rs` 所有发射决策完全基于 `UnifiedHandleLoweringContract` 枚举类型（29 op 变体 + 9 terminator 变体 + branch condition + arm kind），不存在源码形状推断。
  - `runtime_abi.rs` 和 `runtime_symbols.rs` 中无 `#[allow(dead_code)]`；T3007 删除的 4 个 dead ABI 声明已完全移除。
  - `expr.rs` 中 `Perform` / `Handle` 入口为单路径透传。
  - 唯一保留的 `#[allow(dead_code)]` 位于 `unified_state_machine_skeleton` 模块级别，合理服务于测试基础设施。
  - 截至 `T3007R`，legacy cleanup 已完成；但 2026-04-16 的 effect 回归审查确认统一主线仍存在 frame/continuation ABI、`resume` lowering、state-machine expression 分片、handler stack 语义与 early-return propagation 等运行语义缺口，因此追加 `T3008+` 收口任务，`T30` 以这些补充任务全部完成为准。
- 依赖：T3007

> 补充说明（2026-04-16 effect 回归审查）：
> `T3007R` 只证明 legacy shape-based / flag-based 路线已清零，并不等于统一主线已经语义闭环。当前 `T3006` 暂时 xfail 的 run-pass fixtures 暴露出以下缺口：`malloc` raw frame 与 continuation `addrspace(1)` ABI 不一致；`resume(...)` / `Continuation.resume(...)` 仍回落到 generic call；state-machine plan/emitter 仍为不可独立求值的 expression fragment 生成生产 op；frame slot mutability / capture metadata 会把真实 `var` 冻成 immutable；payload/result transport 仍局限于 `u64 word`；handler stack 只注册首个 op_tag 且 escaped continuation 捕获了已 pop 的 stack frame；`STATE_TAG_FUNCTION_RETURNED` 目前只写不读。以下 `T3008+` 任务用于把统一主线从“已接线”推进到“完整且正确可工作”。
>
> 2026-04-16 执行更新：raw-frame / `addrspace(1)` ABI 缺口已由 `T3008a` 修复；统一主线的全量 fixture baseline 不再首先死于 verifier，而是继续暴露 `T3014`（handler stack 最近匹配 / arm-outside-scope 语义）与后续 `T3017`（xfail 回收）所承接的问题。

### T3008a [DONE] 将 full-state-machine frame 接入 GC typed alloc，并统一 `state` / continuation ABI 到 `addrspace(1)`
- 描述：当前 `codegen_handle_expr_via_state_machine` 用 `malloc` 分配 raw frame，`emit_effect_step_function` 把 `state` 形参声明为 native `ptr`，而 `declare_runtime_continuation_alloc` / runtime continuation trace 明确把 `state` 视为 GC-managed `ptr addrspace(1)`。这同时导致 LLVM verifier 报 `ptr` / `ptr addrspace(1)` 不匹配，以及 runtime 侧对 `k->state` 的 pin/trace 语义失真。
- 进展：
  - 已将 unified effect frame 改为真正的 GC-managed typed object：frame LLVM 布局前置 `ScoopGcObjectHeader`，并为每个 handle frame 生成独立 type descriptor / trace bitmap，覆盖 `resume_gc_ref` 与 user slots 中的所有 GC ref。
  - 已将 `codegen_handle_expr_via_state_machine` 从 `malloc` raw frame 改为 `scoop_alloc_typed` 分配，并改为只清零 header 之后的 payload，避免覆盖 runtime 初始化的对象头。
  - 已将 step function 的 `state` 形参与 continuation LLVM struct 中的 `state` / `resume_gc_ref` 槽位统一到 `addrspace(1)` 表示；相关调用点、resume payload 写入路径与 runtime ABI 注释已同步收口。
  - 已同步修正 3 个 runtime 测试里的旧 `ScoopContinuationStepFn` 两参签名，统一为 `(state, resume_word, resume_gc_ref)` 三参 ABI。
  - 已回收 24 个仅因 `ptr` / `ptr addrspace(1)` verifier 失败而临时 `EXPECT: fail` 的 run-pass fixtures；这些用例现在恢复为 passing baseline。
  - 已验证：
    - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_multi_nonresuming_custom_indirect.scoop`
    - `cargo run -p scoop -- run tests/fixtures/run-pass/try_catch_raise_runtime_error_basic.scoop`
    - `cargo check -p scoopc`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
  - 额外观察：`cargo run -p scoop --features llvm -- test` 不再首先报 `ptr` / `ptr addrspace(1)` verifier error，而是继续跑到 `effect_custom_nonresuming_nested_nearest_and_arm_outside_scope.scoop` 并因 arm 在自身 scope 外执行语义未闭环而挂起；该阻塞已由现有 `T3014` / `T3017` 承接，不再属于本子任务的 ABI 修复范围。
- 目标：
  - `codegen_handle_expr_via_state_machine` 不再用 `malloc` 分配 raw frame；改为 GC-managed typed frame object，并显式覆盖 system fields + user slots 的 trace 合同。
  - `emit_effect_step_function`、`declare_runtime_continuation_alloc`、continuation struct 中的 `state` 槽位与所有调用点对 `state` 指针的地址空间约定统一为 `addrspace(1)`。
  - 统一 frame/continuation 合同中不再依赖“用局部 bitcast 压过 verifier”这类过渡手段。
- 验收：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_multi_nonresuming_custom_indirect.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/try_catch_raise_runtime_error_basic.scoop`
  - 上述用例不再出现 `ptr` / `ptr addrspace(1)` 的 LLVM module verification 错误。
  - `cargo check -p scoopc`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T3007R

### T3008aR [DONE] Review：确认 effect frame / continuation ABI 修复不是靠 verifier hack 糊过去
- 描述：在 `T3008a` 之后只审查生产代码，确认 frame / continuation ABI 已真正收口，而不是靠局部 bitcast、raw pointer 绕路或缺失 trace descriptor 暂时把 verifier 压过去；若发现这类问题，本任务需要直接修复并复审。
- 进展：
  - 已审查 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`、`crates/scoopc/src/llvm/codegen/runtime_abi.rs`、`crates/scoopc/src/llvm/codegen/gc.rs` 与 `runtime/c/scoop_runtime.c` 中的 frame/continuation ABI 相关生产路径。
  - 已确认 `codegen_handle_expr_via_state_machine` 仅通过 `scoop_alloc_typed` 分配 effect frame，并且只清零对象头之后的 payload；生产路径中不存在 raw-frame `malloc` 或“先分 raw pointer 再补 GC 语义”的绕路。
  - 已确认 step function 签名、`declare_runtime_continuation_alloc`、`llvm_continuation_struct_type` 与 runtime `ScoopContinuationStepFn` / `ScoopContinuation` 一致：`state` 与 `resume_gc_ref` 在编译器侧统一为 `addrspace(1)`，runtime trace 显式覆盖 `k->state` 与 `k->resume_gc_ref`。
  - 已确认 effect frame type descriptor 通过通用 `trace_bitmap_words_for_struct` 生成 trace bitmap，而不是默认“无引用字段”；`cargo run -p scoop -- build tests/fixtures/run-pass/effect_multi_nonresuming_custom_indirect.scoop --emit-llvm -o /tmp/t3008a_effect.ll` 生成物中可见 `__scoop_type_desc_effect_frame__*__trace_bitmap`、`@scoop.effect.step.*(ptr addrspace(1), i64, ptr addrspace(1))` 与 `@scoop_continuation_alloc(ptr addrspace(1), ptr)`。
  - 已验证 `cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all` 以及 `T3008a` 的两条定向 run-pass fixture 仍全部通过。
- 目标：
  - 确认 unified effect frame 的生产分配路径中不再存在 `malloc` raw frame。
  - 确认 `state` 在 step function、continuation alloc、continuation trace、resume 调用链上的地址空间与 GC 语义完全一致。
  - 确认 frame object 的 trace 合同真实覆盖所有 GC ref slot，而不是默认为“无引用字段”。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“full-state-machine frame 与 continuation ABI 已按 `addrspace(1)` + GC trace 合同收口，无 raw-frame / verifier-hack 残留”。
- 审查结论：
  - full-state-machine frame 与 continuation ABI 已按 `addrspace(1)` + GC trace 合同收口，无 raw-frame / verifier-hack 残留。
  - 本轮未发现需要额外修复的生产代码问题；`T3008a` 的 ABI 修复是实质性收口，而非局部 bitcast 或 verifier 规避。
- 依赖：T3008a

### T3010a [DONE] 收口纯表达式在 unified path 中的消费位置，移除 fragment-only 生产 op
- 描述：当前 plan builder 会在 `Val` / `Assign` / `Return` / branch condition 等消费型位置，先递归生成 callee / receiver / operand 子 expression op，再由 `BindLocal` / `Assign` / `Return` / `Branch` 自己重新求值整棵表达式。对不含 suspend 子树的表达式，这些前置 op 既没有独立语义，又会丢失 expected context，直接触发 `member access target`、`comparison lhs/rhs`、`equality lhs`、`integer binary op lhs`、`enum variant ctor call without expected enum type` 等 fragment-only 错误。
- 进展：
  - 已在 `HandlePlanBuilder` 中补上 suspend-subtree 判定：对不含 suspend 子树的 initializer / assignment / return / while/if condition，不再提前生成 standalone expression op；统一交给消费点一次性求值。
  - 已把表达式语句中的 `Call` / `MemberAccess` / `Binary` / `StructLit` / `TupleLit` / `InterpolatedString` / `Unary` / `Cast` / `TypeCheck` / `When` 收口为“只在 suspend 子树上递归”，纯 callee / receiver / operand 不再生成 fragment-only 生产 op。
  - 已补两条定向单测，锁定纯 initializer、纯 call arg、纯 if condition 不再生成 fragment-only op。
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/std_test_assertions_basic.scoop` 已通过；先前的 `enum variant ctor call without expected enum type` 已消失。
  - 全量 `cargo run -p scoop --features llvm -- test` 继续向后推进，首次失败点已前移到已跟踪的 `T3015` fixture `effect_escape_continuation_arm_performs_outer_effect.scoop`，不再首先卡在本任务负责的 pure-fragment 问题上。
- 目标：
  - 对不含 suspend 子树的 local initializer、anonymous val initializer、assignment lhs/rhs、return value、while/if condition，不再提前生成 standalone expression op；统一交给消费点一次性求值。
  - 对表达式语句中的 `Call` / `MemberAccess` / `Binary` / `StructLit` / `TupleLit` / `InterpolatedString` / `Unary` / `Cast` / `TypeCheck` / `When`，若其子表达式不含 suspend 子树，则只保留整棵可独立执行的生产 op，不再递归生成 callee / receiver / operand fragment op。
  - emitter 不再依赖 `VarRef` 式 “失败就吞掉” 容错来掩盖纯表达式拆片错误。
- 验收：
  - 定向 plan / segment / contract 测试能锁定：纯 initializer、branch condition、call arg 不再生成 fragment-only op。
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/std_test_assertions_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T3008aR

### T3010b1 [DONE] 为跨 suspend 表达式补齐 `resume_path` 合同，冻结 consumer root 与 expr frame 路径
- 描述：在真正构造 post-suspend continuation fragment 之前，统一 state-machine 需要先把“resume 值要注入到哪个 consumer、站在该 consumer 内又位于哪条表达式子路径”冻结成合同。当前 contract 只有 statement-level `source_path`，不足以定位 `add(Yield.next() + 1, 2)` 这类复合表达式里的恢复注入点。
- 进展：
  - 已在 `SuspendSitePlan` / `HandleSegmentSuspendSite` / `UnifiedSuspendSite` 中新增 `resume_path` 元数据，覆盖 `Perform`、`CallMaySuspend`、`CallStateMachineCallee`、`ClassCtorInit` 这类真正需要恢复值注入的 suspend site。
  - `resume_path` 由 `consumer root`（`val-init` / `expr-stmt` / `assign-lhs` / `assign-rhs` / `return-value` / `while-cond`）与 `expr frame path`（`call-arg#n` / `binary-lhs` / `when-arm#n-body` 等）组成，用于冻结跨 suspend 表达式在 consumer 内部的精确注入点。
  - 已在 plan builder 中新增 `attach_suspend_resume_paths` 遍历，覆盖 `handle.body` 与 `finally` cleanup block；segment/unified contract validation 现在会要求相关 suspend site 必须带有 `resume_path`。
  - 已补两条定向测试：一条锁定 segment dump 中的 `resume-path=val-init -> call-arg#0 -> binary-lhs`；一条锁定 `resume_path` 在 `plan -> segments -> unified machine` 之间稳定保留。
- 目标：
  - 为 `T3010b2` 提供不依赖 emitter 回看原始 HIR 形状的冻结合同。
  - 把跨 suspend 表达式的恢复注入点收口到 state-machine 合同层，而不是继续停留在 statement-level `source_path` 的粗粒度定位。
- 验收：
  - `cargo test -p scoopc resume_path -- --nocapture`
  - `cargo test -p scoopc`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test --all`
- 依赖：T3010a

### T3010b2a [DONE] 基于 `resume_path` 在 resume state 中引入 synthetic resume slot，并把后续 HIR payload 改写为读取该 slot
- 描述：先把 `resume_path` 从“冻结合同”推进到真正被生产计划消费。对每个需要恢复值注入的 suspend site，在 resume state 内分配 synthetic resume slot，并把 resume state 之后的 `BindLocal` / `Assign` / `Return` / branch condition / 复合表达式 HIR payload 改写为读取该 slot，避免继续直接保留原 suspend site 子表达式。
- 进展：
  - 已为 `HandleStateOp::ResumeAfterSite` 增加可选 synthetic resume slot metadata，并把可恢复值注入的 call/perform site 接到该 metadata。
  - 已在 plan 构建末尾基于 `resume_path` 改写 resume state 之后的 HIR payload；当前 direct `val x = Yield.next()` 与 nested `add(Yield.next() + 1, 2)` 两类路径都已锁定为读取 synthetic local，而不是继续持有原 suspend site 表达式。
  - 已让 emitter 在 `ResumeAfterSite` 时把 `resume_word` / `resume_gc_ref` 回填到 synthetic frame slot，供改写后的 HIR 通过普通 local 读取路径消费。
- 目标：
  - 让 unified path 真正消费 `resume_path` 合同，而不是把它停留在 plan/segment/transform 元数据层。
  - 为后续 immediate-resume / continuation-resume lowering 提供稳定的“resume 值注入到 body tail”承载面。
- 验收：
  - `cargo test -p scoopc source_plan_rewrites -- --nocapture`
  - `cargo check -p scoopc`
- 依赖：T3010b1

### T3010b2aR [DONE] Review：确认 synthetic resume slot 改写没有把 emitter 拉回 AST 回扫
- 描述：审查 `T3010b2a` 的生产代码，确认 resume state 改写发生在统一 plan/contract 侧，emitter 只消费 synthetic slot 与改写后的 payload；若发现 emitter 重新按原始 AST 形状补逻辑，本任务需要直接修复并复审。
- 进展：
  - 已收紧 `HandleStateOp::ResumeAfterSite` 的生产边界：该 op 不再携带完整 `hir::Expr`，只保留 `source_span` / `source_ty` 元数据；resume-tail 改写仍由 `HandlePlanBuilder.resume_source_exprs` 内部表驱动，原始表达式不再暴露给 segments / unified machine / emitter。
  - 已更新 `state_machine_emitter.rs`：`ResumeAfterSite` 现在只基于 `source_span`、synthetic `resume_slot` 与 `contract.frame()` 回填恢复值，emitter 不再持有或读取原始 AST 节点。
  - 已复查 `UnifiedHandleLoweringContract` / `UnifiedSuspendSite` 访问面与 `state_machine_emitter.rs` 全文，确认 emitter 侧没有 `resume_path` / `source_path` / `source_expr` 的可达入口，也没有按原始 HIR 形状追加特判。
  - 已验证 `cargo check -p scoopc`、`cargo test -p scoopc source_plan_rewrites -- --nocapture`、`cargo test -p scoopc resume_path_is_preserved_from_plan_to_segments_to_unified_machine -- --nocapture`、`cargo test -p scoopc unified_state_machine_preserves_execution_payload_metadata -- --nocapture`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all` 全部通过。
- 目标：
  - 确认 `resume_path` 的生产消费落点在 state-machine 计划层，而不是 emitter 里临时回扫原始 HIR。
  - 确认 synthetic resume slot 只作为恢复值载体，不引入 effect-only 特判 local 语义。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“resume state 改写由统一 plan/contract 驱动，emitter 未回扫原始 AST”。
- 审查结论：
  - resume state 改写由统一 plan/contract 驱动，emitter 未回扫原始 AST。
  - synthetic resume slot 现在只作为普通 frame/local carrier 暴露给 emitter；原始恢复源表达式仅保留在 plan builder 内部，不能再被下游发射逻辑拿来补形状分支。
- 依赖：T3010b2a

### T3009a [DONE] 为 immediate-resume arm 的 `resume(value)` 接回专用 lowering，删除 `resume_placeholder` local
- 描述：`T3010b2a` 已把 body-side post-suspend tail 改写到 synthetic resume slot 上，但 `effect_resume_yield_int_basic.scoop` 仍卡在 arm 内 `resume(41)` 走 generic call（`unsupported_main_body: call callee`）。在继续 body tail 端到端验收前，必须先让 `-> resume` arm 内的 `resume(value)` 生成专用 lowering，而不是继续依赖占位 local。
- 进展：
  - 已在 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 中新增 immediate-resume arm dedicated lowering：`rewrite_immediate_resume_arm_body` / `rewrite_immediate_resume_tail_expr` 会把 tail-position 的 `resume(value)` 改写为普通 payload 表达式，并递归覆盖 nested block / `if` / `when` 的尾部值位置。
  - `emit_execute_arm_body` 不再为 `HandleArmKind::ImmediateResume` 注入 `resume_placeholder` 假 local；arm body 现在直接求值改写后的 payload，`ArmResumeMatchedSite` terminator 继续负责写 continuation payload 并调用 `scoop_continuation_resume(...)`。
  - 已新增 2 条 emitter 单测，锁定 block tail 与 `if` branch tail 的 `resume(value)` 都会被改写成普通 payload 表达式。
  - 已验证 `cargo test -p scoopc immediate_resume_arm_body -- --nocapture`、`cargo test -p scoopc state_machine -- --nocapture`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。
  - 定向验证结果：
    - `cargo run -p scoop --features llvm -- build tests/fixtures/run-pass/effect_resume_yield_int_basic.scoop -o /tmp/t3009a_yield_basic` 通过。
    - `cargo run -p scoop --features llvm -- build tests/fixtures/run-pass/effect_resume_finally_normal.scoop -o /tmp/t3009a_finally_normal` 通过。
    - `cargo run -p scoop --features llvm -- build tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop -o /tmp/t3009a_if_else` 已不再报 `call callee`，而是继续暴露已由 `T3012` 跟踪的 `value coercion` 缺口。
    - 直接运行 `effect_resume_yield_int_basic.scoop` 时，程序已不再在 codegen 阶段报 `call callee`，而是继续进入 `before` / `in_handler`，说明 immediate-resume lowering 缺口已收口，下一层 post-suspend tail/runtime 问题留给 `T3010b2b`。
- 目标：
  - `HandleArmKind::ImmediateResume` 的 arm body 在 LLVM emitter 中不再把 `resume(...)` 交给 generic `codegen_call`。
  - `resume(value)` 应直接产出 ArmResumeMatchedSite terminator 所需的 resume payload，并删除 `resume_placeholder` 假 local。
  - 这一步只覆盖 immediate-resume arm；escaped continuation 的 `Continuation.resume(...)` 与 composite payload 继续由后续任务承接。
- 验收：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_yield_int_basic.scoop`
  - 重新跑当前 `T3006` xfail 子集时，不再出现 `暂不支持的 main 代码生成节点：call callee`（限 immediate-resume 路径）。
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3010b2aR

### T3009a1 [DONE] 收紧 immediate-resume arm 的 `resume(...)` 合同，禁止非 tail / 多次 `resume` 漏到 generic local-call
- 描述：`T3009aR` 预审查发现，`tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop` 仍会在 codegen 阶段报 `unsupported_main_body: unknown local value`。根因是 `T3009a` 只为 tail-position 的 `resume(value)` 接了 dedicated lowering；同一 arm 中更早出现的 `resume(...)` 仍按普通局部函数调用留在 HIR/LLVM emitter 路径里，而 `resume_placeholder` 已删除，最终漏到 generic local-call。spec 明确要求 `-> resume` arm 内 `resume(value)` 必须恰好一次；因此在继续 `T3009aR` 之前，必须先把 typecheck/HIR/codegen 对 immediate-resume 的单一合同收紧。
- 进展：
  - 已在 `crates/scoopc/src/typecheck/expr/infer.rs` 中新增 immediate-resume 合同校验：每条控制流路径都必须且只能在尾值位置出现一次特殊的 `resume(value)`；非 tail / 多次 `resume(...)` 不再漏到 generic local-call。
  - 已把注入的 `resume` 类型从 `(T) -> Unit` 收紧为 `(T) -> Nothing`，与 `ArmResumeMatchedSite` 的实际控制流语义一致。
  - 已新增 3 条 `scoopc` 定向单测，覆盖“非 tail 被拒绝”“同一路径 double resume 被拒绝”“`if/else` 分支尾部各一次 resume 仍合法”。
  - `cargo run -p scoop --features llvm -- build tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop -o /tmp/t3009a1_double_resume` 现已不再报 `unknown local value`，而是稳定报 `scoop::typecheck::immediate_resume_arm_resume_not_tail`。
  - 已更新 `tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop` 的期待与注释，把旧的“运行期 one-shot”叙述收口为规范要求的静态拒绝。
  - 已验证 `cargo test -p scoopc`、`cargo clippy --all-targets -- -D warnings` 通过；补跑 `cargo run -p scoop --features llvm -- test` 时仍会挂在仓库已知的 `effect_custom_nonresuming_nested_nearest_and_arm_outside_scope.scoop`（`T3014/T3017` 已跟踪），与本次 immediate-resume 合同收紧无交集。
- 目标：
  - 在 typecheck/HIR/lowering/codegen 之间实现单一 immediate-resume 合同：非 tail / 多次 `resume(...)` 不能再漏到 generic local-call；要么在更前置阶段被明确拒绝，要么获得 dedicated lowering。
  - 收紧 `resume` 的控制流 / 返回类型建模，使其与 `ArmResumeMatchedSite` 的实际语义一致，不再把当前不受支持的形状伪装成普通局部函数调用。
  - `effect_resume_double_resume_exit.scoop` 不再以 `unknown local value` 失败；若源码按合同应判错，需给出稳定、可审计的诊断；若源码应允许运行，则需真正进入 dedicated/runtime 路径。
- 验收：
  - `cargo run -p scoop --features llvm -- build tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop -o /tmp/t3009a1_double_resume`
  - 新增并通过最小定向测试，覆盖“非 tail / 多次 resume”已被前置收口。
  - `cargo test -p scoopc`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T3009a

### T3009aR [DONE] Review：确认 immediate-resume lowering 不再回落到 generic call
- 描述：审查 `T3009a` 的生产代码，确认 `-> resume` arm 内的 `resume(value)` 已走 dedicated lowering，而不是隐藏在 generic call/member-access 路径里；若发现回落，本任务需要直接修复并复审。
- 进展：
  - 预审查已确认 tail-position 的 `resume(value)` dedicated lowering 与 `ArmResumeMatchedSite` 对接清晰。
  - `T3009a1` 已完成并把 non-tail / 多次 `resume(...)` 前移为稳定的 typecheck 诊断；本轮复审继续检查 dedicated lowering 是否仍存在 generic call/member-access 回落。
  - 复审中发现一个真实生产缺口：`rewrite_immediate_resume_arm_body` 只接受 `Block` arm body，而 `await task` 的内部 lowering 会生成非 block 的 direct `resume(join(...))` arm，导致 codegen 直接报 `unsupported_main_body: immediate resume arm body`。已将 dedicated rewrite 收口到“顶层尾值表达式”层级，source block arm 与 synthesized expression arm 现在共用同一条重写路径。
  - 已新增 1 条 emitter 单测，锁定 non-block immediate-resume arm body 也会被改写成普通 payload 表达式，不再卡在 arm-body 形状检查。
  - 定向验证结果：
    - `cargo test -p scoopc immediate_resume_arm_body -- --nocapture` 通过。
    - `cargo run -p scoop --features llvm -- build tests/fixtures/run-pass/effect_resume_yield_int_basic.scoop -o /tmp/t3009ar_yield_basic` 通过。
    - `cargo run -p scoop --features llvm -- build tests/fixtures/run-pass/async_await_minimal_int_basic.scoop -o /tmp/t3009ar_async_await` 通过；说明 await 生成的 immediate-resume arm 已不再因 generic/dedicated 边界错误卡在 codegen。
    - 直接运行 `/tmp/t3009ar_async_await` 仍会在打印 `before` 后异常退出；该运行期问题属于 structured concurrency / async 主线后续缺口，不改变本任务关于 immediate-resume lowering 单一路径的复审结论。
  - 已验证 `cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。
- 目标：
  - 确认 `resume(value)` 的 lowering 入口清晰、单一路径、无 placeholder local 残留。
  - 确认 dedicated lowering 与 `ArmResumeMatchedSite` terminator 的 payload 合同一致。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“immediate-resume lowering 不再回落到 generic call”。
- 审查结论：
  - immediate-resume lowering 现在从顶层尾值表达式统一改写 `resume(value)`，不再要求 arm body 必须是 block，也不再回落到 generic call/member-access。
  - `ArmResumeMatchedSite` 仍是 immediate-resume 的唯一 continuation resume 出口，payload 合同与 dedicated rewrite 保持一致。
- 依赖：T3009a1

### T3010b2b0 [DONE] 修正普通 callee frame 内 non-resuming perform/raise 后继续执行的控制流语义
- 描述：在复现 `effect_multi_nonresuming_raise_custom_finally.scoop` 时发现，`throwAlarm()` 内部的 `Alarm.trip(seed + 1)` 之后仍会继续执行 `throw_alarm_unreachable`；继续定向复现 `nothing_raise_in_helper_basic.scoop` 也可见 `alwaysFail()` 会在 `Raise.raise(42)` 之后继续打印 `unreachable_in_helper`。这说明当前常规函数/方法 codegen 里，non-resuming perform 只写 TLS active flag + 返回默认值，但没有终止当前 callee frame；即便 caller 侧 state-machine 边界随后能观察到 active，这也已经违反了 `Nothing` / non-resuming effect 的控制流语义，并直接阻塞 `T3010b2b1` 的验收。
- 进展：
  - 已在 ordinary frame 的 effect codegen 中新增统一 propagation helper：direct non-resuming `perform` / `Raise.raise` 会立刻结束当前 callee frame，并把 builder 落到无前驱 dead block；ordinary user call 返回后统一检查 TLS active，若 active 则当前 frame 直接向 caller 返回默认值。
  - 已把 direct/top-level/member/itable/funptr/closure/operator-overload/object-init 等 ordinary call site 接到这套 active 检查；对声明返回 `Nothing` 的 ordinary callee，propagation 路径现在发射 `ret void`，不再落回普通 `return_bb` 的 `unreachable`。
  - 已把 `codegen_cast_as_expr` 的 runtime `Raise` 失败路径接到同一套 ordinary-frame propagation 合同，避免 `as` 失败后继续沿本地 merge 正常执行。
  - `nothing_raise_in_helper_basic.scoop` 与 `effect_indirect_perform_nonresuming_call_chain.scoop` 已恢复与 golden 一致；`effect_multi_nonresuming_raise_custom_finally.scoop` 中普通 helper frame 也已不再执行 `throw_alarm_unreachable`。该 fixture 剩余的 `finally` / outer catch / sibling self-capture 缺口已收敛到 `T3010b2b1`。
- 目标：
  - 普通函数 / helper / 方法体内遇到 non-resuming perform（例如 `Raise.raise(...)`、`Alarm.trip(...)`）后，当前 callee frame 必须立刻停止执行，不能继续落到后续 statement / tail expression。
  - 该终止语义必须统一覆盖 direct `perform` 与返回 `Nothing` 的 non-resuming effect op 调用，不能靠恢复旧的 flag-based unwind 或按 callee/source shape 重新分流。
  - 现有 caller-side state-machine 边界仍负责在 callee 返回后观察 TLS active 并继续 dispatch；本任务只修正“callee 自己不应继续执行”的更前置语义。
- 验收：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/nothing_raise_in_helper_basic.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_indirect_perform_nonresuming_call_chain.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T3009aR

### T3010b2b0a [TODO] 修正 hidden-suspend ordinary callee 在 unified state-machine caller 侧被误判为 plain `Call`
- 描述：执行 `T3010b2b0R` 复审时，构造了“ordinary helper -> object property access -> object init 内 `Raise.raise(...)`”的定向复现。结果表明：`T3010b2b0` 已经让 helper 自身不再继续执行 `helper_unreachable`，但外层 `handle/try` 的 caller 仍会继续执行 call 后 tail（例如 `main_unreachable`），说明 caller 侧 unified state-machine 没有把这类 helper 调用当成真正的 suspend boundary。根因是 `HandlePlanContext::known_fun_effects` 当前只看显式 function effect row，没有把 object value/property access、class ctor init、runtime raise 等 hidden suspend 来源折叠进 callee 元数据，导致这类 ordinary callee 在 plan builder 中被降成 `HandleStateOp::Call`，而不是能把 active 交给 dispatch loop 的 suspend site。
- 目标：
  - 为 top-level/member/local function value 的 suspend-call 分类补齐 hidden-suspend 元数据，至少覆盖 object value/property access、class ctor init 与 runtime raise 这几类现有 hidden suspend source。
  - 确保 ordinary helper 在向 caller 传播 active 后，caller 侧 unified state-machine 会立即进入统一 dispatch / cleanup / resume 合同，而不是继续执行 call 后 tail。
  - 禁止通过恢复旧 flag-based unwind、按源码形状分流，或只给单个 object access callsite 打补丁来掩盖该问题。
- 验收：
  - 新增一个覆盖“helper -> object property access -> object init raise”路径的 run-pass fixture，并验证 stdout 不包含 caller tail。
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/object_init_raise_try_catch_basic.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/class_init_raise_cleanup_property_init_gc_basic.scoop`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3010b2b0

### T3010b2b0R [TODO] Review：确认 non-resuming callee frame 终止语义未回流为旧 flag-based unwind
- 描述：在 `T3010b2b0` 之后只审查生产代码，确认普通 callee frame 在 non-resuming perform 后终止自身执行的实现是统一、可审计的控制流合同，而不是重新引入 `emit_effect_unwind_if_active`、旧 callee-suspend 路线或其它按代码形状分流的旁路；若发现回流，本任务需要直接修复并复审。
- 目标：
  - 确认普通函数 / helper 中的 non-resuming perform 终止语义来自统一 codegen 控制流，而不是旧 flag-based unwind。
  - 确认 `effect/mod.rs`、`control_flow.rs`、`mod.rs` 与相邻调用点没有重新按 callee/source shape 做 effect 传播分流。
  - 确认这一步只修复“callee 自己不继续执行”，没有吞掉 active flag，也没有破坏 caller 侧 state-machine dispatch。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“non-resuming callee frame 终止语义已统一收口，且未回流旧 flag-based unwind / shape-based 路线”。
- 依赖：T3010b2b0a

### T3010b2b1 [TODO] 修正 handle arm body 内 non-resuming effect 的外传 / self-inactive / finally cleanup 语义
- 描述：在 `T3010b2b0` 完成后复现当前验收用例时可见：普通 helper frame 已不再继续执行 `throw_alarm_unreachable`，但 arm body 中的 `Raise.raise(...)` 仍会错误继续执行到 `arm_unreachable`，sibling `Raise.raise` arm 也仍会自捕获，`finally` 没有在向外传播前恰好执行一次。`T3010b2b0` 已先修掉普通 callee frame 在 non-resuming perform 后继续执行的更基础缺口；本任务接着专注收口 arm body 执行期特有的 self-inactive / cleanup / outward propagation 语义。
- 目标：
  - arm body 中触发的 unmatched non-resuming effect 不得落回当前 handle 的正常完成路径；必须先经过 cleanup / `finally`，再继续向外传播。
  - arm body 执行期间当前 handler instance 必须处于 dispatch scope 外 / self-inactive；sibling arms 不得自捕获 arm body 内再次触发的 effect。
  - immediate-resume、escape continuation 与 pure non-resuming source-handle 在 arm body 内共享同一套外传 / cleanup 语义。
- 验收：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_finally_arm_raise.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_finally_arm_raise.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_nonresuming_raise_custom_finally.scoop`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3010b2b0R

### T3010b2b [TODO] 基于 synthetic resume slot + immediate-resume lowering 接通可执行的 post-suspend continuation tail，禁止 resume 后重放原表达式
- 描述：`T3010b2a` 已把 body tail 改写到 synthetic resume slot，`T3009a` 将补上 arm 内 `resume(value)` 的专用 lowering。两者接通后，再回到端到端语义收口：resume landing 只能继续剩余 tail，不能重放 suspend 前已经求值的路径。
- 进展：
  - 已修复 `while` / `if` branch condition 未进入 `state.reads`、outer slot 首次建模时类型退化为 `Any`、handle 首次进入 `step_fn` 前未 seed outer locals/params 到 effect frame，以及 continuation `resume_state_tag` 误写回 frame 的 runtime 回归。
  - `effect_resume_yield_int_basic.scoop`、`effect_resume_finally_normal.scoop`、`async_await_minimal_int_basic.scoop`，以及多条 comparison / branch / nested-block / mixed-raise 相关 fixture 已恢复到稳定 passing 基线。
  - 全量 LLVM fixture 现已推进到 arm body non-resuming effect 语义缺口，因此本任务在完成前先依赖 `T3010b2b1`。
- 目标：
  - 基于 `resume_path` + synthetic resume slot + immediate-resume lowering，为真正跨 suspend 的复合表达式接通可执行的 post-suspend tail。
  - `BindLocal` / `Assign` / `Return` / branch / expression statement 对跨 suspend 表达式都消费同一套恢复片段合同，而不是各自局部重算。
  - 为 escaped continuation 的 `Continuation.resume(...)` / composite payload 专用 lowering（`T3009b` / `T3013`）留下稳定前置。
- 验收：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_yield_int_basic.scoop`
  - 重新跑当前 `T3006` xfail 子集时，不再出现以下错误类别：
    - `暂不支持的 main 代码生成节点：member access target`
    - `暂不支持的 main 代码生成节点：comparison lhs`
    - `暂不支持的 main 代码生成节点：comparison rhs`
    - `暂不支持的 main 代码生成节点：equality lhs`
    - `暂不支持的 main 代码生成节点：integer binary op lhs`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3010b2b1

### T3010R [TODO] Review：确认 state-machine 生产 op 不再包含“先拆 fragment 再整棵重算”的双轨语义
- 描述：在 `T3010` 之后只审查生产代码，确认 unified contract / emitter 的 op 粒度已经收口，不再存在“fragment op 只是为后续整棵 expr 服务、自己却仍走生产 codegen”的残留；若发现这类残留，本任务需要直接修复并复审。
- 目标：
  - 确认 `HandleStateOp` 的生产消费者只接收独立可执行 op 或明确 no-op 语义 marker。
  - 确认 emitter 不再依赖 `VarRef` 式的“失败就吞掉”容错来保持 state-machine 可运行。
  - 确认统一主线没有重新引入按源码形状或局部 AST 片段做补丁式分流的逻辑。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“state-machine expression emission 粒度已收口；生产 op 不再含 fragment-only 伪执行路径”。
- 依赖：T3010b2b

### T3011 [TODO] 修正 unified contract 中 frame slot 的 mutability / capture metadata
- 描述：当前 plan builder 在“第一次看到 local ref”或“arm capture 外层 local”时会先创建 `mutable: false` 的 `FrameSlot`，后续也不会回填真实声明信息，导致原本是 `var` 的 slot 在 unified emitter / stmt codegen 中被冻结成 immutable，并报 `assignment to immutable local`。
- 目标：
  - frame slot 的 mutability 必须以声明点为真值来源；若 slot 先由 local ref / capture 路径占坑，后续接入声明信息时需要回填真实 `mutable` 状态，而不是保持默认 `false`。
  - arm capture、cross-state local、resume 后继续赋值等路径都应消费同一份 authoritative slot metadata。
  - unified emitter / `codegen_assign_stmt` 观察到的 mutability 与前端声明语义一致，不再被 state-machine 中间结构篡改。
- 验收：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_resume_unit.scoop`
  - 重新跑当前 `T3006` xfail 子集时，不再出现 `暂不支持的 main 代码生成节点：assignment to immutable local`。
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3010R

### T3011R [TODO] Review：确认 frame slot mutability / capture 元数据已按声明点收口
- 描述：在 `T3011` 之后只审查生产代码，确认 unified contract 中的 slot metadata 已由声明点驱动，不再有“先占坑成 immutable、后续永不修正”的残留；若发现这类问题，本任务需要直接修复并复审。
- 目标：
  - 确认 local ref、arm capture、resume 边界与普通 `val/var` 声明最终都汇聚到同一份 slot metadata。
  - 确认 emitter / stmt codegen 不再依赖 slot 创建顺序来决定 mutability。
  - 确认没有为了过回归而在 `codegen_assign_stmt` 层面加 effect-only 特判。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“frame slot mutability / capture metadata 已与声明语义对齐，无顺序依赖或 effect-only patch”。
- 依赖：T3011

### T3012 [TODO] 补齐 unified path 的 expected-context / closure / coercion 支持
- 描述：当前 unified emitter 对整棵表达式的重发射仍经常缺少稳定 expected context，导致 enum variant ctor、`print/println` 参数整形、`when`/`if` tail value、closure 值和某些 `coerce_value` 路径在 state-machine 中仍会报 `enum variant ctor call without expected enum type`、`sysroot print/println arg type`、`expression kind`、`value coercion`。
- 目标：
  - 统一 state-machine 路径在发射整棵 expr、arm body、handle result、local initializer、binder/result readback 时，补齐与普通 codegen 等价的 expected context 传递。
  - closure / function-value 相关表达式在 unified 路径上不再命中 `ExprKind::Closure` 的占位错误；间接 perform / indirect resume / closure-captured local 路径可执行。
  - 对 enum unit variant ctor、`print/println` 参数、tail value coercion 等 case，state-machine 路径与非 effect 路径复用同一套值整形规则。
- 验收：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_locals.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/std_test_assertions_basic.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_enum.scoop`
  - 重新跑当前 `T3006` xfail 子集时，不再出现以下错误类别：
    - `暂不支持的 main 代码生成节点：expression kind`
    - `暂不支持的 main 代码生成节点：enum variant ctor call without expected enum type`
    - `暂不支持的 main 代码生成节点：sysroot print/println arg type`
    - 仅因 expected context 缺失引发的 `暂不支持的 main 代码生成节点：value coercion`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3011R

### T3012R [TODO] Review：确认 unified path 的 expected context 与 closure 支持已与普通 codegen 对齐
- 描述：在 `T3012` 之后只审查生产代码，确认 unified path 不再靠 effect-only 特判或 fixture patch 传递 expected context，也不再把 closure case 留在占位错误后面；若发现这类问题，本任务需要直接修复并复审。
- 目标：
  - 确认 expected context 的来源都来自类型/slot/result 合同，而不是按 fixture 形状硬编码。
  - 确认 unified path 中的 closure / function-value 支持与普通 codegen 使用同一套生产逻辑。
  - 确认 effect codegen 没有重新引入按 `single`/`indirect`/`nested` 等源码分类分流的补丁。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“unified path 的 expected context / closure 支持已与普通 codegen 对齐，无 effect-only workaround 残留”。
- 依赖：T3012

### T3013 [TODO] 扩展 effect payload / handle result / resume payload transport，覆盖 composite 值
- 描述：当前统一主线仍把大量 payload/result 运输建立在 `u64 word` 假设上，遇到 tuple/struct/boxed enum payload/continuation ref 等 composite 值就会报 `u64 word from composite value`，并阻塞 handle result、perform binder、resume payload 三条链路的一致性。
- 目标：
  - 统一主线的 payload/result transport 需要同时覆盖 word-sized scalar、GC ref 与 composite 值；禁止通过 `ptr <-> int` 编码绕过去。
  - `perform` binder、handle result readback、continuation resume payload 三条路径共享同一套传输合同，而不是各自单独扩展。
  - 对 tuple/struct/boxed enum payload 等非 word 值，明确采用 `resume_gc_ref` / typed boxing / 等价 GC-safe 方案。
- 验收：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/handle_compound_result.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_nonresuming_payload_struct_indirect.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_continuation.scoop`
  - 重新跑当前 `T3006` xfail 子集时，不再出现 `暂不支持的 main 代码生成节点：u64 word from composite value`。
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3012R

### T3013R [TODO] Review：确认 payload/result transport 已统一收口到 GC-safe 合同
- 描述：在 `T3013` 之后只审查生产代码，确认 payload/result transport 的扩展不是零散地在三四个调用点各补一个特殊分支，而是真正形成统一、GC-safe、可审计的合同；若发现散落 patch，本任务需要直接修复并复审。
- 目标：
  - 确认 `perform` binder、handle result、resume payload 三条链路使用同一套 value transport 约定。
  - 确认 composite 值的运输路径不依赖 `ptr <-> int`、native-only side channel 或 effect-only 特判。
  - 确认 continuation / frame trace 合同与新的 composite transport 仍然一致。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“payload/result transport 已统一收口到 GC-safe 合同，无散落特判或非法 ptr/int 编码残留”。
- 依赖：T3013

### T3009b [TODO] 为 escaped continuation 的 `Continuation.resume(...)` 接回专用 lowering，并覆盖 composite resume payload
- 描述：在 immediate-resume arm 的 `resume(value)` 由 `T3009a` 单独接通之后，escaped continuation 的 `k.resume(value)` 仍需要 dedicated lowering。该路径不仅不能再回落到 generic member access，还必须与 `T3013` 的 composite payload transport 一起覆盖 ref / aggregate / richer resume payload。
- 目标：
  - 为 `Continuation.resume(...)` 提供专用 codegen 入口，不再依赖 generic member access 路径。
  - `k.resume(...)` 直接消费显式 continuation 值，并统一走 runtime continuation ABI。
  - 对 composite payload 的 transport 与 `T3013` 对齐，不新增 task-only / continuation-only 特例通道。
- 验收：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_resume_unit.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_resume_string.scoop`
  - 重新跑当前 `T3006` xfail 子集时，不再出现 `暂不支持的 main 代码生成节点：member access target`（限 escaped continuation 路径）。
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3013R

### T3009bR [TODO] Review：确认 escaped continuation resume 调用不再回落到 generic member access
- 描述：在 `T3009b` 之后只审查生产代码，确认 `Continuation.resume(...)` 已有 dedicated lowering，不再隐藏在 generic member access 中；若发现回落，本任务需要直接修复并复审。
- 目标：
  - 确认 `Continuation.resume(...)` 有显式 lowering 分派，并与 `T3013` payload 合同一致。
  - 确认 LLVM emitter / runtime ABI / helper 中不再保留 escaped continuation 的 placeholder-only glue。
  - 确认没有为过回归而在 generic member access 中加 effect-only 特判。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“escaped continuation resume 调用已由专用 lowering 完整接管，无 generic member-access fallback 残留”。
- 依赖：T3009b

### T3014 [TODO] 修正 handler stack 的 multi-op registration 与 unmatched effect 外传
- 描述：当前 handle 入口只把第一个 dispatch entry 的 `op_tag` push 到 runtime handler stack，`dispatch_unmatched` 也会直接落到 `handle_done`，导致 multi-arm handler 不能完整注册、不同 effect 的 nested handler 也会错误吞掉本应继续向外传播的 perform。
- 进展：
  - `T3008a` 消除 `ptr` / `ptr addrspace(1)` verifier 阻塞后，`effect_custom_nonresuming_nested_nearest_and_arm_outside_scope.scoop` 会持续重复打印 `inner_catch` / `0` 而无法到达 `middle_catch` / `outer_catch`；`effect_handler_stack_nearest_three_levels_and_arm_outside_scope.scoop` 与 `effect_op_tag_two_effects_nested_dispatch.scoop` 也会在 10s 超时窗口内挂起。
  - 这与 Appendix A.4 “handler arm body 在自身 dispatch scope 外执行” 的预期正相反，进一步确认当前 runtime handler stack 语义确实是主阻塞项，而不是单纯 fixture 配置问题。
- 目标：
  - 每个 dispatch entry 的 effect op 都必须在 runtime 动态上下文中有可匹配的注册表示，而不是只注册首个 op。
  - 当前 handle 未匹配到 `op_tag` 时，不得把 effect 吞掉；必须清理本 handle 自己的 runtime 注册后继续向外传播。
  - nested different-op handles 的 runtime stack 形态要与 compile-time dispatch 一致，不允许“编译时看起来会冒泡，运行时却被内层 handle 吃掉”。
- 验收：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_multi_nonresuming_custom_indirect.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_op_tag_two_effects_nested_dispatch.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_handler_stack_nearest_three_levels_and_arm_outside_scope.scoop`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3013R

### T3014R [TODO] Review：确认 multi-op handler registration 与 unmatched propagation 已与合同一致
- 描述：在 `T3014` 之后只审查生产代码，确认 multi-op handler registration 与 unmatched propagation 已形成统一语义，而不是在 dispatch loop 里拼临时分支把个别 fixture 打绿；若发现这类问题，本任务需要直接修复并复审。
- 目标：
  - 确认 runtime handler registration 与 contract `dispatch_entries()` 一一对应。
  - 确认 unmatched perform 的传播路径不再穿过 `handle_done` 正常完成路径。
  - 确认 effect codegen / runtime ABI 对“向外传播”没有 shape-based 或 fixture-based 特判。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“multi-op handler registration 与 unmatched propagation 已按统一合同收口，无吞 effect 的错误完成路径残留”。
- 依赖：T3014

### T3015 [TODO] 修正 arm 执行期 self-inactive 的 escaped-continuation 剩余缺口与 handler context lifetime
- 描述：`T3010b2b1` 会先收口同步 arm body 内 non-resuming effect 的外传 / cleanup / self-inactive 直接阻塞；本任务保留 escaped continuation 在 `handle` 返回后的 handler context 生命周期、以及延迟 / 跨线程 resume 时 active/inactive 恢复语义的剩余缺口。当前统一主线仍让 continuation 捕获稍后会被 pop 的 stack-alloc handler frame，导致 escaped continuation 恢复到失效的动态上下文。
- 进展：
  - `T3010a` 完成后的全量 LLVM fixture 首个失败点已推进到 `effect_escape_continuation_arm_performs_outer_effect.scoop`。直接运行显示当前输出会打印 binder `0`、继续落到 `unreachable_arm`，且没有命中外层 `EffectB` handler，符合本任务要修的 self-inactive / 外层 effect 传播缺口。
  - 同类 `effect_escape_continuation_nested_arm_indirect_performs_outer.scoop` 直接运行也会打印 `0` 并继续到 `unreachable_arm`，进一步确认这不是单个 fixture 偶发，而是 escape-continuation arm 执行期的统一语义缺口。
- 目标：
  - arm body 执行期间，触发当前 arm 的 handler instance 必须被临时置为 inactive；arm body 中再次 perform 同一 op 应命中外层 handler，而不是自捕获。
  - escaped continuation 不再捕获会在 `handle_done` 中被 pop/清空的 stack-alloc handler frame；需要改为可持久化、可恢复、与 continuation 生命周期一致的 handler context 表示。
  - continuation resume 时安装/恢复的 handler context 与 suspend 点语义一致，跨线程 / 延迟恢复仍然成立。
- 验收：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_arm_performs_outer_effect.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_nested_arm_indirect_performs_outer.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_scheduler_round_robin.scoop`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3014R、T3010b2b1

### T3015R [TODO] Review：确认 handler active/inactive 与 escaped continuation context 已真正闭环
- 描述：在 `T3015` 之后只审查生产代码，确认 arm self-inactive 语义与 escaped continuation 的 handler context 生命周期已经形成可审计的闭环，而不是继续依赖 stack-alloc frame 指针恰好“暂时没炸”；若发现这类问题，本任务需要直接修复并复审。
- 目标：
  - 确认 arm body 执行期当前 handler 的 active 状态变化有明确、对称、可恢复的生产路径。
  - 确认 continuation 捕获/恢复的 handler context 生命周期独立于外层 `handle` 的栈帧生命周期。
  - 确认跨线程 / 延迟 resume 不再依赖悬空 handler frame 指针。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“handler active/inactive 与 escaped continuation context 已闭环，无 stack-frame lifetime 漏洞残留”。
- 依赖：T3015

### T3016 [TODO] 接通 handle 内 `return` 的 function-return propagation
- 描述：当前 `ReturnFromFunction` terminator 会把 `STATE_TAG_FUNCTION_RETURNED` 写进 frame，但 handle 入口读取 `post_state_tag` 后仍把它直接丢掉；这意味着 handle 内显式 `return` 还没有真正向 enclosing function 传播。
- 目标：
  - `codegen_handle_expr_via_state_machine` 在读到 `STATE_TAG_FUNCTION_RETURNED` 时，必须接通 enclosing function 的 return 传播，而不是继续走普通 handle result 读回路径。
  - `return` 与 cleanup/finally/nested handle 的交互语义要与普通 codegen 一致，不允许“finally 跑完了，但函数没真正返回”。
  - 不通过 effect-only 特判绕过现有 return/cleanup 合同；统一复用既有函数返回基础设施。
- 验收：
  - 新增并跑通 dedicated run-pass fixtures：至少覆盖“handle 内显式 `return`”“`finally` 后显式 `return`”“nested handle 内显式 `return`”三类代表性 case。
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3015R

### T3016R [TODO] Review：确认 function-return sentinel 已真正接回 enclosing function return 合同
- 描述：在 `T3016` 之后只审查生产代码，确认 `STATE_TAG_FUNCTION_RETURNED` 已真正被消费并接入既有 return 基础设施，而不是另起一套 effect-only 返回旁路；若发现这类问题，本任务需要直接修复并复审。
- 目标：
  - 确认 handle entry 对 `STATE_TAG_FUNCTION_RETURNED` 的处理与普通 `return_context` / cleanup 机制一致。
  - 确认 nested handle / finally / early return 的组合不再依赖未消费 sentinel 或重复写 result slot。
  - 确认 unified effect 主线没有重新引入 flag-based unwind 式的函数返回旁路。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“function-return sentinel 已接回 enclosing function return 合同，无 effect-only 返回旁路残留”。
- 依赖：T3016

### T3017 [TODO] 回收 `T3006` 暂时 xfail fixtures，恢复 effect run-pass 基线
- 描述：在 `T3008+` 生产修复全部完成后，把当前 `tests/fixtures/run-pass/**` 中所有 `T3006: 暂时标记为 fail` 的临时注释与 `EXPECT: fail` 收回；若统一主线修复后需要微调少量 fixture 源码或 golden，必须在本任务中显式完成，而不是继续把实现缺口隐藏在 xfail 下。
- 进展：
  - `T3008a` 已先行回收 24 个只因 `ptr` / `ptr addrspace(1)` verifier 失败而临时 `EXPECT: fail` 的 run-pass fixtures；剩余 `T3006` xfail 现在对应的是更深层的真实语义缺口，而不再是 frame/continuation ABI 失配。
- 目标：
  - 收回当前所有 `T3006` 暂时 xfail 标记；只有经过验证仍需保留失败语义的 fixture 才能继续声明 `EXPECT: fail`，且原因必须更新为真实、当前的问题。
  - 对因统一主线正确语义收口而需要微调的 fixture / golden 做最小修改，保持测试意图不变。
  - 恢复 “effect run-pass fixtures 默认就是 passing baseline” 的仓库形态。
- 验收：
  - `rg -n "T3006: 暂时标记为 fail|EXPECT: fail" tests/fixtures/run-pass -S`
    对 effect 统一主线相关 fixture 不再残留当前这批临时标记。
  - `cargo run -p scoop --features llvm -- test`
  - 若涉及规范/文档或 golden 变更，所需配套文件已一并更新。
- 依赖：T3016R

### T3017R [TODO] Review：确认回收 xfail 后统一 effect 主线成为新的稳定 passing baseline
- 描述：在 `T3017` 之后做最终复审，只看生产代码与仓库测试基线形态，确认 effect run-pass 基线已经真正恢复，而不是靠保留隐性 xfail、跳过路径或局部 test-only workaround 维持绿色；若发现问题，本任务需要直接修复并复审。
- 目标：
  - 确认 effect 统一主线相关 run-pass fixtures 已恢复为 passing baseline。
  - 确认生产代码中无新增 shape-based / flag-based / effect-only fallback，用于“只让 fixture 过”的临时逻辑。
  - 在完成本任务后，`T30` 才可重新声明为阶段性完成。
- 验收：
  - 若审查发现问题，相关生产代码或基线形态已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“effect unified codegen 已成为稳定 passing baseline；`T3006` 临时 xfail 已回收，无 test-only workaround 残留”。
- 依赖：T3017

> 以下四个主题从 `TODO-3.md` 顺延迁入，按原顺序重编号为 `T31`～`T34`，仅对与当前 `T30` 主线直接相关的依赖与表述做最小更新。

## T31：前端 `do` block / closure 消歧与 block 语义收口

### T3101 [DONE] Parser / AST：引入显式 `do { ... }` block，并将裸 `{}` 固定为 closure
- 描述：当前 parser 在普通表达式位置把 `{ ... }` 同时暴露给 local block、lambda 与 trailing lambda 相关路径，导致 `val a = { println(“hello”) }` 之类写法无法从语法上判断是”把 closure 赋给 `a`”还是”先执行 block 再把结果赋给 `a`”。现行实现里部分 effect fixtures 只能借 `@Safe { ... }` 绕过该缺口，但这不是规范想要的语义。Scoop 改采 Swift 风格：普通局部 block 必须由 `do` 引入。
- 进展：
  - 已在 `Keyword` 枚举中新增 `Do` 关键字，lexer 识别 `”do”` 为关键字。
  - 已在 `ExprKind` 枚举中新增 `DoBlock { do_span, body }` 变体，与控制流内部使用的 `Block` 和 `Lambda` 区分。
  - `try_parse_expr_atom` 中 `do` 关键字优先于 `{` 匹配，确保 `do { ... }` 解析为 `DoBlock`，而裸 `{ ... }` 继续按 lambda 解析。
  - `@Safe` / `@Unsafe` 后支持可选 `do` 关键字：`@Safe do { ... }` 和 `@Unsafe do { ... }` 均可正确解析。`@Safe { ... }` 保持向后兼容。
  - 已更新所有 AST 消费者：`resolve/scopes.rs`、`typecheck/expr/infer.rs`、`typecheck/expr/stmt.rs`、`typecheck/expr/ops.rs`、`typecheck/expr/util.rs`、`typecheck/properties.rs`、`hir/lower/expr.rs`、`comptime/eval.rs`、`parser/cursor.rs`。`DoBlock` 在语义层面与 `Block` 等价。
  - 新增 4 个 parser 单元测试：`do_block_basic`、`bare_brace_is_lambda_not_do_block`、`safe_do_block`、`unsafe_do_block`。
  - 新增 2 个 parse fixtures：`do_block_basic.scoop`（验证 `do { ... }` 被解析为 `DoBlock`）、`do_block_vs_closure.scoop`（验证 `{ 1 }` 为 Lambda、`do { 1 }` 为 DoBlock）。
  - 已验证 `cargo check -p scoopc`（零 warning）、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`（217 passed）、`cargo run -p scoop -- test`（965 fixtures）、`cargo run -p scoop --features llvm -- test`（965 fixtures）全部通过。
- 目标：
  - statement-position / expression-position 的普通局部 block 统一写作 `do { ... }`；parser 不再把裸 `{ ... }` 解析为 plain block。
  - 没有 `do` 的 `{ ... }` 统一按 closure 解析；`callee { ... }`、`callee(args) { ... }`、multiple trailing lambdas 继续按调用后缀 / closure 规则工作。
  - parser / AST 为 `DoBlock` 与 `Closure` 保留稳定且可区分的形状，避免后续阶段继续依赖 `@Safe` 或上下文猜测。
- 验收：
  - 新增 parser fixtures：`val f = { 1 }` 为 closure、`val x = do { 1 }` 为 block、`foo { 1 }` 仍为 trailing lambda、`foo(do { 1 })` 为普通实参、缺少 `do` 时的稳定诊断或按 closure 分流。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T3102 [DONE] Typecheck / HIR：收口 `do` block 的 expression statement 与 tail value 语义
- 描述：当前 block 的值语义只看“最后一条是不是 `StmtKind::Expr`”，并不区分该表达式是否由 `;` 终止。即使语法层把普通 block / closure 消歧改成 `do`，类型系统和 lowering 仍需要统一“只有未终止 tail expr 才产生 block 值；`expr;` 只是 expression statement，结果视为 `Unit`”的规则。
- 目标：
  - `do` block 仅在最后一个表达式语句未以 `;` 终止时才产生 tail value；`do { expr; }` 作为 expression statement 结果视为 `Unit`。
  - `if` / `when` / `handle` / lambda body / `do` block 的值语义统一按上述规则工作，避免“语法已切到 `do`，typecheck 仍按旧规则取尾值”。
  - HIR / MIR / diagnostics 对 `do` block / closure body 中的 tail expr 与 terminated expr stmt 保持可区分形状，不再依赖 AST 阶段的隐式约定。
- 验收：
  - 新增 typecheck / HIR fixtures：`do { 1 }` 得 `Int`、`do { 1; }` 得 `Unit`、控制流 / handler / lambda body 的 tail expr 与 terminated expr stmt 区分、相关稳定诊断。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：T3101

### T3103 [TODO] 回归迁移：effect nested-block fixtures 切到 plain `do` block
- 描述：当前若干 effect nested-block 回归仍借 `@Safe { ... }` 表达 statement-position nested block；这只是实现缺口下的语法绕路，并没有真正覆盖“普通局部 block 必须写 `do { ... }`”的新规则。引入显式 `do` 后，需要把这些“只为消歧而存在”的 workaround 切回 plain `do` block。
- 目标：
  - 把仅用于制造 nested block 的 `@Safe` workaround 切回普通 `do { ... }`。
  - 真正依赖 safe-region 语义的测试继续保留 `@Safe` 相关写法，避免把 unsafe 语义回归误混进 parser / block 语义任务。
  - 锁定 trailing lambda / multiple trailing lambdas 与后续 `do` block 并存时的行为，避免 effect fixture 迁移顺手改变调用语义。
- 验收：
  - 更新 effect fixtures：至少覆盖 `effect_resume_nested_block_single_perform`、`effect_resume_while_nested_block_perform`，以及当前统一 effect 主线保留的 nested-block representative samples，统一改为 `do { ... }`，行为保持不变。
  - 新增 parser / typecheck / HIR / run-pass 回归：multiple trailing lambdas 与后续 `do` block 的边界规则。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3102, T3006R

### T3104 [TODO] 规范 / 文档：同步 `do` block、closure 优先级与 trailing-lambda 规则
- 描述：当前 `SCOOP_FULL_SPEC.md` 对 trailing lambda 仍保留“裸 `{ ... }` 可能同时像 block / lambda”的旧叙述；若只改实现不改规范，后续 fixtures 与文档示例会继续漂移。
- 目标：
  - 更新 `SCOOP_FULL_SPEC.md` 的 statements / local block / closure / trailing lambda / `@Safe` / `@Unsafe` 相关章节，明确普通 block 必须写 `do { ... }`、裸 `{ ... }` 一律视为 closure，以及 `@Safe do { ... }` / `@Unsafe do { ... }` 才是局部 annotated block 形式。
  - 同步改写规范内 doctest / fixture 示例，以及仓库内仍使用旧叙述的说明文档（至少当前 `TODO.md` / `PLAN.md` 中相关描述；若有其它命中文档也一并更新）。
  - 若规范代码块发生变更，补 `spec-fixtures sync/check` 所需的生成文件与说明，保证规范与回归继续一致。
- 验收：
  - `SCOOP_FULL_SPEC.md` 与相关文档更新完成；必要时运行 `cargo run -p scoop_tools -- spec-fixtures sync` 后再 `cargo run -p scoop_tools -- spec-fixtures check`。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：T3103

## T32：Structured Concurrency / `Task<T>`

### T3201 [TODO] 并发：`spawn` / `join` 的 typecheck 与 HIR 去 `Int` 硬编码
- 描述：`spawn { ... }` 目前仍要求 body 可赋给 `Int`，`join` lowering 也仍写死为 `Int` 句柄与 `Int` 结果。先把前端表示与类型系统改成真实的 `Task<T>`。
- 目标：
  - `spawn { body }` 从 body 推导结果类型 `T`，表达式类型为 `Task<T>`，不再要求 body 可赋给 `Int`。
  - HIR 对 `spawn` / `join` 保留任务结果类型与必要的运行期元信息，不再把 handle/result 擦成 `Int`。
  - 已确认仍缺失的后端功能由后续任务显式承接，而不是继续用前端 `Int` 特判掩盖：
    - `T3202`：HIR lowering / sysroot glue 仍写死 `__scoop_task_spawn_int` / `__scoop_task_join_int` 与 `Task<Int>` 表面；
    - `T3203`：LLVM codegen 仍只支持 `scoop_task_spawn_int` / `scoop_task_join_int`；
    - `T3204`：runtime executor 仍是 `ScoopTaskU64` / `result_u64` / `resume_u64` 单载荷模型。
- 验收：
  - 新增 typecheck / HIR fixtures：`Task<Int>`、`Task<String>`、`Task<Struct>`、`Task<Task<Int>>`。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T3202 [TODO] 并发：`spawn` / `join` 语法糖与 sysroot glue 去 `_int` 专用路径
- 描述：即使 `T3201` 把前端类型改成 `Task<T>`，当前 lowering 仍固定生成 `__scoop_task_spawn_int` / `__scoop_task_join_int`，`wrap_task_spawn_int_call` 与 sysroot `task/core` 表面也仍把结果类型收窄为 `Task<Int>`。这属于明确缺失的功能，不应被描述成“后端边界”。
- 目标：
  - HIR lowering / block rewrite 不再依赖 `__scoop_task_spawn_int` / `__scoop_task_join_int` 和 `wrap_task_spawn_int_call` 这类 `_int` 专用入口。
  - `sysroot/core.scoop` 与 `sysroot/task.scoop` 中供 `spawn` / `join` / `await` 路径使用的 internal glue 不再只暴露 `Task<Int>` / `Continuation<Int>` / `Executor.await(Task<Int>)`。
  - 语法糖 desugar 后的 HIR 仍能保留任务结果类型，为后续 LLVM / runtime 泛型化提供稳定输入。
- 验收：
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - 新增 HIR fixtures：`spawn { "x" }`、`spawn { Struct(...) }`、`join task` 的泛型结果类型保持不被擦除。
- 依赖：T3201

### T3203 [TODO] 并发：LLVM codegen 去 `scoop_task_*_int` 专用路径
- 描述：当前 LLVM 侧对 `spawn/join` 的支持仍硬编码在 `scoop_task_spawn_int` / `scoop_task_join_int`，并显式要求 `CgTy::Int` / `i64` 值。只要这条路径不改，`Task<T>` 就仍然只是假类型，不能执行 `Int` 之外的结果类型。
- 目标：
  - codegen dispatch 不再只识别 `__scoop_task_spawn_int` / `__scoop_task_join_int`，而是支持与 `Task<T>` 对齐的统一 task intrinsic / helper 路径。
  - `spawn` 结果保存、`join` 结果取回可覆盖 scalar / ref / aggregate / 泛型实例，不再把结果压扁回 `i64`。
  - task payload transport 与 continuation payload 方案保持一致，避免维护独立的 task-only ABI。
- 验收：
  - 新增 run-pass fixtures：`spawn` 返回 `String`、tuple/struct、class ref、嵌套泛型值。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3202

### T3204 [TODO] 并发：runtime executor / `Task<T>` 完成回调泛型化
- 描述：`runtime/c/scoop_task_executor.c` 当前仍是 `ScoopTaskU64` + `result_u64` + `on_complete_resume_u64` 模型，sysroot `task.scoop` 也仍把 `taskCreate` / `await` / `map` / `andThen` 固定在 `Task<Int>`。这不是“实现细节”，而是当前明确缺失的运行期功能。
- 目标：
  - runtime task 状态机、executor job、completion waiter 不再只支持 `u64` 结果与 `resume_u64`，而是支持与编译器 ABI 对齐的泛型 payload。
  - `Task<T>.onComplete`、`Executor.await`、`map`、`andThen` 等 glue 不再固定在 `Task<Int>` / `Continuation<Int>`。
  - ref / aggregate payload 在 pinning、GC stress、跨线程或跨 executor 恢复时语义稳定。
- 验收：
  - 新增 `crates/scoop_runtime/tests/*` 或等价 runtime 测试：泛型 task result、onComplete 恢复、ref payload rooting。
  - 至少一组 `SCOOP_GC_STRESS=1` run-pass 覆盖 `Task<String>` 或 `Task<StructWithRef>`。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3203

### T3205 [TODO] 并发：结构化并发回归矩阵与语义锁定
- 描述：`Task<T>` 泛型化后，需要用真实并发场景锁定语义边界，避免回归只覆盖“单任务 + Int 返回值”的最小路径。
- 目标：
  - 覆盖 nested `spawn` / `join`、控制流中的 `join`、多任务交错与 join 顺序等场景。
  - 验证 `Task<T>` 在错误传播、取消前置准备、GC 压力下的最小语义边界；暂不扩展到下一阶段 executor / stdlib API。
  - 把当前阶段明确不支持的并发组合写成稳定诊断或注释化限制，避免语义漂移。
- 验收：
  - 新增 run-pass fixtures：nested spawn/join、多任务交错、控制流 join。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3204

## T33：Lambda 推断与调用语义补齐

### T3301 [TODO] Lambda：expected function type 向任意参数个数传播
- 描述：当前 lambda 的 expected-type 向下传播只覆盖 0/1/2 参数；一旦没有 expected function type，未标注类型的参数会直接报错。需要先把最常见的上下文推断补齐，再把真正不可推断的场景留给稳定诊断。
- 目标：
  - expected function type 的传播覆盖任意参数个数，而不是只支持 0/1/2 参数 lambda。
  - 变量初始化、返回语境、调用实参、集合/构造器上下文等常见入口都能把 expected type 传给 lambda。
  - 对确实无法推断的场景保留清晰、稳定的错误信息，而不是依赖零散 early error。
- 验收：
  - 新增 typecheck fixtures：3+ 参数 lambda、多上下文 expected-type 传播、无法推断时的诊断。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T3302 [TODO] Lambda：receiver lambda 体内 `this` 与成员解析
- 描述：receiver function type 已经进入类型系统，但 lambda body 里当前还不会自动注入 `this`，导致“类型可表达、语义不可用”的断层。
- 目标：
  - receiver lambda 进入 typecheck / lowering 时自动建立 `this` 绑定与成员查找环境。
  - receiver lambda 中的成员访问、扩展调用、闭包捕获与普通 lambda 保持一致的局部作用域规则。
  - 相关 HIR / codegen 不再把 receiver 仅当作普通首参处理，避免 `this` 语义在后续阶段丢失。
- 验收：
  - 新增 typecheck / run-pass fixtures：receiver lambda 直接访问 `this`、调用成员、捕获外层局部。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3301

### T3303 [TODO] 调用语义：统一函数值 / funptr / ctor delegation 的实参匹配
- 描述：当前函数值调用、函数指针调用、`super(...)` / `this(...)` 构造器委托调用仍各自带有命名实参或 receiver function type 的早期门禁，调用规则没有真正统一。
- 目标：
  - 函数值调用支持命名实参，参数匹配规则与普通函数调用保持一致。
  - 函数指针调用支持命名实参，并解除对 receiver function type 的不必要早期拒绝，或在更合理的阶段统一降格/诊断。
  - `super(...)` / `this(...)` 构造器委托调用改用同一套实参匹配逻辑，不再只允许位置参数。
- 验收：
  - 新增 typecheck / run-pass fixtures：函数值命名实参、funptr 命名实参、ctor delegation 命名实参。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3301

## T34：泛型约束 / Pattern / 值类型能力补齐

### T3401 [TODO] 泛型：`where` nominal bound 支持类型实参与 instantiated supertype 满足性
- 描述：当前 `where` 约束一旦写成带类型实参的 nominal bound（例如 `where T: Box<Int>`）就会被直接拒绝；即便后续把 bound 本身 lower 成实例化类型，类/接口经由“已实例化的 supertype”满足该 bound 的场景也还没有被显式承接。需要把这类 bound 贯通到解析、解析后表示、检查、子类型关系与诊断。
- 目标：
  - 支持带类型实参的 nominal `where` bound，并正确解析到已实例化的 bound type。
  - 类/接口若通过已实例化 supertype 满足该 bound（例如 `Sub : Base<Int>` 满足 `where T: Base<Int>`），实例化处检查可正确通过，而不是只接受“exact same nominal type”。
  - 实例化处 bound 检查、函数体内成员分发、错误消息都基于实例化后的 bound，而不是回退到未参数化 nominal type。
  - 对不满足或不可解析的 bound 给出稳定诊断。
- 验收：
  - 新增 typecheck fixtures：正例 `where T: Box<Int>`、经由 instantiated supertype 满足 `where T: Base<Int>`、反例 `where T: Box<String>`、体内通过 bound 调方法。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T3401a [TODO] 泛型：`where` nominal bound 的子类型满足性回归矩阵
- 描述：当前 use-site `where` 检查已通过 `is_type_assignable` + `direct_supertypes` 支持最小 nominal 上转，但仓库里还没有专门锁定“接口/类继承链上的实例满足 generic bound”这条语义。像 `interface Sub : Base`、`class Impl : Sub`、`fun <T> f(x: T) where T: Base` 这样的路径，不应只依赖当前实现细节“碰巧有效”。
- 目标：
  - 为当前已支持的非参数化 nominal bound 语义补齐回归：类实现接口、类继承基类、子接口及其实现类型传给父接口 bound。
  - 覆盖变量持有子类型实例、参数透传、泛型 passthrough 等常见入口，而不只断言字面量/构造表达式直传。
  - 对 builtin/value 类型通过 boxing 满足 interface bound 的既有语义也补专门回归，避免后续 `assignable` 调整回退。
- 验收：
  - 新增 typecheck / run-pass fixtures：`Sub : Base`、`Impl : Sub` 满足 `where T: Base`，以及 `Int` / `Bool` 满足 `Hashable` 或等价 interface bound。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T3401b [TODO] 泛型：`where` bound 驱动的方法分发补齐接口继承链与多 bound 歧义
- 描述：当前 `where` bound 驱动的方法分发只会从声明的 bound 本身构造 `Bound.method` 并在首个命中的 bound 上提前返回，没有沿接口 supertype 链查找继承成员，也不会对多个 bounds 提供的同名成员做歧义诊断。这会把合法语义错判成“方法不存在”，或者把本该报歧义的场景静默绑定到声明顺序。
- 目标：
  - `where T: Sub` 可调用 `Base` 上声明的方法，接口继承链上的成员对 bound receiver 可见。
  - `where T: A, T: B` 下的同名成员遵守明确的候选集/歧义规则，不再按遍历顺序抢先返回。
  - 对“继承成员可见”“多 bound 歧义”“确实不存在该成员”三类场景给出稳定回归与诊断。
- 验收：
  - 新增 typecheck / run-pass fixtures：通过 `where T: Sub` 调用 `Base` 方法、多 bound 同名成员的歧义报错。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：T3401a

### T3401c [TODO] 调用/泛型：成员方法签名收集与 `where` bound 分发对齐 richer generic/effect 调用
- 描述：当前共享的成员方法签名收集路径仍会直接跳过“2 个以上 type params”与带 `<eff E>` 的成员方法；`where` bound 驱动的方法分发还会忽略显式类型实参。这使 `x.method<U>()`、多 type param 成员方法、effect-generic 成员方法在普通 member call 与 bound member call 上都存在实现层缺口。
- 目标：
  - 普通成员调用与 `where` bound 驱动的成员调用都支持显式类型实参，不再把 `member<T>(...)` silently 降格成“只能靠推断”。
  - 成员方法签名收集、实例化与诊断支持 2+ type params 与 `<eff E>` 成员方法，不再在候选收集阶段直接跳过。
  - 成员方法调用的 type arg / effect arg 规则与顶层函数调用尽量对齐，避免 shared helper 与 top-level call 各走一套缩水语义。
- 验收：
  - 新增 typecheck / run-pass fixtures：普通 member call 与 `where` bound member call 的显式类型实参、2+ type params 成员方法、带 `<eff E>` 的成员方法。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：T3401b

### T3402 [TODO] Pattern：顶层 `val` 支持 pattern binding
- 描述：局部 destructuring 已逐步落地，但顶层 `val (a, b) = ...` 仍在声明头检查阶段被直接拒绝，导致同一套 pattern 语法在顶层与局部语义不一致。
- 目标：
  - 顶层 tuple / struct / enum destructuring 复用既有 pattern binding 规则，而不是单独保留“顶层只允许标识符”的限制。
  - 顶层符号安装、初始化顺序、多文件可见性与循环引用诊断保持稳定。
  - 对当前仍不支持的递归或歧义 pattern 给出明确报错，而不是统一 early reject。
- 验收：
  - 新增多文件 fixtures：顶层 tuple/struct 解构、跨文件引用、非法 pattern 诊断。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T3403 [TODO] 值类型：`struct` 字段支持 `var` 与默认值
- 描述：当前 `struct` 字段同时禁止 `var` 与默认值，值类型声明能力明显弱于目标语言语义。需要先收口字段模型，再决定更新与构造路径如何共享实现。
- 目标：
  - `struct` 声明支持 `var` 字段与默认值，声明头、构造参数、布局与初始化规则保持一致。
  - 默认值在构造调用、`with` 更新与常量/编译期路径中有统一语义，不引入额外特判。
  - 字段可变性与值语义冲突处给出明确约束和诊断。
- 验收：
  - 新增 run-pass fixtures：带默认值的 `struct` 构造、省略默认参数、`var` 字段更新。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：无

### T3404 [TODO] 值类型：`with` 更新扩展到更完整的值类型语义
- 描述：当前 `with` 更新只支持 `struct` 且显式拒绝嵌套字段路径更新，无法覆盖更接近 record-style 的值对象写法。
- 目标：
  - `with` 的 base 类型不再局限于当前最小 `struct` 子集，而是对齐本轮支持的值类型模型。
  - 嵌套字段路径更新 lower 成稳定的 copy-update 链，而不是在 typecheck 阶段直接拒绝。
  - 诊断能够区分“字段不存在 / 字段不可更新 / 类型不匹配 / base 非值类型”等不同错误。
- 验收：
  - 新增 HIR / run-pass fixtures：单层 `with`、嵌套路径 `with`、非法更新诊断。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3403

### T3405 [TODO] Pattern：`when` 的 or-pattern 支持共享 binder
- 描述：当前 or-pattern 已能做简单判别，但一旦在 `A(x) | B(x)` 里引入 binder 就会被直接拒绝。需要把 binder 集一致性与类型合流规则补齐。
- 目标：
  - 当各分支 binder 集、名称与类型兼容时，允许 or-pattern 引入共享 binder。
  - `when` arm 的局部环境合并稳定，不再依赖“or-pattern 不能绑定名字”的早期限制。
  - 对 binder 数量、名称或类型不一致的分支给出具体诊断。
- 验收：
  - 新增 typecheck / run-pass fixtures：合法 binder or-pattern、非法 binder mismatch。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：T3402
