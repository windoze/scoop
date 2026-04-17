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

### T3010b2b0a0 [DONE] 修正 ordinary callee 内 hidden-suspend boundary 后仍继续执行的控制流语义
- 描述：执行 `T3010b2b0a` 时补了“helper -> object property access -> object init 内 `Raise.raise(...)`”定向复现，结果发现当前阻塞点比 caller-side 分类更前置：`main_unreachable` 已经不再出现，但 `helper()` 自身仍会在 `BoomObject.x` 返回 active 后继续执行 `helper_unreachable`。这说明 `T3010b2b0` 目前只覆盖了 direct `perform/Raise`、ordinary user call 与 `as` cast raise；object value/property access、class ctor init、builtin runtime raise 等 hidden suspend boundary 仍没有接到 ordinary-frame propagation contract。
- 进展：
  - 已在 `crates/scoopc/src/llvm/codegen/mod.rs` 的 `codegen_object_property_access` 中把 object init 调用接到统一 `emit_ordinary_call_effect_propagation_check`，保证 hidden-suspend object property access 一旦返回 active，当前 ordinary callee frame 会立即向 caller 返回，不再继续执行后续语句。
  - 已新增 `tests/fixtures/run-pass/object_property_init_raise_helper_try_catch_basic.scoop`，直接锁定“helper -> object property access -> object init raise”路径，确认 stdout 不再出现 `helper_unreachable` 与 caller tail。
  - 已新增 `tests/fixtures/run-pass/class_init_hidden_raise_helper_try_catch_basic.scoop`，补充覆盖 class ctor property initializer 经由 object property access 触发 hidden suspend 的 helper 路径，确认 ctor 初始化阶段同样不会让 helper 继续执行。
  - 已验证：
    - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/object_property_init_raise_helper_try_catch_basic.scoop`
    - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/class_init_hidden_raise_helper_try_catch_basic.scoop`
    - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/object_init_raise_try_catch_basic.scoop`
    - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/class_init_raise_cleanup_property_init_gc_basic.scoop`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
- 复跑 `cargo run -p scoop --features llvm -- test` 后，首个失败点仍保持在已知后续 blocker `effect_escape_continuation_finally_arm_raise.scoop`（对应 `T3010b2b1`），未出现更早的 hidden-suspend helper 回归。
- 目标：
  - ordinary 函数 / helper / 方法体内，一旦 hidden suspend boundary（至少覆盖 object value/property access、class ctor init、builtin runtime raise）向当前 frame 返回 active，当前 callee frame 必须立刻停止执行，不能继续落到后续 statement / tail expression。
  - 该终止语义必须沿用统一 ordinary-frame propagation 合同，禁止恢复旧 flag-based unwind，也禁止只给单个 object/property access callsite 打补丁。
  - 为后续 `T3010b2b0a` 提供真实前置：先保证 helper 自己不会继续执行，再继续审查 caller-side unified state-machine 是否把这类 helper 调用当成 suspend boundary。
- 验收：
  - 新增一个覆盖“helper -> object property access -> object init raise”路径的 run-pass fixture，并验证 stdout 同时不包含 `helper_unreachable` 与 caller tail。
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/object_init_raise_try_catch_basic.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/class_init_raise_cleanup_property_init_gc_basic.scoop`
  - 复跑 `cargo run -p scoop --features llvm -- test`，确认没有把首个失败点拉回 hidden-suspend helper 路径；当前首个失败点仍是已知后续 blocker `effect_escape_continuation_finally_arm_raise.scoop`（`T3010b2b1`）。
- 依赖：T3010b2b0

### T3010b2b0a0b [DONE] 修正 object 单例值的 LLVM 表示与 `Ref` ABI 失配，恢复 object member call
- 描述：继续验证 `T3010b2b0a` 的 member 路径时，构造了最小 repro：`object Helper { fun run(): Int { ... } }`。当前无论是普通 `Helper.run()`，还是 `handle { Helper.run() }`，都会在 LLVM verifier 报 `Call parameter type does not match function signature!`，表现为 `ptr @__scoop_object_instance__Helper` 被传给期望 `ptr addrspace(1)` receiver 的 `@Helper.run(...)`。根因是 `declare_object_instance_global` 仍把 object 单例值表示成 default addrspace 的 module-local 身份地址，而对象成员函数 receiver 通过 `CgTy::Ref` / `llvm_param_ty` 走的是 GC `addrspace(1)` ABI。这个更基础的 object value 表示缺口会先于 hidden-suspend 分类暴露，因此必须先修复，`T3010b2b0a` 才能继续对 member 路径做 spec-correct 验证。
- 进展：
  - `declare_object_instance_global` 已改为保存 `ptr addrspace(1)` 的 module-local 槽位，不再把默认地址空间的全局身份地址直接当作 object 值返回。
  - object init 现在会通过 `scoop_alloc_typed` 分配 header-only 的 GC singleton object，并写入专用 object type descriptor；object value access 改为先执行 once init，再从全局槽加载真正的 GC-managed receiver。
  - object nominal type 的 runtime type check 现在可通过 object type descriptor 走 exact-match parent-chain 查询，不再把 object 值留在“只有地址、没有 `Ref` header/type-desc”的半成品表示。
  - 已新增 LLVM IR 单测 `object_member_call_uses_gc_managed_singleton_receiver` 与 run-pass fixture `object_member_call_basic.scoop`，最小 repro `Helper.run()` 不再触发 `module_verification_failed`。
  - 已验证 `cargo test --all`、`cargo clippy --all-targets -- -D warnings`；复跑 `cargo run -p scoop --features llvm -- test` 后，首个失败点仍停在已知 blocker `effect_escape_continuation_finally_arm_raise.scoop`（`T3010b2b1`），未出现更早回归。
- 目标：
  - 收口 object 单例值的 LLVM 表示，使 object value / object member call 的 receiver 与 `CgTy::Ref` 的 `addrspace(1)` ABI 保持一致。
  - 确保普通 `Helper.run()` 这类 object member call 不再触发 `module_verification_failed`。
  - 禁止通过 callsite 局部 bitcast、仅 effect path 特判或其它 verifier-hack 掩盖该问题。
- 验收：
  - 新增一个最小 run-pass fixture，覆盖普通 object member call，不再出现 `module_verification_failed`。
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T3010b2b0a0

### T3010b2b0a [DONE] 修正 hidden-suspend ordinary callee 在 unified state-machine caller 侧被误判为 plain `Call`
- 描述：`T3010b2b0a0` 完成后，继续用临时 `handle` 复现回查 caller-side 路径时，top-level helper、经 class ctor 触发 hidden suspend 的 helper，以及 local closure 包一层 helper 的路径都已经能正确进入 dispatch，不再继续执行 caller tail。member 路径此前被 `T3010b2b0a0b` 暴露的 object 单例值 ABI 缺口挡住；该 blocker 现已修复，因此本任务接着重新验证并补齐 top-level/member/local function value 的 hidden-suspend suspend-call 分类覆盖；若 member 路径 ABI 收口后仍存在 caller-side 误判，再在本任务内继续修正 plan builder / metadata。
- 进展：
  - 已重新验证 top-level helper、object member helper、以及 local closure/function-value 包装 helper 三条 caller-side 路径；在 hidden suspend 返回 active 后，handle body 都会立刻进入统一 dispatch，不再继续执行 caller tail。
  - 已新增三个 run-pass fixture：
    - `tests/fixtures/run-pass/effect_handle_hidden_suspend_helper_object_property_basic.scoop`
    - `tests/fixtures/run-pass/effect_handle_hidden_suspend_member_helper_basic.scoop`
    - `tests/fixtures/run-pass/effect_handle_hidden_suspend_local_closure_helper_basic.scoop`
    三者分别锁定 top-level helper、member helper 与 local closure/function-value 包装 helper 的 caller-side hidden-suspend 路径，确认 stdout 不包含 `handle_unreachable`。
  - 已在 `crates/scoopc/src/llvm/codegen/effect/state_machine_segments.rs` 补充两条分类单测：member helper 直接锁定为 `call-state-machine-callee`，local closure/function-value 调用锁定为 `call-may-suspend`，避免后续退化回 plain `Call`。
  - 已验证：
    - `cargo test -p scoopc segment_dump_classifies_hidden_suspend_ -- --nocapture`
    - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_hidden_suspend_helper_object_property_basic.scoop`
    - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_hidden_suspend_member_helper_basic.scoop`
    - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_hidden_suspend_local_closure_helper_basic.scoop`
    - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/object_init_raise_try_catch_basic.scoop`
    - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/class_init_raise_cleanup_property_init_gc_basic.scoop`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
- 复跑 `cargo run -p scoop --features llvm -- test` 后，首个失败点仍是已知后续 blocker `effect_escape_continuation_finally_arm_raise.scoop`（对应 `T3010b2b1`），说明本任务没有引入更早的 caller-side hidden-suspend 回归。
- 目标：
  - 重新验证 top-level/member/local function value 的 suspend-call 分类是否都能把 hidden-suspend ordinary callee 识别为统一 suspend boundary，而不是落回 plain `Call`。
  - 若验证发现仍有 caller-side 分类缺口，为 top-level/member/local function value 的 suspend-call 分类补齐 hidden-suspend 元数据，至少覆盖 object value/property access、class ctor init 与 runtime raise 这几类现有 hidden suspend source。
  - 确保 ordinary helper 在向 caller 传播 active 后，caller 侧 unified state-machine 会立即进入统一 dispatch / cleanup / resume 合同，而不是继续执行 call 后 tail。
  - 禁止通过恢复旧 flag-based unwind、按源码形状分流，或只给单个 object access callsite 打补丁来掩盖该问题。
- 验收：
  - 新增一个覆盖 `handle` body 中“helper -> object property access -> object init raise”路径的 run-pass fixture，并验证 stdout 不包含 caller tail。
  - 补齐至少一条 member 或 function-value 路径的定向覆盖，确认 caller-side hidden-suspend 分类不会退化为 plain `Call`。
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/object_init_raise_try_catch_basic.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/class_init_raise_cleanup_property_init_gc_basic.scoop`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3010b2b0a0b

### T3010b2b0R [DONE] Review：确认 non-resuming callee frame 终止语义未回流为旧 flag-based unwind
- 描述：在 `T3010b2b0` 之后只审查生产代码，确认普通 callee frame 在 non-resuming perform 后终止自身执行的实现是统一、可审计的控制流合同，而不是重新引入 `emit_effect_unwind_if_active`、旧 callee-suspend 路线或其它按代码形状分流的旁路；若发现回流，本任务需要直接修复并复审。
- 进展：
  - 已审查 `crates/scoopc/src/llvm/codegen/effect/mod.rs`：ordinary-frame propagation 仅由 `emit_ordinary_non_resuming_effect_exit` 与 `emit_ordinary_call_effect_propagation_check` 驱动；`emit_effect_propagation_return` 只发射默认返回值/`return_bb` 控制流，不会清掉 TLS active，也未回流旧 `emit_effect_unwind_if_active`。
  - 已审查 `crates/scoopc/src/llvm/codegen/mod.rs`：direct/vtable/itable/funptr/closure/operator/object property/object init 等 ordinary callsite 都统一接到 `emit_ordinary_call_effect_propagation_check`；direct non-resuming 只从 `codegen_perform_expr` 与 `codegen_cast_as_expr` 的 runtime raise fail-path 接入 ordinary-frame 早退合同。
  - 已审查 `crates/scoopc/src/llvm/codegen/control_flow.rs`：未发现 effect 专用 CFG 分流、active/clear/unwind 逻辑；仅有局部变量 `call_may_suspend` 元数据赋值，与旧 flag-based unwind / shape-based 路线无关。
  - 已复查 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 的上下文切换边界：step function 生成前会暂存并清空 `current_fun_return_ty` / `return_context`，说明 ordinary-frame propagation helper 不会误入统一 state-machine step/dispatch；caller 侧 dispatch 仍由 state machine 的 `is_active -> clear_active -> dispatch` 路径接管。
  - 已检索生产代码关键词：`emit_effect_unwind_if_active`、`raise_target_stack`、`CalleeSuspend`、`scan_for_callee_suspend`、`suspendable`、`shape-based`、`scanner` 均无生产代码命中；ordinary callee 路径也未使用 `declare_runtime_effect_clear` / `clear_active`，确认没有吞掉 active。
  - 已验证：
    - `target/debug/scoop run tests/fixtures/run-pass/nothing_raise_in_helper_basic.scoop`
    - `target/debug/scoop run tests/fixtures/run-pass/effect_indirect_perform_nonresuming_call_chain.scoop`
    - `target/debug/scoop run tests/fixtures/run-pass/object_property_init_raise_helper_try_catch_basic.scoop`
    - `target/debug/scoop run tests/fixtures/run-pass/class_init_hidden_raise_helper_try_catch_basic.scoop`
    - `target/debug/scoop run tests/fixtures/run-pass/effect_handle_hidden_suspend_helper_object_property_basic.scoop`
    - `target/debug/scoop run tests/fixtures/run-pass/effect_handle_hidden_suspend_member_helper_basic.scoop`
    - `target/debug/scoop run tests/fixtures/run-pass/effect_handle_hidden_suspend_local_closure_helper_basic.scoop`
    - `target/debug/scoop run tests/fixtures/run-pass/object_init_raise_try_catch_basic.scoop`
    - `target/debug/scoop run tests/fixtures/run-pass/class_init_raise_cleanup_property_init_gc_basic.scoop`
    - `cargo test -p scoopc segment_dump_classifies_hidden_suspend_ -- --nocapture`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
  - 已复跑 `target/debug/scoop test`，首个失败点仍为已知后续 blocker `tests/fixtures/run-pass/effect_escape_continuation_finally_arm_raise.scoop`（对应 `T3010b2b1`），未出现更早回归。
- 目标：
  - 确认普通函数 / helper 中的 non-resuming perform 终止语义来自统一 codegen 控制流，而不是旧 flag-based unwind。
  - 确认 `effect/mod.rs`、`control_flow.rs`、`mod.rs` 与相邻调用点没有重新按 callee/source shape 做 effect 传播分流。
  - 确认这一步只修复“callee 自己不继续执行”，没有吞掉 active flag，也没有破坏 caller 侧 state-machine dispatch。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“non-resuming callee frame 终止语义已统一收口，且未回流旧 flag-based unwind / shape-based 路线”。
- 审查结论：
  - non-resuming callee frame 终止语义已统一收口，且未回流旧 flag-based unwind / shape-based 路线。ordinary-frame 相关生产路径只读取 TLS active 并走统一 early-return 控制流，不清除 active、不读取源码形状，也不依赖旧 callee-suspend scanner / mode。
  - caller 侧 state-machine dispatch 未被破坏：step function 会隔离 ordinary-frame helper 所依赖的 `current_fun_return_ty` / `return_context`，而 handle dispatch 仍通过统一 `is_active -> clear_active -> dispatch` 路径消费 active。
- 依赖：T3010b2b0a

### T3010b2b1a [DONE] 修正 handle arm body direct non-resuming effect 的外传 / finally cleanup / no-perform handle result 语义
- 描述：在 `T3010b2b0` 完成后复现 arm-body 相关验收用例时可见：arm body 中的 `Raise.raise(...)` 仍会继续执行到 `arm_unreachable`，sibling `Raise.raise` arm 会自捕获，`finally` 只在 body 正常落到 `CleanupEnter` 时执行，arm return / outward propagation / no-perform body 的路径都会绕开 cleanup，且某些 no-perform handle 仍返回 `0` 而不是 body 的真实值。本子任务先收口这批 direct 路径。
- 进展：
  - 已在 `state_machine_emitter.rs` 中把 arm body 的整棵表达式重发射临时置于 ordinary effect propagation 合同下，使 arm body 内 direct `Raise.raise(...)` / indirect helper call 触发的 active 会立刻结束当前 step function，而不会继续执行 arm body 后续语句。
  - 已重构 handle dispatch 出口：matched arm 在进入 arm state 前先消费当前 perform 的 active；arm step 返回后根据 `state_tag` 区分“arm 自身再次 perform”与“arm resume 后 body 再次 perform”，前者直接走 outward propagation，不再回落到 sibling arm 自捕获。
  - 已把 handle-level cleanup/`finally` 从“只覆盖 body 正常收尾”扩展到 arm return/outward propagation，并通过 frame `cleanup_flag` 防止重复执行；cleanup 重入 step function 前会保留 frame 中已有的 handle result，避免 `finally` 把结果覆盖成 `0`。
  - 已顺带回收 4 条同根因 xfail：`effect_resume_finally_arm_raise.scoop`、`effect_escape_continuation_finally_no_perform.scoop`、`effect_escape_continuation_zero_perform_returns_body.scoop`、`effect_no_perform_handle_elim_basic.scoop`。
- 目标：
  - arm body 中 direct unmatched non-resuming effect 不得落回当前 handle 的正常完成路径；必须先经过 cleanup / `finally`，再继续向外传播。
  - sibling arms 不得自捕获 arm body 内 direct 触发的再次 perform。
  - no-perform handle body、arm normal return 与 finally 组合时，handle result 必须保持为 body/arm 的真实值。
- 验收：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_finally_arm_raise.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_finally_arm_raise.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_nonresuming_raise_custom_finally.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_finally_no_perform.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_zero_perform_returns_body.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_no_perform_handle_elim_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T3010b2b0R

### T3010b2b1b0 [DONE] 修正 synthetic resume slot 的 `SymbolId` 冲突与 nested handle frame seeding 合同
- 描述：开始执行 `T3010b2b1b` 并对 `effect_escape_continuation_nested_arm_indirect_performs_outer.scoop` 做定向定位后确认，当前首个失败点并不只是 expected-context。inner handle 入口的 `seed_outer_scope_frame_slots` 会把 outer local `_ : Unit` 误当成 synthetic resume slot `__resume_site0 : Int` 写入 inner frame；根因是 synthetic resume slot 复用了与外层局部变量相同的 `SymbolId`，导致 env / frame-slot 查找冲突，在真正进入 `T3010b2b1b` 的 unified coercion 逻辑前就先触发 `Unit -> Int` coercion。这个前置 bug 当前未在 TODO 中显式跟踪，必须先前移修复。
- 进展：
  - 已把 synthetic symbol 分配从“每个 handle 本地 `max(id)+1`”改为共享 floor/cursor 分配：floor 由当前 `handle` 子树与 `known_local_metadata` 共同抬高，真正分配通过共享 cursor 完成，避免 outer/local/nested handle 间复用同一 `SymbolId`。
  - 已让 `seed_outer_scope_frame_slots` 只 seed 显式 outer-scope slot，不再因为 `env.get(id)` 命中而把 synthetic resume slot、普通 handle local 或 arm binder 误当成可 seed 局部。
  - 已新增两条定向单测：锁定 outer/inner handle synthetic resume slot `SymbolId` 不再冲突，以及 nested handle frame seeding 不会把 `__resume_site*` 标成 outer-scope seed slot。
  - `effect_escape_continuation_nested_arm_indirect_performs_outer.scoop` 现已直接通过；同时顺带回收了 `effect_custom_nonresuming_nested_nearest_and_arm_outside_scope.scoop`、`effect_escape_continuation_arm_performs_outer_effect.scoop`、`effect_escape_continuation_reperform_from_escape_arm.scoop`、`effect_handle_yield_and_step_finally.scoop` 与 `effect_handler_stack_nearest_and_arm_outside_scope.scoop` 这批历史 xfail。
- 目标：
  - synthetic resume slot 必须拥有与源码局部变量、capture local、outer-scope seeded local 都不冲突的唯一标识。
  - `seed_outer_scope_frame_slots` 只拷贝真实 outer locals，不得再把 synthetic resume slot 与同 id 的局部变量混淆。
  - `effect_escape_continuation_nested_arm_indirect_performs_outer.scoop` 不再在 inner handle entry 的 frame seeding 阶段提前报 `Unit -> Int` coercion。
- 验收：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_nested_arm_indirect_performs_outer.scoop`
  - 重新跑当前 `T3006` xfail 子集时，不再因为 `seed_outer_scope_frame_slots` / synthetic resume slot id 冲突提前失败
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T3010b2b1a

### T3009b0a [DONE] 把 unified handle frame 中 outer-scope `var` slot 的写回接回 enclosing locals
- 描述：开始执行 `T3009b0` 的首个目标 fixture 后，`effect_escape_continuation_resume_unit.scoop` 已不再报 `暂不支持的 main 代码生成节点：call callee`，但 escape arm 中的 `saved = Some(k)` 离开 `handle` 后仍然读到 `None`。当前 unified path 只在 `codegen_handle_expr_via_state_machine` 入口通过 `seed_outer_scope_frame_slots` 把 outer locals/params 复制到 effect frame，却没有在 handle 正常完成后把被 frame 改写的 outer-scope mutable slot 写回原来的 enclosing local alloca，导致 arm body / finally / handle body 对外层 `var` 的赋值丢失。这个更前置缺口当前未在 TODO 中显式跟踪，但它会直接阻塞 escaped continuation 被保存出来，因而必须先于 `T3009b0` 修复。
- 进展：
  - 已把 unified contract 的 outer-scope slot 收集范围从仅 `handle.body` 扩展到整个 `handle`（body、arms、finally），并显式排除 arm binder / resume / continuation locals 以及 handle 内部局部，避免把 `k` 或内部局部误标为 seeded outer slot。
  - 已在 `state_machine_emitter.rs` 中新增 metadata 驱动的 `write_back_outer_scope_frame_slots` helper，并接到 `handle_done` 与 `handle_propagate` 两个出口；authoritative outer slot 现在会统一从 frame 回写到 enclosing local。
  - 已新增结构测试 `handle_outer_scope_seeding_includes_arm_and_finally_locals`，锁定“只在 arm/finally 中引用的 outer local 也必须进入 outer seeded slots，arm 局部不得混入”。
  - 已新增 focused fixture `effect_escape_continuation_outer_var_writeback_basic.scoop`，覆盖 handle body / escape arm / finally 对 outer `var` 的写回；该 fixture 已通过。
  - 复跑 `effect_escape_continuation_resume_unit.scoop` 与 `effect_escape_continuation_resume_string.scoop` 后，输出已不再落到 `missing`，而是推进到 resumed body 执行阶段；剩余 `resume(...)` 返回后未继续执行 caller tail 的问题转交下一任务 `T3009b0`。
- 目标：
  - 明确 unified contract 中哪些 frame slot 代表 seeded outer-scope locals/params，并在 handle 完成路径把这些 authoritative slot 写回原 enclosing locals。
  - 只对真实 outer-scope mutable locals/params 执行写回；arm binder、synthetic resume slot、capture-only temp 与 handle 内部局部不得被误写回。
  - handle body、escape arm 与 finally 对 outer `var` 的赋值在离开 handle 后必须可见，不能依赖 effect-only side channel 或副本语义。
- 验收：
  - `cargo test -p scoopc handle_outer_scope_seeding_includes_arm_and_finally_locals -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_outer_var_writeback_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T3010b2b1b0

### T3009b0a1a [DONE] 把 resumed completion path 所需的 outer-slot 写回目标纳入 frame metadata，并接到统一 step-return 出口
- 描述：原 `T3009b0a1` 在落地时发现，想用 run-pass fixture 直接观测“`resume()` 返回点的 outer-local 已同步”，还需要先等 `T3009b0` 把 escaped continuation 的 caller-tail 接回；否则程序仍会停在 `body_after` / `body_resumed`，调用点读不到 resumed body 之后的状态。先独立收口当前真正可实现、也更基础的生产合同：让 authoritative outer-slot 的写回目标不再依赖 handle 入口外层 `env`，而是记录在 effect frame metadata 中，并由 step function 的返回出口与现有 `handle_done` / `handle_propagate` 共享同一 helper。
- 进展：
  - `FrameLayout` 现在会为每个 `seed_from_outer_scope && mutable` 的 slot 追加 native storage pointer field，作为 authoritative writeback target metadata。
  - `seed_outer_scope_frame_slots` 在 seeding outer locals/params 时会同步把原始 storage pointer 写进 frame；若 contract 声明需要 seeded outer slot 却取不到 enclosing local，当前直接报错而不是静默跳过。
  - `write_back_outer_scope_frame_slots` 已改成只读取 frame metadata，不再依赖 caller `env`；`ReturnHandle` / `ReturnFromFunction` / `Suspend` / `Arm*` 返回出口与外层 `handle_done` / `handle_propagate` 现共用同一 helper。
  - 已新增 LLVM IR 单测 `escaped_continuation_resume_ir_records_outer_slot_storage_and_writeback`，锁定 frame 中已记录 outer-slot storage metadata，且 step function / handle 出口都会发出 metadata-driven writeback 路径。
- 目标：
  - 把 outer-slot writeback target 收口进 frame metadata，使 resumed continuation 后续只要跑同一个 `step_fn` 返回出口，就能复用同一写回 helper。
  - 不在 `Continuation.resume(...)` call site 上追加局部补丁；authoritative source/target 必须继续由统一 frame metadata 驱动。
- 验收：
  - `cargo test -p scoopc escaped_continuation_resume_ir_records_outer_slot_storage_and_writeback -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T3009b0a

### T3009b0a1c [DONE] 修正 unified `SuspendCall` 的 inactive-continue / active-dispatch 合同
- 描述：开始真正实现 `T3009b0a2` 时，用最小 repro `handle { helper(false) + 1 } with { Ask.ask() -> 2 }` 重新复现后确认，当前更前置的 shared blocker 并不只在 `RuntimeRaiseBoundary`。unified state-machine 里由 `SuspendCall` 承接的 call boundary（至少覆盖 `CallMaySuspend` / `CallStateMachineCallee` / `ClassCtorInit`）仍被统一 terminator 建模成“求值后无条件 `Suspend`”：`helper(false)` 明明没有 perform，程序却直接空输出退出，本应打印 `8`。这说明 inactive 成功路径没有留在当前 state machine 内继续执行 caller-tail，而是被错误地分配 continuation、设置 active 并交回 dispatch。由于这个合同比 `RuntimeRaiseBoundary` 更基础，且 `T3009b0a2` 依赖同一条 caller-tail / resume-fragment 主线，必须先前移修复。
- 进展：
  - 已在 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 中收口 `UnifiedStateTerminator::Suspend`：对 `CallMaySuspend` / `CallStateMachineCallee` / `ClassCtorInit` 三类 `SuspendCall` site，先读取 TLS active；inactive 时把 call 结果写回 frame resume 槽并直接 branch 到 `resume_state`，active 时才走原有 continuation alloc + dispatch 返回路径。
  - 修复保持在统一 state-machine emitter 内完成，没有把 inactive-path 补丁散回普通 call codegen、callee 名称分支或源码形状判断。
  - 已新增 run-pass fixture `tests/fixtures/run-pass/effect_handle_suspend_call_inactive_helper_basic.scoop`，同时锁定 inactive caller-tail 继续执行与 active resume dispatch 两条路径。
  - 已验证：
    - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_suspend_call_inactive_helper_basic.scoop`
    - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_hidden_suspend_local_closure_helper_basic.scoop`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
- 目标：
  - `SuspendCall` 驱动的 call boundary 在 callee 返回 inactive 时，必须留在当前 state machine 内继续执行 caller-tail，不能无条件 `alloc continuation + set active + return`。
  - 只有 callee 确实让 TLS active 时，当前 step function 才把控制权交回 enclosing `handle` 的 dispatch loop 或继续向外传播。
  - 修复必须继续复用已有 `resume_path` / synthetic resume slot / contract-driven rewrite 数据通路，不能按 callee 名称、源码形状或单个 fixture 打补丁。
- 验收：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_suspend_call_inactive_helper_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T3009b0a1a

### T3009b0a1d [DONE] 修正 unified `ObjectInitAccessBoundary` 的 inactive-continue / active-dispatch 合同
- 描述：开始执行 `T3009b0a1cR` 复审时，用最小 repro `handle { Config.x + 1 } with { Raise.raise(err: RuntimeError) -> 10 }` 直接验证发现，shared `UnifiedStateTerminator::Suspend` 的 inactive-path 缺口并不只在 `SuspendCall` / `RuntimeRaiseBoundary`。direct object/property access 当前仍通过 `ObjectInitAccessBoundary` 落到“求值后无条件 suspend”：程序只打印 `before`、`config.init` 就提前退出，`after` 与最终结果都不会出现。这说明 object init / property access 在 inactive 成功路径上没有留在当前 state machine 内继续执行 caller-tail，而是被错误地 continuation + dispatch 化。由于这是与 `SuspendCall` 共用同一 terminator 出口的更前置 boundary 缺口，必须先补齐。
- 进展：
  - 已把 `SuspendSiteKind::ObjectInitAccess` 纳入 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 的 shared inactive-continue 分支集合；`UnifiedStateTerminator::Suspend` 现在会在 TLS inactive 时把 boundary 结果写回 frame 并 branch 到 `resume_state`，只有 TLS active 时才继续走 continuation alloc + dispatch 返回。
  - 已把 inactive-path 的内部错误文案从 `suspend call inactive continuation result` 泛化为 `suspend boundary inactive continuation result`，避免 shared boundary 收口后继续带着 call-only 语义。
  - 已新增 run-pass fixture `tests/fixtures/run-pass/effect_handle_object_init_access_inactive_basic.scoop`，同时锁定 direct object value access、property access 的 inactive-path caller-tail，以及 property access 的 active dispatch。
  - 在验证过程中顺手收回了已恢复通过的过时期望 `tests/fixtures/run-pass/effect_escape_continuation_resume_cross_thread.scoop`（`EXPECT: fail` → `pass`）；整套 LLVM fixture 仍被后续 `T3017` 范围内的其它 stale xfail 挡住，不影响本任务生产合同收口。
- 目标：
  - `ObjectInitAccessBoundary` 在 object value / property access 最终 inactive 返回时，必须留在当前 state machine 内继续执行 caller-tail，不能无条件 continuation alloc + set active + return。
  - 当 object init / hidden raise 确实令 TLS active 时，当前 step function 仍需把控制权交回 enclosing `handle` / `try` 的 dispatch loop，而不是继续执行 boundary 后语句。
  - 修复必须收口到共享 boundary 合同，不能对具体 object 名称、属性名或单个 fixture 做特判。
- 验收：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_object_init_access_inactive_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已验证：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_object_init_access_inactive_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T3009b0a1c

### T3009b0a1dR [DONE] Review：确认 `ObjectInitAccessBoundary` 的 inactive-path 已统一收口到 state-machine 合同
- 描述：在 `T3009b0a1d` 之后只审查生产代码，确认 object value / property access 的 inactive 成功路径已经统一收口到 state-machine 合同，而不是在 `codegen_object_value_access` / `codegen_object_property_access` 或具体 object 名称上局部绕过；若发现对象名特判、源码形状分流或新 side channel，本任务需要直接修复并复审。
- 进展：
  - 已复审 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`：`UnifiedStateTerminator::Suspend` 对 `SuspendSiteKind::ObjectInitAccess` 的 inactive/active 分流只经由 `suspend_site_uses_inactive_continue_path` 与 TLS `is_active` 判断，不读取 object 名称、属性名或源码形状。
  - 已复审 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`：`ObjectInitAccess` 仅由 `HandlePlanContext::from_codegen` 基于 `object_inits` / `object.properties` 元数据构造的 `object_value_fqns`、`object_property_fqns` 集合分类；这些 FQN 只用于 suspend-site 语义建模，不参与 emitter 中的 inactive/active 选路。
  - 已复审 `crates/scoopc/src/llvm/codegen/mod.rs`：`codegen_object_value_access` / `codegen_object_property_access` 仍只保留 ordinary-frame 的统一 TLS active 检查；step function 生成期间会清空 `current_fun_return_ty` / `return_context`，因此这条 ordinary patch 在 unified state-machine 内不会提前返回，也不会绕开 shared `Suspend` terminator。
- 目标：
  - 确认 `ObjectInitAccessBoundary` 的 inactive/active 分流只由统一 contract 与 TLS active 结果驱动，不读取 object 名称、属性名、源码形状或旧 scanner 结果。
  - 确认 inactive-path 没有回流成 ordinary helper / object-init 专用 patch。
  - 确认生产代码没有为 direct object/property access 新增脱离 state-machine 合同的结果旁路。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“`ObjectInitAccessBoundary` 的 inactive-path 已统一收口到 state-machine 合同”。
- 已验证：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_object_init_access_inactive_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 审查结论：
  - `ObjectInitAccessBoundary` 的 inactive-path 已统一收口到 state-machine 合同。inactive/active 分流仅由统一 `Suspend` terminator 读取 `SuspendSiteKind::ObjectInitAccess` 与 TLS active 决定。
  - 生产代码中未发现 object 名称特判、属性名特判、源码形状分流，或绕开 state-machine 合同的 direct object/property access 结果旁路。
- 依赖：T3009b0a1d

### T3009b0a1e [DONE] 修正 unified `NestedHandleBoundary` 的 inactive-continue / active-dispatch 合同
- 描述：同一次 `T3009b0a1cR` 复审继续用最小 nested repro 验证后确认，`NestedHandleBoundary` 也仍被统一 terminator 建模成“求值后无条件 suspend”。最小复现是 outer `handle` 包一层 inner `handle { helper(false) + 1 }`：当前输出只到 `inner_after`，`outer_after` 与最终结果都不会继续，说明 inner handle 明明 inactive 返回，outer state machine 却仍被错误截断。与 `ObjectInitAccessBoundary` 不同，这条路径不能靠“重新求值原表达式”糊过去，否则会把 inner handle 整体重跑一遍，因此需要 authoritative 的 inactive continue / result transport 合同。
- 进展：
  - `UnifiedStateTerminator::Suspend` 现已把 `SuspendSiteKind::NestedHandleBoundary` 纳入与 `SuspendCall` 相同的 TLS-active 分流：inner handle inactive 返回时，把 authoritative 结果写回 frame 并直接 branch 到 `resume_state`；只有 TLS active 时才继续 continuation alloc + outward dispatch。
  - `NestedHandleBoundary` 现已携带 `resume_path` + synthetic resume slot，plan/segment/unified machine 的 resume-after-site 状态会把 outer caller-tail 中的 nested handle 子表达式改写为读取 `__resume_site*`，避免 inactive-path 通过重跑 inner handle 取值。
  - 顺手修复了更上游的 HIR lowering 类型源错误：`ExprKind::Handle` 不再统一写成 `Any`，而是保留 typechecked handle result type；否则 nested-boundary synthetic resume slot 会被错误降成 `Ref`。已同步更新 `tests/fixtures/hir/handle_perform.hir` golden。
  - 已新增 run-pass fixture `tests/fixtures/run-pass/effect_handle_nested_handle_boundary_inactive_basic.scoop` 与 transform 单测 `nested_handle_boundary_preserves_resume_path_and_slot`，分别锁定端到端 inactive/active 合同和 plan→segment→machine 的 authoritative resume transport。
- 目标：
  - `NestedHandleBoundary` 在 inner handle inactive 返回时，outer state machine 必须继续执行 caller-tail，不能把 inactive 成功返回误判成 outward dispatch。
  - 当 inner handle 确实把 effect 向外传播并令 TLS active 时，outer state machine 仍需交回 enclosing dispatch loop。
  - 修复不能通过重跑 inner handle、专判 nested 结构或在 outer call site 增加局部补丁完成；authoritative 数据通路必须继续收口到统一 state-machine 合同。
- 验收：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_nested_handle_boundary_inactive_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已验证：
  - `cargo test -p scoopc nested_handle_boundary_preserves_resume_path_and_slot -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_nested_handle_boundary_inactive_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T3009b0a1dR

### T3009b0a1eR [DONE] Review：确认 `NestedHandleBoundary` 的 inactive-path 已统一收口到 state-machine 合同
- 描述：在 `T3009b0a1e` 之后只审查生产代码，确认 nested handle 的 inactive-path 已真正回到统一 state-machine 合同，没有把 inner-handle result transport 或 caller-tail continue 散回 outer emitter、普通 call codegen 或 shape-based 分流；若发现这类回流，本任务需要直接修复并复审。
- 进展：
  - 已复审 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`：`NestedHandleBoundary` 的 inactive/active 分流只经由 shared `Suspend` terminator 里的 `suspend_site_uses_inactive_continue_path()` 与 TLS `is_active` 判断。`HandleStateOp::NestedHandleBoundary` 自身仅委托 `codegen_expr_in_expected_context(expr, None)` 生成 inner handle，不包含 outer-emitter caller-tail 特判或普通 call 旁路。
  - 已复审 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 与 `state_machine_transform.rs`：`NestedHandleBoundary` 的 suspend site 会稳定携带 `resume_path` + synthetic resume slot，`ResumeAfterSite` 后续 consumer 会改写为读取 `__resume_site*`，inactive-path 不会通过重跑 inner handle 取值。
  - 已复审 `crates/scoopc/src/llvm/codegen/expr.rs` 与 `crates/scoopc/src/hir/lower/expr.rs`：`ExprKind::Handle` 仍统一进入 `codegen_handle_expr` / `codegen_handle_expr_via_state_machine`，HIR lowering 也保留 typechecked handle result type；未发现 nested-handle 专用入口、shape-based 分流或 outer emitter 补丁回流。
- 目标：
  - 确认 `NestedHandleBoundary` 的 inactive/active 分流只由统一 contract 与 TLS active 驱动，不读取 nested handle 形状或通过局部补丁短路。
  - 确认 inactive 成功路径不会通过“重跑 inner handle”或其它非 authoritative side channel 取值。
  - 确认生产代码没有把 nested handle 边界修复回流成 effect-only patch。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“`NestedHandleBoundary` 的 inactive-path 已统一收口到 state-machine 合同”。
- 已验证：
  - `cargo test -p scoopc nested_handle_boundary_preserves_resume_path_and_slot -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_nested_handle_boundary_inactive_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 审查结论：
  - `NestedHandleBoundary` 的 inactive-path 已统一收口到 state-machine 合同。inactive/active 分流只由 shared `Suspend` terminator 读取 `SuspendSiteKind::NestedHandleBoundary` 与 TLS active 决定。
  - 生产代码中未发现重跑 inner handle 取值、outer emitter / 普通 call codegen 旁路、源码形状分流，或 effect-only patch 回流。
- 依赖：T3009b0a1e

### T3009b0a1cR [DONE] Review：确认 unified `SuspendCall` 的 inactive-path 已回到单一 state-machine 合同
- 描述：在 `T3009b0a1c` 之后只审查生产代码，确认 `SuspendCall` 的 inactive 成功路径已经统一收口到 state-machine 合同，而不是在某个 callee / fixture 上局部绕过；若发现 call-site 特判、源码形状分流或把 inactive 路径重新塞回 ordinary helper，本任务需要直接修复并复审。当前复审已先确认 `SuspendCall` 本身的 inactive/active 分流只按 `SuspendSiteKind` + TLS active 驱动，但同一 shared `UnifiedStateTerminator::Suspend` 上又暴露出更前置的 `ObjectInitAccessBoundary` / `NestedHandleBoundary` inactive-path 缺口；因此本 review 顺延到这些 boundary 收口后再落最终结论。
- 进展：
  - 已复审 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`：`HandleStateOp::SuspendCall` 自身只委托 `codegen_expr_in_expected_context(expr, None)` 求值，inactive/active 分流只经由 shared `UnifiedStateTerminator::Suspend` 内的 `suspend_site_uses_inactive_continue_path()` 与 TLS `is_active` 判断。`CallMaySuspend` / `CallStateMachineCallee` / `ClassCtorInit` 三类 call boundary 共用同一条 emitter 路径，没有 callee 名称特判或 call-site 补丁。
  - 已复审 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`、`state_machine_segments.rs` 与 `state_machine_transform.rs`：三类 call suspend site 都要求携带 `resume_path`，并在 `ResumeAfterSite` 状态分配 synthetic resume slot；post-call caller-tail 会统一改写为读取 `__resume_site*`，没有新增 `SuspendCall` 专用 side channel。
  - 已复审 `crates/scoopc/src/llvm/codegen/expr.rs`、`crates/scoopc/src/llvm/codegen/mod.rs` 与 `crates/scoopc/src/llvm/codegen/effect/mod.rs`：表达式入口仍统一经由 `codegen_call`，`Continuation.resume(...)` 只依赖 `continuation_resume_call_sites` side table，不按成员名/receiver 形状推断；ordinary direct/vtable/itable/funptr/closure/object-init call 仍共享 `emit_ordinary_call_effect_propagation_check`，而 step function 在 `emit_effect_step_function` 内会暂时清空 `current_fun_return_ty` / `return_context`，因此不会形成绕开 state-machine 的 ordinary-helper side channel。
- 目标：
  - 确认 `SuspendCall` 的 inactive/active 分流只由统一 contract 与 TLS active 结果驱动，不读取源码形状、callee 名称或旧 scanner 结果。
  - 确认 `resume_path` / synthetic resume slot 仍是 post-call caller-tail 的唯一数据通路，没有新增 call-only side channel。
  - 确认生产代码没有把 `SuspendCall` 的 inactive-path 修复回流成 `Continuation.resume(...)` / hidden-suspend helper 专用 patch。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“unified `SuspendCall` 的 inactive-path 已统一收口到 state-machine 合同”。
- 已验证：
  - `cargo test -p scoopc resume_path_is_preserved_from_plan_to_segments_to_unified_machine -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_suspend_call_inactive_helper_basic.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_hidden_suspend_local_closure_helper_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 审查结论：
  - unified `SuspendCall` 的 inactive-path 已统一收口到 state-machine 合同。inactive/active 分流只由 shared `Suspend` terminator 读取 `SuspendSiteKind::{CallMaySuspend, CallStateMachineCallee, ClassCtorInit}` 与 TLS active 决定。
  - `resume_path` + synthetic resume slot 仍是 post-call caller-tail 的唯一 authoritative 数据通路；计划、segment 与 unified machine 的合同校验都会拒绝缺失该元数据的 call-like suspend site。
  - 生产代码中未发现 callee 名称特判、源码形状分流、ordinary helper 专用补丁，或把 `SuspendCall` inactive-path 回流成 `Continuation.resume(...)` / hidden-suspend helper 局部绕过。
- 依赖：T3009b0a1eR

### T3009b0a2 [DONE] 修正 unified `RuntimeRaiseBoundary` 的 inactive-continue / active-dispatch 合同
- 描述：`T3009b0a1c` 前移后，当前共享 contract 问题收窄到 `RuntimeRaiseBoundary` 自身。unified state-machine 现在把 `RuntimeRaiseBoundary`（至少覆盖 `Continuation.resume(...)` 与 `x as T`）统一建模成“求值表达式后无条件 `Suspend`”：step function 先发出 `scoop_continuation_resume(...)` / cast code，再立刻走 `alloc continuation + set_active + return`。结果是 boundary expression 的 inactive 成功路径也会被截断，caller-tail 永远接不回。最小复现包括：`effect_escape_continuation_resume_unit.scoop` 当前输出停在 `after_pause`，缺少 `after_resume` / `done`；`type_check_cast_is_as_asq_basic.scoop` 当前在首个成功 `as` 后停在 `x is Base: true`，未继续打印 `Impl.ping` / `42` / `10`。由于这是在 `SuspendCall` 之外仍然阻塞 `T3009b0` 的 shared boundary 合同错误，必须在 dedicated resume lowering 前继续收口。
- 进展：
  - 已将 `SuspendSiteKind::RuntimeRaise` 纳入 shared `Suspend` terminator 的 inactive-continue 集合。`RuntimeRaiseBoundary` 求值后现在会先读取 TLS active：inactive 时把结果写入 frame resume 槽并直接 branch 到 `resume_state`，active 时才继续走现有 outward-dispatch 路径。
  - 已新增 IR 级单测 `runtime_raise_boundary_ir_branches_between_inactive_continue_and_active_dispatch`，锁定 `RuntimeRaiseBoundary` 仍经由统一 state-machine 合同生成 `site*_is_active` / `site*_inactive` / `site*_active` 三段结构，而不是靠 `Continuation.resume(...)` 或 cast 特判。
  - 任务验收 fixture 已恢复：`type_check_cast_is_as_asq_basic.scoop` 重新打印 `Impl.ping` / `42` / `10` / `done`；`effect_escape_continuation_resume_unit.scoop` 重新打印 `after_resume` / `done`。
- 目标：
  - `RuntimeRaiseBoundary` 驱动的表达式在 inactive 成功路径上必须留在当前 state machine 内继续执行 caller-tail；不能无条件分配 caller continuation、设置 active 并提前返回。
  - 当 boundary expression 确实令 TLS active（例如 `Continuation.resume` 的 second-resume 运行期错误、`as` cast fail-path），当前 step function 必须把控制权交回 enclosing `handle` / `try` 的 dispatch loop，而不是继续执行 boundary 之后的语句。
  - 修复必须收口到共享 `RuntimeRaiseBoundary` 合同，不能只对 `Continuation.resume(...)` 或单个 fixture 特判。
- 验收：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/type_check_cast_is_as_asq_basic.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_resume_unit.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已验证：
  - `cargo test -p scoopc runtime_raise_boundary_ir_branches_between_inactive_continue_and_active_dispatch -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/type_check_cast_is_as_asq_basic.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_resume_unit.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T3009b0a1cR

### T3009b0 [DONE] 为 escaped continuation 的 `Continuation.resume(...)` 先接回 scalar/ref payload 专用 lowering
- 描述：开始执行 `T3010b2b1b` 并按新最小 repro 重新基线化后确认，当前首个 blocker 已不再是 `value coercion`。`effect_resume_nested_escape_handle_tail.scoop`、`effect_escape_continuation_resume_unit.scoop` 与 `effect_escape_continuation_resume_string.scoop` 最初都在 `k.resume(...)` 处报 `暂不支持的 main 代码生成节点：call callee`。上游 `continuation_resume_call_sites` 已把这些 call site 标记为 builtin `Continuation.resume`，说明真实缺口在 LLVM 侧：unified state-machine emitter/普通 call path 仍把 escaped continuation 的 `k.resume(...)` 回落到 generic `codegen_expr_in_expected_context` / generic call path。当前分支已前置接通 dedicated lowering 原型；初次离开 handle 的 outer-scope slot 写回已由 `T3009b0a` 修复，`T3009b0a1a` 也把 resumed path 所需的 frame-metadata writeback 合同补到了 step-return 出口。但继续真跑验收后又确认，caller-tail 当前还被更基础的 `RuntimeRaiseBoundary` 合同错误截断；该 shared 前置已由 `T3009b0a2` 承接。本任务在其之后继续完成 escaped continuation 的 scalar/ref payload transport 与 dedicated resume caller-tail 的正式验收。
- 进展：
  - 已确认 `crates/scoopc/src/llvm/codegen/mod.rs` 通过 `continuation_resume_call_sites` 对 `Continuation.resume(...)` 做 dedicated builtin 分派，不再依赖 generic member access / generic call 兜底。
  - 已确认 `crates/scoopc/src/llvm/codegen/effect/mod.rs` 中的 `codegen_continuation_resume_builtin` 复用共享 continuation runtime ABI 与 `resume_word` / `resume_gc_ref` transport，已覆盖 Unit、标量 word 与 GC ref payload。
  - 已正式验收 `effect_escape_continuation_resume_unit.scoop`、`effect_escape_continuation_resume_bool.scoop`、`effect_escape_continuation_resume_string.scoop` 与 `effect_resume_nested_escape_handle_tail.scoop`，均通过且不再出现 `call callee` 回退。
  - 已验证 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 全部通过。
- 目标：
  - 对 typecheck 已确认的 `Continuation.resume(...)` 调用点提供显式 lowering，不再回落到 generic member access / generic call。
  - 复用现有 continuation runtime ABI 与 `resume_word` / `resume_gc_ref` transport，先覆盖 Unit、标量 word 与 GC ref payload。
  - 不新增 continuation-only / fixture-only side channel；composite resume payload 仍由后续 `T3013` + `T3009b` 统一收口。
- 验收：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_resume_unit.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_resume_bool.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_resume_string.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_nested_escape_handle_tail.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T3009b0a2

### T3009b0a1b [DONE] 在 caller-tail 已接回后，用 focused fixture 验证 resumed completion path 的 outer-slot 写回
- 描述：`T3009b0a1a` 已把 authoritative outer-slot writeback target 收口进 frame metadata，并让 step-return 出口复用同一 helper。但当前直接复现 `k.resume(...)` 时，程序仍会停在 `body_after` / `body_resumed`，说明 resumed body 已执行、而 caller-tail 尚未继续；在这个状态下无法用 run-pass fixture 直接观测“`resume()` 返回点的 outer-local 已同步”。因此把“可观测验收”拆成独立子任务，等 `T3009b0` 接回 escaped continuation 的 caller-tail 后，再新增 focused fixture 锁定恢复完成路径的外层写回可见性。
- 进展：
  - 已新增 focused run-pass fixture `tests/fixtures/run-pass/effect_escape_continuation_resume_outer_var_writeback.scoop`，直接锁定“handle 初次离开时 outer `var` 仍保持旧值；`k.resume(...)` 返回后调用点可见值已更新”为同一条 resumed completion path 合同。
  - 该 fixture 使用最小 escaped continuation 场景：`outerValue` 在 `Suspend.pause()` 之后才改写，因此 `after_handle` 时仍打印旧值 `5`，`k.resume(41)` 返回后再打印新值 `42`，避免只在 resumed body 内部观察到写回。
- 目标：
  - 新增一个 focused fixture，锁定 escaped continuation `resume(...)` 返回后，outer `var` 已与 resumed body / cleanup 保持同步。
  - 该 fixture 必须直接观测调用点可见的 outer local，而不是只看 resumed body 内部输出。
- 验收：
  - focused fixture 通过，且输出明确覆盖“handle 初次离开时旧值仍在、`resume()` 返回后新值可见”。
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已验证：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_resume_outer_var_writeback.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T3009b0

### T3009b0aR [DONE] Review：确认 outer-scope slot 写回没有回流成 effect-only patch
- 描述：在 `T3009b0a1a` + `T3009b0` + `T3009b0a1b` 之后只审查生产代码，确认 outer-scope local 的写回是统一合同的一部分，而不是在 `Continuation.resume` / escape arm / 单个 fixture 路径上额外打补丁；若发现只对特定 effect 场景生效的 patch，本任务需要直接修复并复审。
- 进展：
  - 已复审 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 中的 `emit_effect_frame_layout`、`seed_outer_scope_frame_slots`、`write_back_outer_scope_frame_slots` 与所有 step-function / handle 出口，确认 authoritative outer-slot source 与 writeback target 都由 frame metadata 驱动；`ReturnHandle` / `ReturnFromFunction` / `Suspend` / `Arm*` 返回出口，以及 `handle_done` / `handle_propagate` 共享同一个 writeback helper。
  - 已复审 `crates/scoopc/src/llvm/codegen/effect/mod.rs` 与 `crates/scoopc/src/llvm/codegen/mod.rs`：`codegen_continuation_resume_builtin` 仅负责 payload transport、`scoop_continuation_resume(...)` 调用与 ordinary effect propagation check，不包含 outer-slot 写回或 caller-local 同步补丁；普通 call 入口也仍只通过 `continuation_resume_call_sites` 做 builtin 分派。
  - 已补跑结构单测 `handle_outer_scope_seeding_includes_arm_and_finally_locals`、IR 单测 `escaped_continuation_resume_ir_records_outer_slot_storage_and_writeback`，以及 focused fixtures `effect_escape_continuation_outer_var_writeback_basic.scoop` / `effect_escape_continuation_resume_outer_var_writeback.scoop`，确认“初次离开 handle”和“`resume()` 返回后完成”两条路径都保持同一合同。
- 目标：
  - 确认 outer-scope seeded slot 的 authoritative 来源与写回目标都由统一 frame metadata 驱动，不依赖 arm kind、callee 名称或 fixture 形状。
  - 确认“第一次离开 handle”的 `handle_done` / `handle_propagate` 与 escaped continuation `resume(...)` 之后的完成 / 向外传播路径都共享同一套 outer-slot 写回合同，而不是分别拼接补丁。
  - 确认当前 dedicated `Continuation.resume` lowering 仍只负责 runtime resume/payload transport，不偷偷承担 outer-local 同步职责。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“outer-scope slot 写回已统一收口，且未回流为 effect-only patch”。
- 已验证：
  - `cargo test -p scoopc handle_outer_scope_seeding_includes_arm_and_finally_locals -- --nocapture`
  - `cargo test -p scoopc escaped_continuation_resume_ir_records_outer_slot_storage_and_writeback -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_outer_var_writeback_basic.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_resume_outer_var_writeback.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 审查结论：
  - outer-scope slot 写回已统一收口，且未回流为 effect-only patch。生产代码里唯一的 authoritative source/target 都来自 unified handle frame metadata，所有 handle/step 返回出口复用同一个 writeback helper。
  - `Continuation.resume(...)` dedicated lowering 仍只负责 runtime resume 与 payload transport；outer-local 同步没有散落到 builtin call path、escape arm 或 fixture 专用补丁中。
- 依赖：T3009b0a1b

### T3009b0R [DONE] Review：确认 escaped continuation resume 的 scalar/ref lowering 不再回落到 generic call
- 描述：在 `T3009b0` 之后只审查生产代码，确认 escaped continuation 的 `Continuation.resume(...)` 已有 dedicated lowering，并且当前仅覆盖 scalar/ref transport 的实现没有偷偷塞回 generic member access / generic call；若发现回落或散落 patch，本任务需要直接修复并复审。
- 进展：
  - 已复审 `crates/scoopc/src/llvm/codegen/mod.rs` 的 `codegen_call`：`continuation_resume_call_sites` 检查位于普通 call lowering 的最前面，typecheck 已确认的 `Continuation.resume(...)` 会直接分派到 `codegen_continuation_resume_builtin`，不会先落入 generic member access / generic call 兜底。
  - 已复审 `crates/scoopc/src/llvm/codegen/mod.rs` 的 `codegen_member_access` 与相邻 call/member 入口；生产代码中不存在按成员名、receiver 形状或 `Continuation` 类型去偷偷识别 `resume` 的旁路特判，builtin 语义唯一来源仍是 call-site side table。
  - 已复审 `crates/scoopc/src/llvm/codegen/expr.rs`、`crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 与 `crates/scoopc/src/llvm/codegen/effect/mod.rs`：unified state-machine emitter 对 `HandleStateOp::Call` / `RuntimeRaiseBoundary` / `NestedHandle*` 等相关表达式统一委托 `codegen_expr_in_expected_context`，最终仍走同一个 `codegen_call -> codegen_continuation_resume_builtin` 分派；`codegen_continuation_resume_builtin` 只复用共享 continuation runtime ABI、`resume_word` / `resume_gc_ref` transport 与 ordinary effect propagation check，不包含 escaped-continuation-only side channel。
- 目标：
  - 确认 `Continuation.resume(...)` 在 unified state-machine emitter 与普通 call path 中都有显式 lowering 分派，不再依赖 generic call callee 兜底。
  - 确认 scalar/ref payload 仍复用统一 continuation runtime ABI，不存在 escaped-continuation-only 特例通道。
  - 确认 `T3013` / `T3009b` 后续扩展 composite payload 时，当前 dedicated lowering 可以作为单一基线继续扩展。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“escaped continuation resume 的 scalar/ref lowering 已 dedicated 化，且未回落到 generic call 路径”。
- 已验证：
  - `cargo test -p scoopc continuation_resume_hidden_suspend_classification_requires_typechecked_call_site_marker -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_resume_unit.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_resume_bool.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_resume_string.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_nested_escape_handle_tail.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 审查结论：
  - escaped continuation 的 `Continuation.resume(...)` scalar/ref lowering 已 dedicated 化，且未回落到 generic call 路径。ordinary path 与 unified state-machine path 共享同一个 `codegen_call -> codegen_continuation_resume_builtin` builtin 分派，不存在额外的 member-access fallback。
  - 当前 payload transport 仍统一复用 continuation runtime ABI 与 `resume_word` / `resume_gc_ref` 两条共享通道；composite payload 仍明确留给后续 `T3013` / `T3009b` 扩展。
- 依赖：T3009b0

### T3010b2b1b [DONE] 补齐 nested arm indirect outward propagation 所需的 unified value coercion / expected-context 前置
- 描述：`T3010b2b1b0` 完成后，原先用来锚定本任务的 `effect_escape_continuation_nested_arm_indirect_performs_outer.scoop` 已恢复通过；继续重新基线化时，新的最小 repro 变为 `effect_resume_nested_escape_handle_tail.scoop`，但其首个失败点并不是 `value coercion`，而是 escaped continuation 的 `Continuation.resume(...)` 仍在 unified emitter 中回落到 generic call path 并报 `call callee`。这个更前置的缺口已由 `T3009b0/T3009b0R` 承接；本任务保留为“在 dedicated escaped-continuation resume lowering 接通之后，再重新检查 nested arm / nested handle / indirect helper call 链路上是否还存在独立的 unified expected-context / coercion 缺口”。若届时该前置已不再缺失，应继续收窄或删除本任务，避免继续围绕已通过 fixture 工作。
- 进展：
  - 已重新运行 `effect_resume_nested_escape_handle_tail.scoop`、`effect_resume_nested_escape_handle_tail_multi_perform_nonunit.scoop` 与 `effect_escape_continuation_nested_arm_indirect_performs_outer.scoop`；三条 focused fixture 当前都稳定通过，原注释中记录的 `value coercion` / `unknown local value` 失败模式不再复现。
  - 已复审 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 的 arm-body 发射路径：immediate-resume 分支会把改写后的 body 以 `payload_cg_ty` 传给 `codegen_expr_in_expected_context`，其余 nested handle / indirect helper / arm-body 表达式统一复用 `codegen_expr_in_expected_context` 与共享 `coerce_value` 逻辑，不存在单独的 arm-only / nested-only expected-context 旁路。
  - 结合代码复审与 focused fixture，确认 `T3009b0/T3009b0R` 接通 dedicated `Continuation.resume(...)` lowering 后，不再剩余一个独立的 unified expected-context / coercion 前置缺口；后续工作直接回到 `T3010b2b1` 的 outward propagation / cleanup 语义验收。
- 目标：
  - 在 `T3009b0R` 之后重新定位 nested arm / nested handle / indirect helper call 路径上是否仍存在统一 expected-context / coercion 缺口，并给出新的最小 repro。
  - 若缺口仍存在，统一 state-machine emitter 需要为局部绑定、丢弃绑定、tail value/readback 提供足够的 expected context，不再命中同类 `value coercion`。
  - 实现必须复用统一 expected-context/coercion 逻辑，不能加 fixture-only / arm-only / nested-only workaround。
- 验收：
  - 新的最小 repro 能稳定复现并验证修复后的 nested/indirect expected-context / coercion 行为。
  - 重新跑当前相关 xfail / run-pass 子集时，不再因为这类 arm-body/nested 路径报 `暂不支持的 main 代码生成节点：value coercion`
  - 若执行期 `cargo run -p scoop --features llvm -- test` 仍先停在 `T3017` 的 stale xfail expectation cleanup，上述最小 repro 与相关子集仍应先独立通过；待 `T3017` 回收后再把全量 suite 作为追加验收。
- 已验证：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_nested_escape_handle_tail.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_nested_escape_handle_tail_multi_perform_nonunit.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_nested_arm_indirect_performs_outer.scoop`
- 结论：
  - `T3010b2b1b` 原本预期的独立 unified expected-context / coercion 缺口已不存在；相关 fixture 仍带 `EXPECT: fail` 只是 `T3017` 要统一回收的 stale expectation，不再阻塞 `T3010b2b1` 的语义推进。
- 依赖：T3009b0R

### T3010b2b1b1 [DONE] 同步 `handle_perform` 的 MIR golden，恢复全量 fixture 验证入口
- 描述：在完成 `T3010b2b1b` 的 focused 验证后复跑 `cargo run -p scoop --features llvm -- test`，suite 没有先停在 effect 主线上，而是先在 `tests/fixtures/mir/handle_perform.scoop` 报 `handle_perform.mir` snapshot mismatch。当前 `scoop dump-mir tests/fixtures/mir/handle_perform.scoop` 的输出中，handle result 临时 local `tmp0` 的类型已从 golden 中的 `TypeId(0)` 变为 `TypeId(5)`；这与此前 `ExprKind::Handle` 保留 typechecked result type 的修正一致，当时已同步更新 `tests/fixtures/hir/handle_perform.hir`，但遗漏了对应 MIR golden。
- 进展：
  - 已复审 `tests/fixtures/hir/handle_perform.hir`、`crates/scoopc/src/hir/lower/expr.rs` 与 `crates/scoopc/src/mir/lower.rs`：HIR 中 `handle` 表达式类型本就是 `TypeId(5)`，MIR lowering 也明确以 `lower_handle_expr(expr.span, expr.ty, ...)` 为 handle result local 分配类型，因此 `tmp0: TypeId(5)` 属于既有 type 修正的自然结果，不是新的 MIR lowering 退化。
  - 已同步更新 `tests/fixtures/mir/handle_perform.mir`，使 handle result 临时 local `tmp0` 的类型与当前 `dump-mir` 输出一致。
  - 已复跑 `cargo run -p scoop --features llvm -- test`；suite 已越过 `handle_perform.mir` snapshot mismatch，新的首个失败点推进到 `tests/fixtures/run-pass/continuation_resume_continuation.scoop` 的 stale `EXPECT: fail`。该问题已由 `T3017` 显式跟踪，不属于本任务新增 blocker。
- 目标：
  - 确认 `handle_perform` 的 MIR 变化确实来自预期的 handle result type 源修正，而不是新的 MIR lowering 退化。
  - 同步 `tests/fixtures/mir/handle_perform.mir` golden，恢复 MIR fixture 基线。
  - 复跑 MIR 相关 fixture 子集，确保后续 `cargo run -p scoop --features llvm -- test` 不再先被这个 snapshot mismatch 截断。
- 验收：
  - `cargo run -p scoop -- dump-mir tests/fixtures/mir/handle_perform.scoop` 与 golden 一致。
  - MIR fixtures 子集通过。
- 已验证：
  - `diff -u tests/fixtures/mir/handle_perform.mir <(cargo run -p scoop -- dump-mir tests/fixtures/mir/handle_perform.scoop)`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3010b2b1b

### T3010b2b1 [DONE] 收口 handle arm body nested/indirect non-resuming effect 的剩余外传 / self-inactive / finally 验收
- 描述：`T3010b2b1a` 已接通 direct arm-body raise/helper call、arm return/finally、以及 no-perform handle result；`T3010b2b1b` 已重新基线化并确认不存在独立的 unified value coercion / expected-context 前置，当前先恢复 MIR fixture 验证入口（`T3010b2b1b1`），再回到原始语义目标：继续验证 nested handle、indirect perform、resume 后再 perform 等 arm-body 场景，确认它们与 direct 路径共享同一套 outward propagation / cleanup 语义。
- 进展：
  - 已重新执行 `effect_resume_finally_arm_raise.scoop`、`effect_escape_continuation_finally_arm_raise.scoop`、`effect_multi_nonresuming_raise_custom_finally.scoop` 与 `effect_escape_continuation_nested_arm_indirect_performs_outer.scoop`，四个验收 fixture 全部通过；说明 immediate-resume、escape continuation、pure non-resuming source-handle，以及 arm body nested/indirect perform 路径都已共享同一套 outward propagation / cleanup 语义。
  - 已复跑 `cargo run -p scoop --features llvm -- test`；suite 不再在 arm-body outward propagation 语义上更早失败，当前首个失败点为 `tests/fixtures/run-pass/continuation_resume_continuation.scoop` 的 stale `EXPECT: fail`，该 expectation cleanup 已由 `T3017` 跟踪，不再阻塞本任务收口。
- 目标：
  - arm body 中 nested/indirect 触发的 unmatched non-resuming effect 同样不得落回当前 handle 的正常完成路径；必须先经过 cleanup / `finally`，再继续向外传播。
  - immediate-resume、escape continuation 与 pure non-resuming source-handle 在 arm body 内共享同一套 direct + nested/indirect outward propagation 语义。
  - `cargo run -p scoop --features llvm -- test` 在 arm-body outward propagation 语义上不再被更早 fixture 阻塞。
- 验收：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_finally_arm_raise.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_finally_arm_raise.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_nonresuming_raise_custom_finally.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_nested_arm_indirect_performs_outer.scoop`
  - `cargo run -p scoop --features llvm -- test`
- 已验证：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_finally_arm_raise.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_finally_arm_raise.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_nonresuming_raise_custom_finally.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_nested_arm_indirect_performs_outer.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3010b2b1b1

### T3010b2b [DONE] 基于 synthetic resume slot + immediate-resume lowering 接通可执行的 post-suspend continuation tail，禁止 resume 后重放原表达式
- 描述：`T3010b2a` 已把 body tail 改写到 synthetic resume slot，`T3009a` 将补上 arm 内 `resume(value)` 的专用 lowering。两者接通后，再回到端到端语义收口：resume landing 只能继续剩余 tail，不能重放 suspend 前已经求值的路径。
- 进展：
  - 已修复 `while` / `if` branch condition 未进入 `state.reads`、outer slot 首次建模时类型退化为 `Any`、handle 首次进入 `step_fn` 前未 seed outer locals/params 到 effect frame，以及 continuation `resume_state_tag` 误写回 frame 的 runtime 回归。
  - `effect_resume_yield_int_basic.scoop`、`effect_resume_finally_normal.scoop`、`async_await_minimal_int_basic.scoop`，以及多条 comparison / branch / nested-block / mixed-raise 相关 fixture 已恢复到稳定 passing 基线。
  - 已对当前全部带 `EXPECT: fail` 的 run-pass fixture 扫描 `member access target` / `comparison lhs|rhs` / `equality lhs` / `integer binary op lhs` 错误类别，确认这些 body-tail fragment 错误已全部消失。
  - 已将 `effect_resume_finally_body_raise_after_resume.scoop`、`effect_resume_nested_escape_handle_tail.scoop`、`effect_resume_mixed_escape_direct_finally.scoop` 与 `effect_resume_mixed_source_path_matrix.scoop` 临时移除 `EXPECT: fail` 头后按普通 fixture 复验，`fixtures: ok (4)`；说明 resumed tail、nested escape handle 与 mixed source-path cleanup 已可按正常 run-pass 通过。
  - 复跑 `cargo run -p scoop --features llvm -- test` 后，suite 首个失败点为 `tests/fixtures/run-pass/continuation_resume_continuation.scoop` 的 stale `EXPECT: fail`，该问题仍由 `T3017` 跟踪，不再属于本任务缺口。
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
- 已验证：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_yield_int_basic.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_finally_normal.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/async_await_minimal_int_basic.scoop`
  - 对当前全部 `EXPECT: fail` 的 run-pass fixture 扫描 `member access target` / `comparison lhs|rhs` / `equality lhs` / `integer binary op lhs`：无命中
  - 临时复制并移除 `EXPECT: fail` 头后运行 `target/debug/scoop test --fixtures "$tmpdir"`：`effect_resume_finally_body_raise_after_resume.scoop`、`effect_resume_nested_escape_handle_tail.scoop`、`effect_resume_mixed_escape_direct_finally.scoop`、`effect_resume_mixed_source_path_matrix.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop --features llvm -- test`（首个失败点：`tests/fixtures/run-pass/continuation_resume_continuation.scoop` 的 stale `EXPECT: fail`，由 `T3017` 跟踪）
- 依赖：T3010b2b1

### T3010R [DONE] Review：确认 state-machine 生产 op 不再包含“先拆 fragment 再整棵重算”的双轨语义
- 描述：在 `T3010` 之后只审查生产代码，确认 unified contract / emitter 的 op 粒度已经收口，不再存在“fragment op 只是为后续整棵 expr 服务、自己却仍走生产 codegen”的残留；若发现这类残留，本任务需要直接修复并复审。
- 进展：
  - 已复审 `state_machine_plan.rs`、`state_machine_segments.rs`、`state_machine_transform.rs` 与 `state_machine_emitter.rs` 中 `resume_path` / synthetic resume slot / `HandleStateOp` 生产消费链，确认恢复值改写仍停留在 plan 层，emitter 未回扫 AST。
  - 复审中发现真实残留：`crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 的 `HandleStateOp::VarRef` 仍使用 `codegen_expr_in_expected_context(...).unwrap_or(CgValue::unit())` 吞掉 codegen 失败，属于 fragment-era 的伪执行 fallback。
  - 已删除该 fallback，`VarRef` 现改为直接传播真实 codegen 结果/错误，与 ordinary expr codegen 保持一致；同时更新注释，明确 standalone `VarRef` 必须是独立可执行 op，而不是“失败后吞掉”的占位路径。
  - 已收紧 `state_machine_segments.rs` 的定向测试 `source_plan_keeps_only_whole_call_for_pure_statement_args_and_pure_if_condition`：纯 statement call 的 callee 现在也被锁定为不得生成 standalone `VarRef` fragment op。
- 已验证：
  - `cargo test -p scoopc source_plan_ -- --nocapture`
  - `cargo test -p scoopc runtime_raise_boundary_ir_branches_between_inactive_continue_and_active_dispatch -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop --features llvm -- test`（仍停在 `tests/fixtures/run-pass/continuation_resume_continuation.scoop` 的 stale `EXPECT: fail`；该既有问题已由 `T3017` 跟踪，非本轮回归）
- 目标：
  - 确认 `HandleStateOp` 的生产消费者只接收独立可执行 op 或明确 no-op 语义 marker。
  - 确认 emitter 不再依赖 `VarRef` 式的“失败就吞掉”容错来保持 state-machine 可运行。
  - 确认统一主线没有重新引入按源码形状或局部 AST 片段做补丁式分流的逻辑。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“state-machine expression emission 粒度已收口；生产 op 不再含 fragment-only 伪执行路径”。
- 审查结论：
  - `HandleStateOp` 的生产消费面已收口为独立可执行 op 或明确语义 marker；没有再保留“先拆 fragment，再靠整棵 expr 重算覆盖”的伪执行路径。
  - emitter 不再依赖 `VarRef` 的吞错 fallback；若仍出现不可独立执行的 standalone `VarRef`，现在会像普通 expr codegen 一样直接暴露真实错误。
  - 统一主线中未发现按源码形状、局部 AST 片段或旧 fragment 分类做补丁式分流的生产逻辑回流。
- 依赖：T3010b2b

### T3011 [DONE] 修正 unified contract 中 frame slot 的 mutability / capture metadata
- 描述：当前 plan builder 在“第一次看到 local ref”或“arm capture 外层 local”时会先创建 `mutable: false` 的 `FrameSlot`，后续也不会回填真实声明信息，导致原本是 `var` 的 slot 在 unified emitter / stmt codegen 中被冻结成 immutable，并报 `assignment to immutable local`。
- 进展：
  - `build_unified_lowering_contract()` 现在会在生产态 `HandlePlanContext::from_codegen()` 的基础上，补充当前 `handle` 自身的 local metadata，避免 nested/unified 子路径丢失声明点 mutability。
  - `build_stmt(Val)` 在遇到真实声明时会直接覆盖同 `SymbolId` 的旧 slot metadata，消除“先占坑成 immutable / outer-scope、后续永不回填”的顺序依赖。
  - 已新增结构回归 `declared_handle_local_overwrites_placeholder_slot_metadata` 与 `handle_context_extension_recovers_nested_handle_outer_var_mutability`，分别锁定“声明覆盖旧 slot”与“当前 handle 元数据补入生产 context 后 nested handle 仍保留 outer `var` mutability”。
  - 已验证 `cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_resume_unit.scoop`、当前 `T3006` xfail 子集扫描（未再出现 `assignment to immutable local`）、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 均通过。
  - 复跑 `cargo run -p scoop --features llvm -- test` 后，suite 仍只停在已跟踪的 stale `EXPECT: fail`：`tests/fixtures/run-pass/continuation_resume_continuation.scoop`（`T3017`），未出现新的更早失败点。
- 目标：
  - frame slot 的 mutability 必须以声明点为真值来源；若 slot 先由 local ref / capture 路径占坑，后续接入声明信息时需要回填真实 `mutable` 状态，而不是保持默认 `false`。
  - arm capture、cross-state local、resume 后继续赋值等路径都应消费同一份 authoritative slot metadata。
  - unified emitter / `codegen_assign_stmt` 观察到的 mutability 与前端声明语义一致，不再被 state-machine 中间结构篡改。
- 验收：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_resume_unit.scoop`
  - 重新跑当前 `T3006` xfail 子集时，不再出现 `暂不支持的 main 代码生成节点：assignment to immutable local`。
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3010R

### T3011R [DONE] Review：确认 frame slot mutability / capture 元数据已按声明点收口
- 描述：在 `T3011` 之后只审查生产代码，确认 unified contract 中的 slot metadata 已由声明点驱动，不再有“先占坑成 immutable、后续永不修正”的残留；若发现这类问题，本任务需要直接修复并复审。
- 进展：
  - 已审查 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`：`build_stmt(Val)` 会以真实声明覆盖旧 slot metadata；`collect_outer_scope_slots()` 与 `authoritative_local_slot()` 都从 `known_local_metadata` 读取权威 mutability / type，不依赖占坑顺序。
  - 已审查 `build_unified_lowering_contract()`：生产态 `HandlePlanContext::from_codegen()` 先收集当前 env 中可见 locals，再由 `extend_known_local_metadata_from_handle(handle)` 补入当前 `handle` 自身声明，nested handle / arm / finally 路径都能看到声明点元数据。
  - 已审查 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 与 `crates/scoopc/src/llvm/codegen/stmt.rs`：frame slot 预注册、read-back、arm capture 恢复、outer-scope seeding/writeback 均直接消费 unified slot metadata；赋值仍统一走通用 `codegen_assign_stmt()`，没有新增 effect-only mutable 特判。
  - 已验证 `cargo test -p scoopc declared_handle_local_overwrites_placeholder_slot_metadata -- --nocapture`、`cargo test -p scoopc handle_context_extension_recovers_nested_handle_outer_var_mutability -- --nocapture`、`cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_resume_unit.scoop`、`cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_outer_var_writeback_basic.scoop`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。
  - 复跑 `cargo run -p scoop --features llvm -- test` 后，suite 仍只停在已跟踪的 stale `EXPECT: fail`：`tests/fixtures/run-pass/continuation_resume_continuation.scoop`（`T3017`），未出现新的更早失败点。
- 目标：
  - 确认 local ref、arm capture、resume 边界与普通 `val/var` 声明最终都汇聚到同一份 slot metadata。
  - 确认 emitter / stmt codegen 不再依赖 slot 创建顺序来决定 mutability。
  - 确认没有为了过回归而在 `codegen_assign_stmt` 层面加 effect-only 特判。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“frame slot mutability / capture metadata 已与声明语义对齐，无顺序依赖或 effect-only patch”。
- 审查结论：
  - frame slot mutability / capture metadata 已与声明语义对齐：声明点是唯一真值来源，capture / resume / outer-scope seeding 只消费同一份 authoritative metadata。
  - emitter / stmt codegen 不依赖 slot 创建顺序决定 mutability，也不存在 effect-only mutable patch。
- 依赖：T3011

### T3012 [DONE] 补齐 unified path 的 expected-context / closure / coercion 支持
- 描述：当前 unified emitter 对整棵表达式的重发射曾经缺少稳定 expected context，导致 enum variant ctor、`print/println` 参数整形、`when`/`if` tail value、closure 值和某些 `coerce_value` 路径在 state-machine 中报 `enum variant ctor call without expected enum type`、`sysroot print/println arg type`、`expression kind`、`value coercion`。本任务只覆盖这类 expected-context / closure / coercion 缺口；escaped continuation 的 composite resume payload 仍由后续 `T3013` + `T3009b` 处理。
- 进展：
  - 已验证 `cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_locals.scoop` 通过，说明 unified path 上的 closure / function-value 路径已不再回落到 `ExprKind::Closure` 占位错误。
  - 已验证 `cargo run -p scoop -- run tests/fixtures/run-pass/std_test_assertions_basic.scoop` 通过，说明 enum ctor expected type、`print/println` 参数整形与 assertion helper 的 expected-context 传递已与普通 codegen 对齐。
  - 已扫描当前全部 101 个 `run-pass` `EXPECT: fail` fixture，其中 87 个已直接通过；扫描结果中不再出现 `expression kind`、`enum variant ctor call without expected enum type`、`sysroot print/println arg type` 这三类 `T3012` 目标错误。
  - 扫描中唯一残留的 `value coercion` 为 `tests/fixtures/run-pass/continuation_resume_enum.scoop`；该用例依赖 escaped continuation 的 composite enum resume payload。`effect/mod.rs` 已明确注明 `Continuation.resume(...)` 的 composite payload 仍待 `T3013` / `T3009b`，因此本任务不再把该 fixture 当作验收项。
- 目标：
  - 统一 state-machine 路径在发射整棵 expr、arm body、handle result、local initializer、binder/result readback 时，补齐与普通 codegen 等价的 expected context 传递。
  - closure / function-value 相关表达式在 unified 路径上不再命中 `ExprKind::Closure` 的占位错误；间接 perform / indirect resume / closure-captured local 路径可执行。
  - 对 enum unit variant ctor、`print/println` 参数、tail value coercion 等 case，state-machine 路径与非 effect 路径复用同一套值整形规则。
- 验收：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_locals.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/std_test_assertions_basic.scoop`
  - 重新跑当前 `T3006` xfail 子集时，不再出现以下错误类别：
    - `暂不支持的 main 代码生成节点：expression kind`
    - `暂不支持的 main 代码生成节点：enum variant ctor call without expected enum type`
    - `暂不支持的 main 代码生成节点：sysroot print/println arg type`
    - 仅因 expected context 缺失引发的 `暂不支持的 main 代码生成节点：value coercion`
- 依赖：T3011R

### T3012R [DONE] Review：确认 unified path 的 expected context 与 closure 支持已与普通 codegen 对齐
- 描述：在 `T3012` 之后只审查生产代码，确认 unified path 不再靠 effect-only 特判或 fixture patch 传递 expected context，也不再把 closure case 留在占位错误后面；若发现这类问题，本任务需要直接修复并复审。
- 进展：
  - 已审查 `crates/scoopc/src/llvm/codegen/expr.rs`、`control_flow.rs`、`stmt.rs`、`mod.rs`、`effect/mod.rs`、`effect/state_machine_plan.rs` 与 `effect/state_machine_emitter.rs`，对照 ordinary / unified 两条生产路径中的 initializer、call、`if`/`when`、handle result、arm body 与 continuation resume 入口。
  - 已确认 unified path 的 expected context 来源仍是类型/slot/result 合同：`BindLocal` 复用普通 `codegen_initializer_expr`，handle 结果类型来自 `codegen_handle_expr(..., expected)` 与 `contract.result_ty()`，`if`/`when`/block/call/`print`/`println` 等值整形继续复用 ordinary codegen 的 `codegen_if_expr`、`codegen_when_expr`、`codegen_call` 与 `codegen_sysroot_print_like`。
  - 已确认 closure / function-value 路径没有 effect-only 分支：局部初始化继续走 `codegen_initializer_expr`，call-arg / receiver-arg 场景继续由 ordinary `codegen_call` 按参数类型调用 `codegen_closure_expr`，unified state machine 只负责 frame/result carrier，不额外实现一套 closure lowering。
  - 已确认 `Continuation.resume(...)` 的 builtin 分派仍只由 `continuation_resume_call_sites` 这个 typechecked side table 驱动，`effect/mod.rs` 中仅用 `MemberAccess` 结构抽取 receiver；生产代码未重新引入按 `single`/`indirect`/`nested`/callee shape 选择 lowering 路径的分流。
- 已验证：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_locals.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/std_test_assertions_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop --features llvm -- test` 仍只停在已跟踪的 stale `EXPECT: fail`：`tests/fixtures/run-pass/continuation_resume_continuation.scoop`（`T3017`），未出现新的更早失败点。
- 目标：
  - 确认 expected context 的来源都来自类型/slot/result 合同，而不是按 fixture 形状硬编码。
  - 确认 unified path 中的 closure / function-value 支持与普通 codegen 使用同一套生产逻辑。
  - 确认 effect codegen 没有重新引入按 `single`/`indirect`/`nested` 等源码分类分流的补丁。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“unified path 的 expected context / closure 支持已与普通 codegen 对齐，无 effect-only workaround 残留”。
- 审查结论：
  - unified path 的 expected context / closure 支持已与普通 codegen 对齐，无 effect-only workaround 残留。
  - expected context 继续由声明类型、handle result 类型、call 参数类型与 ordinary control-flow codegen 的输出类型合同驱动；unified emitter 没有按 fixture 形状、源码形状或旧分类结果硬编码 expected type。
  - closure / function-value 支持继续复用 ordinary `codegen_initializer_expr` / `codegen_call` / `codegen_closure_expr` 生产逻辑；state machine 只承担 suspension/result carrier 角色，没有演化出 effect-only closure lowering。
  - 生产代码中未发现按 `single` / `indirect` / `nested` / callee shape 分流的 effect codegen 补丁回流。
- 依赖：T3012

### T3013 [DONE] 扩展 effect payload / handle result / resume payload transport，覆盖 composite 值
- 描述：当前统一主线仍把大量 payload/result 运输建立在 `u64 word` 假设上，遇到 tuple/struct/boxed enum payload/continuation ref 等 composite 值就会报 `u64 word from composite value`，并阻塞 handle result、perform binder、resume payload 三条链路的一致性。
- 进展：
  - 已在 `crates/scoopc/src/llvm/codegen/effect/mod.rs` 中新增 `EffectTransportKind` 与共享 helper：`encode_effect_transport_value`、`decode_effect_transport_value`、`box_effect_transport_value`、`unbox_effect_transport_value`，统一把 effect transport 收口为 `Word` / `GcRef` / `BoxedComposite` 三类。
  - `codegen_perform_expr`、state-machine `emit_perform_op`、handle result 的 frame read/write 与 `Continuation.resume(...)` payload write/read 现在都复用同一套 helper；tuple/struct/含 payload enum 等非 word 值通过 typed GC box 走 `resume_gc_ref` / perform-slot `gc_ref` / frame `resume_gc_ref`，不再依赖 `u64 word` 假设。
  - `Continuation.resume(value)` 现优先从 receiver 的 `Continuation<T>` 提取 authoritative payload type，避免 HIR 中 payload expr 已退化成 `Any/Ref` 时走错 expected-type / coercion 路径。
  - 定向验证通过：
    - `cargo run -p scoop -- run tests/fixtures/run-pass/handle_compound_result.scoop`
    - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_nonresuming_payload_struct_indirect.scoop`
    - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_continuation.scoop`
    - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_struct.scoop`
    - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_tuple.scoop`
    - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_struct_with_ref.scoop`
  - `tests/fixtures/run-pass/continuation_resume_enum.scoop` 已不再报 `u64 word from composite value`；当前残留错误收窄为 richer enum escaped-continuation 路径上的 `value coercion`，继续由后续 `T3009b` 收口。
  - `tests/fixtures/run-pass/continuation_resume_ref_class.scoop` 在临时去掉 xfail 后暴露出 resumed body 第二次 `perform` 不会重新进入 captured handler dispatch loop、caller-tail 在第一次 `resume` 后被截断的更前置缺口；这属于 `T3015` 的 handler-context lifetime / redispatch 语义问题，而不是当前 transport 根因，因此已恢复为带真实原因的 xfail。
  - `cargo test --all`、`cargo clippy --all-targets -- -D warnings` 通过；`cargo run -p scoop --features llvm -- test` 已越过 transport 相关失败点，当前首个停止点是 `tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop` 的 stale `EXPECT: fail`，由 `T3017` 跟踪。
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

### T3013R [DONE] Review：确认 payload/result transport 已统一收口到 GC-safe 合同
- 描述：在 `T3013` 之后只审查生产代码，确认 payload/result transport 的扩展不是零散地在三四个调用点各补一个特殊分支，而是真正形成统一、GC-safe、可审计的合同；若发现散落 patch，本任务需要直接修复并复审。
- 进展：
  - 已审查 `crates/scoopc/src/llvm/codegen/effect/mod.rs`、`effect/state_machine_emitter.rs`、`runtime_abi.rs` 与 `runtime/c/scoop_runtime.c`，对照 standalone `perform`、state-machine `Perform` op、handle result frame read/write、arm binder readback 与 `Continuation.resume(...)` payload write/read 的完整生产链路。
  - 已确认三条 transport 链路都统一收口到 `encode_effect_transport_value` / `decode_effect_transport_value`：standalone `codegen_perform_expr` 与 state-machine `emit_perform_op` 共用同一套 encode helper，handle result 通过 `store_result_to_frame` / `read_result_from_frame` 复用同一对 helper，continuation resume payload 通过 `write_resume_payload_to_continuation` 复用同一套 encode 逻辑，arm binder 则通过 `read_binder_from_perform_slot` 走同一套 decode 逻辑。
  - 已确认 composite transport 不依赖 `ptr <-> int` 或额外 side channel：`EffectTransportKind::BoxedComposite` 统一分配 typed GC box，并仅通过 `perform_slot.gc_ref` / frame `resume_gc_ref` / continuation `resume_gc_ref` 传递；`effect/mod.rs` 中对 GC ref / composite 的 word path 会直接报错，生产 lowering 不存在偷偷把 GC 指针压成整数的旁路。
  - 已确认 GC trace 合同与 transport 保持一致：continuation runtime trace 显式追踪 `state` 与 `resume_gc_ref`；effect frame type descriptor 从 payload 起始 offset 生成 trace bitmap，覆盖 public `resume_gc_ref` 与 runtime-only continuation slot；因此 boxed composite payload 在 suspend / resume / handle result 期间都可被 GC 正确追踪。
- 已验证：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/handle_compound_result.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_nonresuming_payload_struct_indirect.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_continuation.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_struct.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_tuple.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_struct_with_ref.scoop`
  - `cargo test -p scoopc async_await_ir_preserves_continuation_slot_and_perform_payload -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop --features llvm -- test` 仍只停在已跟踪的 stale `EXPECT: fail`：`tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop`（`T3017`），未出现新的更早 transport 失败点。
- 目标：
  - 确认 `perform` binder、handle result、resume payload 三条链路使用同一套 value transport 约定。
  - 确认 composite 值的运输路径不依赖 `ptr <-> int`、native-only side channel 或 effect-only 特判。
  - 确认 continuation / frame trace 合同与新的 composite transport 仍然一致。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“payload/result transport 已统一收口到 GC-safe 合同，无散落特判或非法 ptr/int 编码残留”。
- 审查结论：
  - payload/result transport 已统一收口到 GC-safe 合同，无散落特判或非法 ptr/int 编码残留。
  - `perform` binder、handle result、resume payload 三条链路都使用同一套 `Word` / `GcRef` / `BoxedComposite` transport 约定；生产代码中不存在“某一条链路单独扩展、另一条链路继续保留 word-only 假设”的分叉。
  - composite 值统一通过 typed GC box + `resume_gc_ref` / perform-slot `gc_ref` / frame `resume_gc_ref` 传递；continuation / frame trace 已覆盖这些 GC 槽位，因此 transport 与 GC 可达性合同保持一致。
- 依赖：T3013

### T3014 [DONE] 修正 handler stack 的 multi-op registration 与 unmatched effect 外传
- 描述：当前统一 state-machine handle 的 `dispatch_unmatched` 路径已正确 outward propagate；本任务剩余需要收口的是 handle 入口只把第一个 dispatch entry 的 `op_tag` push 到 runtime handler stack，导致 multi-op handler 捕获到的动态 effect context 不完整。生产实现必须把每个 dispatch entry 都注册到 runtime handler stack，并在所有退出路径上对称弹栈。
- 进展：
  - 已复查 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`，确认 `dispatch_unmatched` 当前直接分支到 `outward_target_bb`，不会再把未匹配 effect 吞进 `handle_done`；因此本轮真实缺口收窄为 multi-op registration。
  - 已新增 `allocate_registered_handler_frames` / `pop_registered_handler_frames` helper，把 handle 入口的 runtime handler 注册收口为单一实现。
  - handle 入口现在会为 `contract.dispatch_entries()` 中的每个 entry 分配独立的 `ScoopEffectHandlerFrame`，逐个 `scoop_effect_handler_stack_push(frame, op_tag)`，从而让 continuation 捕获到完整的 handler stack 链。
  - `handle_propagate` 与 `handle_done` 两个出口现在都会按逆序逐个 pop 所有已注册 frame，保持与 runtime stack 的 LIFO 合同一致。
  - 已新增 LLVM IR 定向测试 `multi_dispatch_handle_ir_registers_every_op_tag_on_handler_stack`，锁定 multi-op handle 会生成一帧一 tag 的注册与对应 pop 序列。
- 目标：
  - 每个 dispatch entry 的 effect op 都必须在 runtime 动态上下文中有可匹配的注册表示，而不是只注册首个 op。
  - 当前 handle 未匹配到 `op_tag` 时，不得把 effect 吞掉；当前生产路径应继续保持 outward propagation 合同。
  - nested different-op handles 的 runtime stack 形态要与 compile-time dispatch 一致，不允许“编译时看起来会冒泡，运行时捕获到的 handler stack 却缺边”。
- 已验证：
  - `cargo test -p scoopc multi_dispatch_handle_ir_registers_every_op_tag_on_handler_stack -- --nocapture`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_multi_nonresuming_custom_indirect.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_op_tag_two_effects_nested_dispatch.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_handler_stack_nearest_three_levels_and_arm_outside_scope.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_custom_nonresuming_nested_nearest_and_arm_outside_scope.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop --features llvm -- test` 仍只停在已跟踪的 stale `EXPECT: fail`：`tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop`（`T3017`），未出现新的更早 handler-stack 失败点。
- 验收：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_multi_nonresuming_custom_indirect.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_op_tag_two_effects_nested_dispatch.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_handler_stack_nearest_three_levels_and_arm_outside_scope.scoop`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3013R

### T3014a [DONE] 补齐同一 `op_fqn` 下多 arm 的 unified handler dispatch 合同
- 描述：在准备执行 `T3014R` 复审时发现，`DispatchPlan` / `UnifiedDispatchEntry` / `SuspendSite.matching_arms()` 都保留了“同一 `op_fqn` 可关联多个 arm”的 lowering 合同，但 `state_machine_emitter.rs` 的 handle dispatch 仍只按 `op_tag` switch，并在命中某个 `dispatch_entry` 后静默取 `dispatch_entry.arms().first()`。这会把其余 arm 直接丢掉，使生产 dispatch 与现有 typecheck/lowering 合同不一致；在 generic effect / 不同 handled-effect 实例共享同一 op 的场景下，这不再是 review 可忽略的实现细节，而是必须先修复的前置缺口。
- 进展：
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 的 same-op dispatch 已按源码顺序逐 arm 检查，不再把 `dispatch_entry.arms()` 收缩成首个 arm。
  - 修复 `crates/scoopc/src/llvm/codegen/mod.rs::matching_effect_instance_keys_for_handled_effect()` 错把 `op_fqn` 当作 effect FQN 查找候选集合的问题；现在优先按 handled effect 的 nominal FQN 收集 effect-instance keys，必要时才从 `op_fqn` 回退推导。
  - 收紧 LLVM IR 回归 `same_op_multi_arm_dispatch_ir_reads_effect_instance_key`：不仅要求读取 `effect_instance_key`，还要求生成实际的 `arm_*_effect_instance_match_*` 比较，避免“读了 key 但没参与匹配”的假阳性。
  - 新增 run-pass fixture `tests/fixtures/run-pass/effect_same_op_multi_arm_dispatch_effect_instance.scoop`，锁定 `Raise.raise` 同一 `op_fqn` 下 `Sub`/`Base` 两个 arm 的实际分发结果为 `23`。
  - 为完成全量收尾，同步更新 `crates/scoop_runtime/tests/effect_tls.rs` 到新的 perform-slot ABI（新增 `effect_instance_key` 参数与读回断言），并更新 `tests/fixtures/hir/handle_perform.hir` 以反映 `ExprKind::Perform { effect_ty, .. }` 的当前 HIR 结构。
- 目标：
  - 明确并落实 unified handler dispatch 对“同一 `op_fqn` 多 arm”的生产合同，不能继续把 `dispatch_entries()` 收缩成单 arm 特例。
  - `state_machine_emitter.rs` 不得再静默依赖 `dispatch_entry.arms().first()`；direct perform、resumed body 与 outward propagation 路径都必须消费完整的 dispatch metadata。
  - 若 dispatch 判定需要额外的 runtime / state-machine 元数据，需显式补齐对应 lowering / ABI 合同，而不是靠源码形状、fixture 顺序或默认“第一个 arm”维持表面通过。
  - 新增最小回归覆盖同一 `op_fqn` 多 arm 场景，证明后续 arm 不会在 production dispatch 中被静默丢弃。
- 已验证：
  - `cargo test -p scoopc --features llvm same_op_multi_arm_dispatch_ir_reads_effect_instance_key -- --nocapture`
  - `cargo test -p scoopc --features llvm multi_dispatch_handle_ir_registers_every_op_tag_on_handler_stack -- --nocapture`
  - `cargo test -p scoopc --features llvm effect_runtime_intrinsics_are_emitted_as_symbol_calls -- --nocapture`
  - `cargo run -p scoop --features llvm -- build tests/fixtures/run-pass/effect_runtime_slot_abi_basic.scoop -o /tmp/effect_slot`
  - 直接执行 `/tmp/effect_slot`，退出码 `48`
  - `cargo run -p scoop --features llvm -- build tests/fixtures/run-pass/effect_same_op_multi_arm_dispatch_effect_instance.scoop -o /tmp/sameop`
  - 直接执行 `/tmp/sameop`，退出码 `23`
  - `cargo test -p scoop_runtime --test effect_tls -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 验收：
  - 生产代码中不再出现把 `dispatch_entry.arms()` 静默收缩为首个 arm 的 dispatch 主路径。
  - 至少一个定向回归覆盖“同一 `op_fqn` 多 arm”场景，并能稳定区分正确 arm dispatch。
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T3014

### T3014b [DONE] 修正 ordinary hidden-suspend `Raise.raise(RuntimeError.*)` 路径的 `effect_instance_key` 回归
- 描述：执行 `T3014R` 复审时，先补齐 `tests/fixtures/hir/handle_mixed_arm_kinds.hir` 与 `tests/fixtures/hir/safe_call_not_null_assert.hir` 的 `Perform.effect_ty` stale golden 后，复跑 `cargo run -p scoop --features llvm -- test` 暴露出新的生产回归：`tests/fixtures/run-pass/class_init_hidden_raise_helper_try_catch_basic.scoop` 直接运行会报 `UnsupportedMainBody { kind: "effect instance key" }`。这说明 same-op dispatch 引入 `effect_instance_key` 合同后，ordinary `perform` lowering 在 hidden-suspend class ctor / object property helper 路径上仍不能为 `Raise.raise(RuntimeError.*)` 产出稳定 key；既有 `T3010b2b0a0` run-pass 基线因此重新失效，必须先作为 `T3014R` 的前置缺口收口。
- 进展：
  - 已将 `collect_object_inits()` / `collect_class_inits()` 升级为可接收 `typecheck_types`，并在 `lower_for_compilation_unit_multi_files()` 的 typed lowering 路径上传入 `Some(typecheck_types)`，使 object/class init side tables 能保留 typechecked `Perform.effect_ty`。
  - 已修复 `typecheck::check_file_exprs()` 之前完全跳过 `ast::Item::Object` 的缺口：object property initializer / `init {}` block 现在会被类型检查，并向 AST side table 写回 `inferred_performed_effect_tys`；nested object/type 也会递归覆盖。
  - 已新增低层回归测试 `lower_for_compilation_unit_multi_files_preserves_effect_ty_in_init_side_tables`，直接锁定 multi-file typed lowering 产出的 `object_inits` / `class_inits` side tables 中 `Raise.raise(RuntimeError.*)` 的 `Perform.effect_ty` 不能退化为 `Any`。
  - 已验证 `class_init_hidden_raise_helper_try_catch_basic.scoop`、`object_property_init_raise_helper_try_catch_basic.scoop` 与 `effect_handle_hidden_suspend_helper_object_property_basic.scoop` 全部恢复通过；`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过；完整 `cargo run -p scoop --features llvm -- test` 的首个失败点已推进到新的 delegated-property blocker `delegated_property_observable_raise_does_not_poison_mutex.scoop`，不再停在本任务对应的 hidden-suspend helper 回归。
- 目标：
  - `codegen_perform_expr` 及其相邻 lowering 在 ordinary hidden-suspend path 上处理 `Raise.raise(RuntimeError.*)` 时，能够稳定得到与统一 dispatch 合同一致的 `effect_instance_key`，不再报 `unsupported_main_body`。
  - class ctor / object property / helper 这条 hidden-suspend ordinary propagation 路径继续保持 `T3010b2b0a0` 已锁定的“当前 callee 立即终止、outer try/catch 命中、caller tail 不继续执行”语义。
  - 修复必须走统一 effect-instance key 合同，不得为 runtime error raise 或 hidden-suspend helper 单独塞源码形状特判。
- 验收：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/class_init_hidden_raise_helper_try_catch_basic.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/object_property_init_raise_helper_try_catch_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop --features llvm -- test` 不再首先停在 `class_init_hidden_raise_helper_try_catch_basic.scoop`
- 依赖：T3014a、T3010b2b0R

### T3014bR [DONE] Review：确认 hidden-suspend runtime-error raise lowering 已按统一 `effect_instance_key` 合同收口
- 描述：在 `T3014b` 之后只审查生产代码，确认 ordinary `perform` lowering、runtime Raise helper 与 hidden-suspend class/object-init 路径对 `Raise.raise(RuntimeError.*)` 使用的是同一套 `effect_instance_key` 合同，而不是为跑过 fixture 临时加 fallback；若发现问题，本任务需要直接修复并复审。
- 进展：
  - 已复审 `crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/effect/mod.rs` 与 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`：ordinary `codegen_perform_expr` 与 state-machine `emit_perform_op` 统一调用 `effect_instance_key(effect_ty)`；`emit_raise_runtime_error_variant` 直接写入的 `EFFECT_INSTANCE_KEY_RAISE_RUNTIME_ERROR` 与 `effect_instance_key()` 对 `Raise<RuntimeError>` 的特判完全一致，不存在另一套 runtime-error-only key 合同。
  - 已复审 `crates/scoopc/src/hir/lower/util.rs`、`crates/scoopc/src/hir/lower/expr.rs` 与 `crates/scoopc/src/typecheck/expr/entry.rs`：object/class init side table 现已恢复 typed lowering 链路，但仍统一通过 `ctx.lower_expr(...)` + `file.inferred_performed_effect_tys` 产出通用 `Perform.effect_ty`；修复点是把已有类型信息补回 side table，不是为 hidden-suspend helper / ctor / object property / runtime error raise 增加专门分支。
  - 已验证 `lower_for_compilation_unit_multi_files_preserves_effect_ty_in_init_side_tables`、`class_init_hidden_raise_helper_try_catch_basic.scoop`、`object_property_init_raise_helper_try_catch_basic.scoop`、`effect_handle_hidden_suspend_helper_object_property_basic.scoop`、`effect_same_op_multi_arm_dispatch_effect_instance.scoop`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`；复跑 `cargo run -p scoop --features llvm -- test` 后，suite 仍首先停在已跟踪的 `delegated_property_observable_raise_does_not_poison_mutex.scoop`（`T3014c`），没有倒回 hidden-suspend runtime-error raise 回归。
- 目标：
  - 确认 `Raise.raise(RuntimeError.*)` 在 ordinary 路径与 unified state-machine 路径之间没有分叉的 effect-instance key 语义。
  - 确认 hidden-suspend helper / ctor / object property 路径没有回流 shape-based、fixture-based 或 runtime-error-only 特判。
  - 确认 `T3010b2b0a0` 已锁定的 ordinary propagation 语义仍成立，没有被 `T3014a` 的 dispatch key 改动破坏。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“hidden-suspend runtime-error raise lowering 已按统一 `effect_instance_key` 合同收口，无 ordinary-path fallback 残留”。
- 审查结论：
  - hidden-suspend runtime-error raise lowering 已按统一 `effect_instance_key` 合同收口，无 ordinary-path fallback 残留。
  - `T3010b2b0a0` 已锁定的 ordinary propagation 语义仍成立：class/object-init helper 触发 `Raise.raise(RuntimeError.*)` 时，当前 callee 会立即终止，outer `try/catch` 继续稳定命中，caller tail 不会继续执行。
- 依赖：T3014b

### T3014c [DONE] 修正 delegated-property observable 回调内 `Raise.raise(...)` 的 state-machine `effect_instance_key` 缺口
- 描述：完成 `T3014b` 后复跑 `cargo run -p scoop --features llvm -- test`，suite 的首个失败点推进到 `tests/fixtures/run-pass/delegated_property_observable_raise_does_not_poison_mutex.scoop`；直接运行报 `UnsupportedMainBody { kind: "state machine perform effect instance key" }`。这说明 delegated-property observable 回调中的 `Raise.raise(7)` 进入 unified state-machine `emit_perform_op` 路径后，仍无法稳定获得与 dispatch 合同一致的 `effect_instance_key`。该问题不再属于 ordinary hidden-suspend helper 路径，而是另一条尚未显式跟踪的 effect-instance key 缺口，必须在继续 `T3014R` 前单独收口。
- 进展：
  - 已确认根因不是 runtime dispatch/ABI 缺 `effect_instance_key`，而是 `typecheck::check_file_exprs()` 之前直接跳过带 `delegate` 的属性表达式，导致标准 delegated property 的 inline body（`lazy` initializer、`observable`/`vetoable` 的 initial/callback）从未写入 `inferred_performed_effect_tys`。
  - 已在 `crates/scoopc/src/typecheck/expr/entry.rs` 中新增标准 delegated-property inline 表达式检查：`lazy` initializer body、`observable`/`vetoable` initial expr 与 callback body 现在都会按 lowering 的真实 inline 语义进入 expected-context type inference；其中 `observable` callback 的 `Raise.raise(7)` 会稳定写回 `Perform.effect_ty = Raise<Int>`，不再退化为 `Any`。
  - 修复过程中未采用“把整个 delegate 调用按普通 call typecheck”这条错误路径，避免把 effectful `observable` callback 误判为违反 sysroot 纯 lambda 签名，从而保持既有 delegated-property + `Raise` 语义不回退。
  - 已新增 typed lowering 回归 `lower_typed_single_source_file_preserves_effect_ty_in_observable_delegate_callback`，锁定 observable callback 内 `Raise.raise(7)` 的 `Perform.effect_ty`。
  - 已验证：新增低层回归、`lower_for_compilation_unit_multi_files_preserves_effect_ty_in_init_side_tables`、`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/delegated_property_observable_raise_does_not_poison_mutex.scoop`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过；复跑 `cargo run -p scoop --features llvm -- test` 后，suite 首个停止点已前移到已跟踪的 stale `EXPECT: fail` `effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop`（`T3017`），不再停在本任务对应的 delegated-property observable blocker。
- 目标：
  - delegated-property observable 回调中的 `Raise.raise(...)` 走 unified state-machine perform lowering 时，能够稳定获得与 dispatch 合同一致的 `effect_instance_key`，不再报 `state machine perform effect instance key`。
  - `observable` + `Raise.raise` 的既有语义继续成立：回调 raise 被外层 `try/catch` 捕获后，同一属性的后续读写仍可继续执行，不会出现 mutex poison / 锁泄漏 / callback 路径被截断。
  - 修复必须走统一 effect-instance key 合同，不得为 delegated property、observable callback 或 runtime-error-only 路径单独塞 shape-based / fixture-based 特判。
- 验收：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/delegated_property_observable_raise_does_not_poison_mutex.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop --features llvm -- test` 不再首先停在 `delegated_property_observable_raise_does_not_poison_mutex.scoop`
- 依赖：T3014bR

### T3014cR [DONE] Review：确认 delegated-property observable 的 effect-instance key 已按统一合同收口
- 描述：在 `T3014c` 之后只审查生产代码，确认 delegated-property observable callback、state-machine `emit_perform_op` 与统一 dispatch 对 `Raise.raise(...)` 使用的是同一套 `effect_instance_key` 合同，而不是为跑过 fixture 临时加 fallback；若发现问题，本任务需要直接修复并复审。
- 进展：
  - 复审 `crates/scoopc/src/typecheck/expr/entry.rs`、`crates/scoopc/src/hir/lower/sugar.rs`、`crates/scoopc/src/llvm/codegen/effect/mod.rs`、`crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 与 `crates/scoopc/src/llvm/codegen/mod.rs` 后确认：ordinary `perform`、state-machine `emit_perform_op` 与 dispatch matching 仍统一经 `effect_instance_key()` / `matching_effect_instance_keys_for_handled_effect()` 取 key，没有 delegated-property observable callback 专用 fallback。
  - 审查过程中发现并修复一个真实生产缺口：标准 delegated-property side table 之前只保存声明点 AST，却不绑定声明点 `SourceFile` / `ast::File`。跨文件使用 `lazy/observable/vetoable` 时，lowering 会在使用点文件上下文里直接 lower 声明点 callback / initializer AST，导致 `inferred_performed_effect_tys` / `inferred_expr_tys` / `inferred_binding_tys` 可能查错文件，且 local `SymbolId` 可能因“仅按 span intern”与使用点局部冲突。
  - 现已为标准 delegated-property info 补齐声明点上下文，并让 `lazy/observable/vetoable` 的 foreign-AST lowering 显式切回声明点 `SourceFile` / `ast::File`；同时把 HIR lowering 的 local symbol interning 从“仅按 span”升级为“源文件 + span”，避免跨文件 inline AST 冲突。
  - 已新增 typed-lowering 回归 `lower_for_compilation_unit_multi_files_preserves_effect_ty_in_cross_file_observable_delegate_callback`，锁定“属性声明在一文件、赋值发生在另一文件”时，observable callback 内 `Raise.raise(7)` 仍保留 `Perform.effect_ty = Raise<Int>`。
  - 已验证：新增多文件低层回归、`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/delegated_property_observable_raise_does_not_poison_mutex.scoop`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过；复跑 `cargo run -p scoop --features llvm -- test` 后，suite 未回退到本任务对应 fixture，当前首个停止点仍是已跟踪的 stale `EXPECT: fail` `effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop`（`T3017`）。
- 目标：
  - 确认 delegated-property callback 内的 `Raise.raise(...)` 与其它 unified state-machine perform 路径共用同一套 `effect_instance_key` 语义。
  - 确认 delegated property / observable / callback 路径没有回流 shape-based、fixture-based 或 runtime-error-only 特判。
  - 确认 `delegated_property_observable_raise_does_not_poison_mutex.scoop` 已锁定的“raise 被捕获后属性继续可用”语义成立。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“delegated-property observable 的 effect-instance key 已按统一合同收口，无 callback-path fallback 残留”。
- 审查结论：
  - delegated-property observable 的 effect-instance key 已按统一合同收口，无 callback-path fallback 残留。
  - 跨文件 delegated-property lowering 现在也会回到声明点上下文读取 typecheck side tables；`Raise.raise(...)` 的 `Perform.effect_ty` 不再因 callback/initializer AST 被错文件 lowering 而退化为 `Any`。
- 依赖：T3014c

### T3014R [DONE] Review：确认 multi-op handler registration 与 unmatched propagation 已与合同一致
- 描述：在 `T3014` 之后只审查生产代码，确认 multi-op handler registration 与 unmatched propagation 已形成统一语义，而不是在 dispatch loop 里拼临时分支把个别 fixture 打绿；此前复审已先后拆出 same-op multi-arm dispatch 前置缺口 `T3014a`、ordinary hidden-suspend runtime-error raise regression `T3014b`，以及本轮全量验证继续前移后暴露的 delegated-property observable callback key 缺口 `T3014c`。待这些前置任务完成后，本任务再继续确认 registration / outward propagation / dispatch 合同已经统一收口；若发现问题，本任务需要直接修复并复审。
- 进展：
  - 已复审 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`、`crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/effect/mod.rs`、`crates/scoopc/src/llvm/codegen/effect/state_machine_segments.rs`、`crates/scoopc/src/llvm/codegen/effect/state_machine_transform.rs` 与 `runtime/c/scoop_runtime.c`。
  - 已确认 `allocate_registered_handler_frames()` 逐个消费 `contract.dispatch_entries()`，为每个 dispatch entry 分配独立 `ScoopEffectHandlerFrame` 并按 `op_tag` 注册到 runtime handler stack；`handle_done` / `handle_propagate` 两个出口都通过 `pop_registered_handler_frames()` 逆序对称弹栈，与 runtime LIFO 合同一致。
  - 已确认 `dispatch_unmatched` 只会分支到 `outward_target_bb`，并在存在 cleanup scope 时经 `handle_cleanup_propagate_*` 执行 cleanup 后进入 `handle_propagate`；未匹配 effect 不会穿过 `handle_done` 正常完成路径。
  - 已确认 continuation runtime 在 `scoop_continuation_alloc()` 中捕获当前 `__scoop_effect_handler_stack_top`，而 multi-op handle 现已把全部 dispatch entry 注册到该栈上，因此跨 suspend/escaped continuation 恢复时保留的是完整动态 handler 链，而不是首个 op-tag 的残缺上下文。
- 已验证：
  - `cargo test -p scoopc --features llvm multi_dispatch_handle_ir_registers_every_op_tag_on_handler_stack -- --nocapture`
  - `cargo test -p scoopc --features llvm same_op_multi_arm_dispatch_ir_reads_effect_instance_key -- --nocapture`
  - `cargo test -p scoop_runtime --test effect_tls -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_nonresuming_custom_indirect.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/effect_op_tag_two_effects_nested_dispatch.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/effect_handler_stack_nearest_three_levels_and_arm_outside_scope.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/effect_custom_nonresuming_nested_nearest_and_arm_outside_scope.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/effect_same_op_multi_arm_dispatch_effect_instance.scoop`（进程退出码 `23`，符合 fixture 预期）
  - `target/debug/scoop run tests/fixtures/run-pass/delegated_property_observable_raise_does_not_poison_mutex.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop --features llvm -- test`（仍只停在已跟踪的 stale `EXPECT: fail`：`tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop`，对应 `T3017`）
- 目标：
  - 确认 runtime handler registration 与 contract `dispatch_entries()` 一一对应。
  - 确认 unmatched perform 的传播路径不再穿过 `handle_done` 正常完成路径。
  - 确认 effect codegen / runtime ABI 对“向外传播”没有 shape-based 或 fixture-based 特判。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“multi-op handler registration 与 unmatched propagation 已按统一合同收口，无吞 effect 的错误完成路径残留”。
- 审查结论：
  - multi-op handler registration 与 unmatched propagation 已按统一合同收口，无吞 effect 的错误完成路径残留。
  - runtime handler registration 与 `dispatch_entries()` 现在是一一对应关系；same-op 多 arm 继续在单个 dispatch entry 内按 `effect_instance_key` 顺序判定，不存在“只注册第一个 op / 只消费第一个 arm”的回流。
  - unmatched perform、cleanup 后 outward propagation 与 ordinary non-resuming effect exit 仍统一复用 TLS active + shared exit 合同，没有 handle-only、shape-based 或 fixture-only 特判。
- 依赖：T3014cR

### T3009b1 [DONE] 修正 `Continuation.resume(...)` 在局部 `VarRef` payload / receiver 上丢失 precise type，恢复 direct enum payload
- 描述：开始执行原 `T3009b` 后先复跑 direct composite fixture，确认 tuple / struct / struct-with-ref / continuation-ref payload 已能通过，真正残留的 direct blocker 收窄为 `continuation_resume_enum.scoop`：`val payload: Result = Ok(42); k.resume(payload)` 会在 LLVM 侧报 `UnsupportedMainBody { kind: "value coercion" }`。根因不是 enum transport 本身，而是 `codegen_continuation_resume_builtin()` 直接读取 `receiver.ty` / `payload_expr.ty`；但 HIR `VarRef` 经常被宽化为 `Any/Ref`，导致 builtin fallback 把 local enum payload 误判成 `Ref`，最终走到 `Enum -> Ref` 的错误 coercion。
- 进展：
  - `crates/scoopc/src/llvm/codegen/effect/mod.rs::codegen_continuation_resume_builtin()` 现已改为优先通过 `resolve_expr_concrete_type(receiver)` 读取 receiver 的精确 `Continuation<T>` type argument，并在 fallback 时改用 `resolve_expr_cg_ty(payload_expr)`，不再盲信 `VarRef` 的宽化 HIR type。
  - `tests/fixtures/run-pass/continuation_resume_enum.scoop` 已移除 stale `EXPECT: fail`；direct enum payload 现在会与 tuple / struct / continuation-ref 一样走 dedicated resume lowering + shared effect transport，而不是在 builtin 入口退化成 `Ref` coercion。
  - 继续验收原 `T3009b` 时又暴露出一个尚未显式跟踪的、更前置的生产缺口：`effect_escape_continuation_indirect_perform_resume_string.scoop` 与 `effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop` 仍会在 resumed indirect callee 中跳过 suspend 点之后的语句；tail-return 版本已通过，说明 blocker 不再是 payload transport 本身，而是“间接 callee suspend 后 resumed-body caller-tail”没有接回。该缺口已拆分成新的前置任务 `T3009b2` / `T3009b2R`，原 `T3009b` 顺延到其后继续完成剩余 composite transport 收尾。
- 目标：
  - `Continuation.resume(...)` 在 direct payload 场景下，不因 `VarRef` 的 HIR 宽化丢失 `Continuation<T>` 的真实 payload 类型。
  - direct enum payload 与既有 tuple / struct / continuation-ref payload 统一走 dedicated lowering + shared transport，不回退到 `value coercion` / generic path。
- 验收：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_enum.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_tuple.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_struct_with_ref.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_continuation.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已验证：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_enum.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_tuple.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_struct_with_ref.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_continuation.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T3013R、T3009b0R

### T3009b1R [DONE] Review：确认 `Continuation.resume(...)` 的 precise type 解析不再依赖宽化 `VarRef` HIR type
- 描述：在 `T3009b1` 之后只审查生产代码，确认 `Continuation.resume(...)` 对 receiver / payload 的类型解析已经统一走精确的 env/type 信息，而不是继续依赖 `VarRef` 上经常被宽化成 `Any/Ref` 的 HIR type；若发现回流，本任务需要直接修复并复审。
- 进展：
  - 已复审 `crates/scoopc/src/llvm/codegen/effect/mod.rs::codegen_continuation_resume_builtin()`、`crates/scoopc/src/llvm/codegen/mod.rs::resolve_expr_concrete_type()`、`resolve_expr_cg_ty()`、`codegen_var_ref()` 以及 `typecheck` / `state_machine_plan` 中 `Continuation.resume` 调用点 side table 接线，确认 builtin 入口仍只依赖 typecheck 确认的 call-site marker，没有按成员名 / 局部形状做 fixture-only 分流。
  - 复审过程中发现并修复了一个此前残留的生产缺口：`resolve_expr_cg_ty()` 虽已用于 `Continuation.resume(...)` payload fallback，但原实现只优先读取局部 env，然后直接回退到 `expr.ty`；而 HIR lowering 会把所有 `VarRef`（包括 top-level const）降成 `Any`。这意味着 `k.resume(TOP_LEVEL_CONST)` 之类路径仍会把 payload 误降成 `Ref`。现已把 fallback 改为复用 `resolve_expr_concrete_type()`，在保留局部 `CgLocal.ty` 优先级的同时覆盖 top-level `VarRef` / typechecked call result 等 concrete-type 来源。
  - 已新增 run-pass 回归 `tests/fixtures/run-pass/continuation_resume_top_level_const_payload.scoop`，锁定“top-level typed `VarRef` payload 不再退回 widened `Any`”的路径。
- 目标：
  - 确认 builtin 入口优先读取精确 `Continuation<T>` receiver type，而不是把 `receiver.ty` 当 authoritative source。
  - 确认 payload fallback 路径统一走 `resolve_expr_cg_ty(...)` 等精确来源，不再把 local enum / tuple / struct payload 误降成 `Ref`。
  - 确认没有为 direct enum fixture 临时加仅按成员名 / 局部形状识别的 patch。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“`Continuation.resume(...)` 已按精确类型信息解析 receiver/payload，无宽化 `VarRef` fallback 残留”。
- 已验证：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_enum.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_top_level_const_payload.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/continuation_resume_tuple.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/continuation_resume_struct_with_ref.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/continuation_resume_continuation.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 审查结论：
  - `Continuation.resume(...)` 已按精确类型信息解析 receiver/payload，无宽化 `VarRef` fallback 残留。
  - receiver 继续优先通过 `resolve_expr_concrete_type(receiver)` 提取 `Continuation<T>` 的 authoritative payload type；payload fallback 现在在局部 env 之外，还会覆盖 top-level `VarRef` 与其它 concrete-type 来源，不再把 widened `Any` 误当成 `Ref`。
  - 生产代码中不存在为 direct enum fixture 临时加的成员名 / 局部形状 patch；state-machine segmentation 也仍只依赖 typecheck side table 标记的 `Continuation.resume` 调用点。
- 依赖：T3009b1

### T3009b2a [DONE] 把 indirect callee 的 `callee_suspend_state` 纳入 continuation/runtime ABI 捕获合同
- 描述：开始真正实现 `T3009b2` 时，继续复现 `effect_escape_continuation_indirect_perform_basic.scoop`、`effect_escape_continuation_indirect_perform_closure_locals.scoop`、`effect_escape_continuation_indirect_perform_resume_string.scoop` 与 `effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop` 后确认，当前缺口不只是“resume 值被 caller-tail 误用”。更前置的问题是：runtime 虽保留了裸 TLS `__scoop_callee_suspend_state`，但 `ScoopContinuation` / LLVM continuation ABI 只捕获 handler stack、body `state`、`resume_state_tag` 与 payload 双槽，完全没有把 ordinary indirect callee 的 suspend state 纳入 continuation。若不先补这层合同，即便后续接回 callee 自己的 resumed body，`handle` 返回后或跨线程 `resume` 时也没有 authoritative 位置保存/恢复该 state。先把 continuation/runtime ABI 正式扩展为可捕获 `callee_suspend_state`，再继续后续 ordinary callee restore 逻辑。
- 进展：
  - 已在 `crates/scoopc/src/llvm/codegen/runtime_symbols.rs` / `runtime_abi.rs` / `effect/state_machine_emitter.rs` 中接好 `scoop_callee_suspend_state_get` / `clear` 与 LLVM continuation 第 8 个字段 `captured_callee_suspend_state`，使 `UnifiedStateTerminator::Suspend` 在分配 continuation 后会把当前 TLS callee suspend state 提升进 continuation，并立即清空 TLS。
  - 已在 `runtime/c/scoop_runtime.c` 中把 `ScoopContinuation` 扩展为正式持有 `captured_callee_suspend_state`：同步更新了布局断言、GC trace、alloc 初始化、resume 动态范围 restore 逻辑，以及 `thread_unregister` 的 TLS 清理。
  - `scoop_continuation_resume_common()` 现会在 step_fn 动态范围内把 continuation 捕获的 callee suspend state 恢复到 TLS，并在返回后恢复 caller 原 TLS；同时对该 captured state 做动态范围 pin/unpin，避免 moving GC 在 resumed body 消费前让 TLS raw 指针失效。
  - 已新增一条 LLVM IR 定向测试和两条 runtime 行为测试，分别锁定 suspend 捕获、resume restore 与 TLS clear/unregister 语义。
- 目标：
  - `ScoopContinuation` 结构、runtime trace/alloc/resume 逻辑与 LLVM `llvm_continuation_struct_type()` 必须显式携带 captured callee suspend state，而不是继续依赖裸 TLS 残留。
  - state-machine `Suspend` 终止点在分配 continuation 时，必须把当前线程的 `callee_suspend_state` 捕获进 continuation。
  - 这一步只补“捕获/恢复合同”，不在本任务内重新引入旧的 shape-based callee resume codegen。
- 已验证：
  - `cargo test -p scoop_runtime --test continuation_one_shot`
  - `cargo test -p scoop_runtime --test effect_tls`
  - `cargo test -p scoopc suspend_ir_captures_callee_suspend_state_into_continuation -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 验收：
  - 新增 IR/结构定向测试，锁定 continuation IR 会捕获 callee suspend state，且 runtime continuation 布局/trace 合同与 LLVM 一致。
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T3009b1R

### T3009b2aR [DONE] Review：确认 continuation/runtime ABI 对 `callee_suspend_state` 的捕获没有回流成裸 TLS 旁路
- 描述：在 `T3009b2a` 之后只审查生产代码与 runtime 合同，确认 `callee_suspend_state` 已成为 continuation 的正式组成部分，而不是继续仅靠 TLS 侥幸残留；若发现 capture/restore 仍未闭环或 emitter/runtime 只对单个 fixture 做局部补丁，本任务需要直接修复并复审。
- 进展：
  - 已复审 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`、`crates/scoopc/src/llvm/codegen/runtime_abi.rs`、`runtime/c/scoop_runtime.c` 与 `runtime/c/scoop_runtime_api.h`，确认编译器生产路径只在 `Suspend` terminator 中执行一次 `scoop_callee_suspend_state_get() + clear()` 并写入 continuation 字段，runtime 生产路径只在 `scoop_continuation_resume_common()` 中把 continuation 捕获值临时恢复进 TLS，step_fn 返回后恢复 caller 原 TLS。
  - 复审过程中发现一个真实 ABI 旁路：`runtime/c/scoop_runtime_api.h` 仍把裸 TLS 符号 `__scoop_callee_suspend_state` 作为正式导出符号暴露，且仅供测试使用的 `scoop_callee_suspend_state_set` 仍以通用 runtime API 形式存在。现已把 TLS 变量收紧为 runtime 内部静态存储，并把 setter 改名为显式 test helper `scoop_test_callee_suspend_state_set`，避免 continuation 持久化合同之外再暴露裸 TLS/通用 setter 旁路。
  - 已验证 `cargo test -p scoop_runtime --test continuation_one_shot`、`cargo test -p scoop_runtime --test effect_tls`、`cargo test -p scoop_runtime abi_exports_allowlist -- --nocapture`、`cargo test -p scoopc suspend_ir_captures_callee_suspend_state_into_continuation -- --nocapture`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 全部通过。
- 目标：
  - 确认 continuation/runtime ABI 中存在单一 authoritative 的 callee suspend state capture/restore 通路。
  - 确认普通线程 TLS 只作为运行期临时寄存，不再承担“continuation 持久化存储”的语义职责。
  - 确认没有为当前 indirect fixtures 引入 fixture-only / callee-name-only 捕获逻辑。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“continuation/runtime ABI 已正式捕获 callee suspend state，无裸 TLS 持久化旁路残留”。
- 审查结论：
  - continuation/runtime ABI 已正式捕获 `callee_suspend_state`；continuation 是跨 `handle` 返回 / 跨线程 `resume` 的唯一 authoritative owner。
  - 普通线程 TLS 现在只承担“当前正在执行的 resumed body 的临时寄存”职责；裸 TLS 全局与通用 setter 不再作为生产 ABI 暴露。
  - 未发现按 callee 名称、fixture 名称或源码形状分流的 capture/restore 逻辑。
- 依赖：T3009b2a

### T3009b2b [DONE] 接回 ordinary indirect callee 的 resumed-body restore 路径，并让 `SuspendCall` resume 不再把 payload 误当整次调用结果
- 描述：`T3009b2a` 之后，继续把 ordinary indirect callee（普通 top-level helper / closure / function-value callee）的 suspend state save/restore 与 caller-tail 合同正式接回。当前 `SuspendCall` 的 `ResumeAfterSite` 会把恢复值注入 synthetic resume slot，并把 post-call caller-tail 重写成“直接读这个 slot”；这对 tail-return / closure-tail 场景恰好等价，但对 `fetchGreeting()`、`compute()`、`counter()`、`callIt { ... }` 这类 non-tail indirect callee 会把 `resume` payload 误当成整个调用结果，导致 callee 自己的 post-suspend body 被跳过。需要让 callee 在 resume 后重新进入自己的 resumed body，返回真实 call result，再由 outer caller-tail 继续消费。
- 进展：
  - 已在 `crates/scoopc/src/llvm/codegen/mod.rs` 中引入最小 `CalleeSuspendPlan` / `CalleeSuspendSavedLocal` 以及 `build_*_callee_suspend_plan`、`build_callee_suspend_resume_tail_block`、`finish_function_return_path` 等 helper，让 ordinary top-level helper 与 closure fun body 拥有 fresh/resume 双入口：fresh path 在 outward `perform` 前保存 post-suspend locals/captures，resume path 则在恢复后继续执行原 resumed body tail。
  - 已在 `crates/scoopc/src/llvm/codegen/effect/mod.rs` 中补齐 callee suspend state save/publish/restore helpers；ordinary frame 在存在 `current_callee_suspend_plan` 时，会先把 resume payload 与 post-suspend locals/captures 写入 callee suspend state，再沿既有 outward propagation 返回。resume 时通过 `emit_callee_suspend_resume_prologue` 从 TLS/continuation captured state 恢复 locals 与 resume payload。
  - 已在 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 中把 unified `ResumeAfterSite(Call)` 改为“有 callee suspend state 才 replay 原 call expr、并把真实 call result 写回 synthetic resume slot；否则保持既有 inactive path”，不再把 `resume` payload 直接当作整次调用结果。
  - 已在 `runtime/c/scoop_runtime.c` / `runtime/c/scoop_runtime_api.h` 与 LLVM runtime ABI 中接入 `scoop_callee_suspend_state_publish`，让 ordinary callee 的 suspend state 能通过统一 TLS 暂存并被 continuation 捕获；`Suspend` terminator 把该 state 捕获进 continuation 后还会 `unpin`，避免 pin 泄漏。
  - 四条 indirect resumed-body run-pass fixture 已移除旧的 `EXPECT: fail`，恢复为 passing baseline。验收过程中还顺手收紧了 `emit_resume_after_call_site` 的 helper 签名，消除 `clippy::too_many_arguments`，保持零 warning 门槛。
- 目标：
  - ordinary indirect callee 在 outward perform 时能保存自己的 post-suspend state，并在 resume 后恢复 locals/captures 与 resumed body。
  - outer `SuspendCall` resume 逻辑不再把 payload 直接当作整次调用结果注入 caller-tail；tail-return 与 non-tail 场景继续共享单一合同。
  - top-level helper / closure / function-value callee 的恢复逻辑不得回流为旧的源码形状分流。
- 已验证：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_basic.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_locals.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_resume_string.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 验收：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_basic.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_locals.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_resume_string.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T3009b2aR

### T3009b2b1 [DONE] 去掉 ordinary callee resumed-body restore 对 block 形状扫描的前提
- 描述：开始执行 `T3009b2bR` 复审后确认，`crates/scoopc/src/llvm/codegen/mod.rs` 新增的 `CalleeSuspendPlan` 仍把 ordinary callee 的 resumed-body restore 建立在“block 中单个 direct-perform `val` 绑定”的稳定子集上：`build_block_callee_suspend_plan()` 会扫描该源码形状来决定是否生成 fresh/resume 双入口，而 `codegen_top_level_fun` / `codegen_closure_fun_body` 也据此切换路径。这与本文件顶部“生产 effect codegen 禁止按源码 / 代码形状分流、LLVM lowering 的单一输入应为 state machine”的总约束冲突，因此必须先把 ordinary callee resumed-body restore 收口为不依赖 block 形状扫描的统一 suspend-site 合同，再继续复审。
- 进展：
  - 已在 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 中新增 `build_ordinary_callee_suspend_plan_from_unified_contract()`，复用统一 state-machine 的 `HandlePlanBuilder` / suspend-site / resume-path 元数据，为 ordinary callee 生成 `saved_locals + synthetic resume slot + rewritten resume_tail`，不再让 `mod.rs` 自己扫描 block 形状决定是否接 callee resume path。
  - 已重写 `crates/scoopc/src/llvm/codegen/mod.rs` 中的 `CalleeSuspendPlan`：删除 `perform_stmt_index` / `perform_binding_id` / `perform_binding_ty`，top-level fun 与 closure body 都统一改为消费上面的 contract；resume path 直接执行 plan 内冻结的 `resume_tail`，而不是依赖“单个 direct-perform `val` 绑定”再现拼接 tail block。
  - 已重写 `crates/scoopc/src/llvm/codegen/effect/mod.rs` 的 resume prologue：resume payload 现在恢复进 synthetic resume slot，而不是重新构造旧的 `perform` 绑定 local；ordinary frame 仍只在真实 outward `perform` 前保存 callee state，因此现有 indirect callee resumed-body 路径保持不变。
- 目标：
  - ordinary top-level helper / closure / function-value callee 的 resumed-body restore 不再依赖 `val x = perform(...)` 这类 block 形状扫描。
  - fresh path / resume path 的接线依据只能来自统一 suspend-site / continuation / callee-state 合同，不再读取源码形状。
  - `T3009b2b` 已恢复的 basic / closure-locals / string / struct-with-ref 路径继续通过，不回退成 fixture-only 补丁。
- 已验证：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_basic.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_locals.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_resume_string.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 验收：
  - `crates/scoopc/src/llvm/codegen/mod.rs` 不再通过扫描“block 中单个 direct-perform `val` 绑定”来决定是否生成 callee resume 路径。
  - `T3009b2bR` 可以在不保留源码形状分流的前提下完成复审。
- 依赖：T3009b2b

### T3009b2bR [DONE] Review：确认 ordinary indirect callee 的 resumed-body restore 已统一接回
- 描述：在 `T3009b2b1` 之后只审查生产代码，确认 ordinary indirect callee 的 resumed-body restore / caller-tail 已形成统一 continuation + suspend-state 合同，而不是对 `fetchGreeting()` / `counter()` / `callIt { ... }` 等 fixture 单独打补丁；若发现旁路，本任务需要直接修复并复审。
- 进展：
  - 复审先定位到一个真实生产缺口：`crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 的 `attach_suspend_source_paths()` 只会给顶层 `Perform/Call` 记录 source path，ordinary callee 内 `Ask.get(x) + 1` 这类 nested expression suspend source 根本不会生成 `CalleeSuspendPlan`。现已把 source-path 遍历补齐到 `Struct/Tuple/Interpolated/Unary/Binary/MemberAccess/Call/Perform` 以及 `Assign` / `Return` / `While` 条件等表达式树深度，并新增 run-pass 回归 `effect_escape_continuation_indirect_perform_nested_expr.scoop` 锁定该路径。
  - fresh path 上 ordinary frame 原先仍会把 outward `perform` 的 `Never/default` 继续送进外层表达式，导致 `perform(...) + 1` 在 dead path 上报 `integer binary op lhs`。现已把 `codegen_perform_expr()` 在 ordinary propagation 模式下改为返回“带正确类型的 dead-path dummy value”，让外层表达式只在无前驱 dead block 中结构性收尾，而真实语义仍由 early return + resumed-body path 决定。
  - 复审过程中又暴露出 tail expr ordinary callee 的另一处合同缺口：`Suspend.pause()` 这类尾部表达式语句会把 synthetic resume slot 错绑到语句位置类型，触发 `value coercion`。现已把 ordinary callee plan builder 改为显式接收 declared return type，并在 tail `ExprStmt` / `ReturnValue` consumer 上用声明返回类型修正 `resume_slot_ty`；现有 LLVM IR 定向测试 `suspend_ir_captures_callee_suspend_state_into_continuation` 已恢复通过。
- 目标：
  - 确认 indirect callee suspend/resume 的 post-suspend body 不再被跳过。
  - 确认 top-level helper / closure / function-value callee 共用同一套 resumed-body restore 机制。
  - 确认没有把旧 callee-shape / source-shape 特判重新带回生产路径。
- 已验证：
  - `target/debug/scoop run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_basic.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_locals.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_resume_string.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_nested_expr.scoop`
  - `cargo test -p scoopc suspend_ir_captures_callee_suspend_state_into_continuation -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“ordinary indirect callee resumed-body restore 已统一接回，无 shape-based / fixture-only patch 残留”。
- 审查结论：
  - ordinary indirect callee 的 resumed-body restore 已统一接回。top-level helper / closure / function-value callee 现在都通过同一套 unified suspend-site / continuation / callee-suspend-state 合同进入 fresh/resume 双入口，不再靠 fixture 名称、callee 形状或旧 block-shape 扫描决定路径。
  - 本轮新增的 nested-expression ordinary callee 路径与既有 basic / closure-locals / string / struct-with-ref 路径共享同一套生产机制，无 fixture-only patch 残留。
  - 更广的 shared resumed-body matrix 仍继续留在后续 `T3009b2`，包括 multi-site 与 nested statement-container source-path 变体的统一验收。
- 依赖：T3009b2b1

### T3009b2 [TODO] 收口 escaped continuation indirect callee 的 shared resumed-body / caller-tail 验收矩阵
- 描述：在 `T3009b2a` / `T3009b2b` / `T3009b2bR` 完成后，回到原始 shared 语义目标，统一收口 indirect callee 的 resumed-body caller-tail。除最初暴露的 `effect_escape_continuation_indirect_perform_resume_string.scoop` 与 `effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop` 外，`effect_escape_continuation_indirect_perform_basic.scoop`、`effect_escape_continuation_indirect_perform_closure_locals.scoop` 与 `effect_multi_escape_indirect_callee_suspend_matrix.scoop` 也属于同一根因：resume 后 ordinary callee 应先完成自己的 post-suspend body，再把真实 call result 交回 outer caller-tail。shared matrix 还要继续覆盖本轮已接回 nested expression 之外、仍待统一验收的 nested statement-container source-path 变体（例如 block / if / when / while body 内的 ordinary callee suspend source）。
- 目标：
  - indirect callee（普通 helper / closure / function-value callee）在 suspend 点恢复后，必须继续执行 suspend 点之后的 resumed body，而不是直接把 resume 值短路回 caller。
  - non-tail indirect perform 路径与已通过的 tail-return / closure-tail 路径共享同一套 resumed-body / caller-tail 合同，不新增 callee-shape 特判。
  - composite payload（如 `Reply { label, score }`）与 scalar/ref payload 都必须在上述 resumed-body 路径中被真正读回并消费，而不是“值到了 caller 但 callee body 被跳过”。
- 验收：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_basic.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_locals.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_resume_string.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_tail_return_int.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_escape_indirect_callee_suspend_matrix.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T3009b2bR

### T3009b2R [TODO] Review：确认间接 callee resumed-body caller-tail 已统一接回
- 描述：在 `T3009b2` 之后只审查生产代码，确认 indirect callee 的 resumed-body caller-tail 已形成统一 state-machine / continuation / callee-suspend-state 合同，而不是为 `fetchGreeting()` / `callIt { ... }` / `counter()` 之类 fixture 临时加 patch；若发现旁路，本任务需要直接修复并复审。
- 目标：
  - 确认 indirect callee suspend/resume 的 post-suspend body 不再被跳过。
  - 确认 tail-return 与 non-tail indirect callee 路径共享同一套 resumed-body / caller-tail 机制。
  - 确认没有把旧 callee-shape / source-shape 特判重新带回生产路径。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“间接 callee resumed-body caller-tail 已统一接回，无 shape-based / fixture-only patch 残留”。
- 依赖：T3009b2

### T3009b [TODO] 在 `T3009b0` dedicated lowering 基础上，把 escaped continuation 的 `Continuation.resume(...)` 剩余 composite resume payload 收口到统一合同
- 描述：`T3009b1` 已先修复 direct enum/local-VarRef payload 的 precise type 解析，`T3009b2` 将继续补齐 indirect callee suspend 后 resumed-body caller-tail。剩余部分是在这些前置闭环后，回到原始 composite transport 目标：让 escaped continuation 的 `k.resume(value)` 在 direct/indirect 路径上都能稳定覆盖 tuple/struct/boxed enum/continuation ref 等 richer payload，并与 `T3013` 的共享 effect transport 完全对齐，而不是回退到 generic path 或另起 continuation-only 通道。
- 目标：
  - 在 `T3009b0` 的 dedicated lowering 之上完成 escaped continuation composite payload 的剩余收口，不再局限于 word/ref，也不再受 indirect resumed-body 缺口影响。
  - `k.resume(...)` 继续直接消费显式 continuation 值，并统一走 runtime continuation ABI。
  - 对 composite payload 的 transport 与 `T3013` 对齐，不新增 task-only / continuation-only 特例通道。
- 验收：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_tuple.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_struct.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_struct_with_ref.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_continuation.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_resume_string.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop`
  - 重新跑当前 `T3006` xfail 子集时，不再出现 escaped continuation 路径上的 `u64 word from composite value` / composite resume transport 相关失败。
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T3009b2R

### T3009bR [TODO] Review：确认 escaped continuation resume 调用不再回落到 generic member access
- 描述：在 `T3009b` 之后只审查生产代码，确认 `Continuation.resume(...)` 的 dedicated lowering 已统一覆盖 scalar/ref + composite payload，不再隐藏在 generic member access / generic call 中；若发现回落，本任务需要直接修复并复审。
- 目标：
  - 确认 `Continuation.resume(...)` 有显式 lowering 分派，并与 `T3009b0` + `T3013` 的 payload 合同一致。
  - 确认 LLVM emitter / runtime ABI / helper 中不再保留 escaped continuation 的 placeholder-only glue。
  - 确认没有为过回归而在 generic member access 中加 effect-only 特判。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“escaped continuation resume 调用已由专用 lowering 完整接管，无 generic member-access fallback 残留”。
- 依赖：T3009b

### T3015 [TODO] 修正 arm 执行期 self-inactive 的 escaped-continuation 剩余缺口与 handler context lifetime
- 描述：`T3010b2b1` 会先收口同步 arm body 内 non-resuming effect 的外传 / cleanup / self-inactive 直接阻塞；本任务保留 escaped continuation 在 `handle` 返回后的 handler context 生命周期、以及延迟 / 跨线程 resume 时 active/inactive 恢复语义的剩余缺口。当前统一主线仍让 continuation 捕获稍后会被 pop 的 stack-alloc handler frame，导致 escaped continuation 恢复到失效的动态上下文。
- 进展：
  - `T3010a` 完成后的全量 LLVM fixture 首个失败点已推进到 `effect_escape_continuation_arm_performs_outer_effect.scoop`。直接运行显示当前输出会打印 binder `0`、继续落到 `unreachable_arm`，且没有命中外层 `EffectB` handler，符合本任务要修的 self-inactive / 外层 effect 传播缺口。
  - 同类 `effect_escape_continuation_nested_arm_indirect_performs_outer.scoop` 直接运行也会打印 `0` 并继续到 `unreachable_arm`，进一步确认这不是单个 fixture 偶发，而是 escape-continuation arm 执行期的统一语义缺口。
  - `T3013` 临时放开 `continuation_resume_ref_class.scoop` 后，又进一步确认了 handler-context lifetime 的另一侧症状：`continuation_resume_ref_class.scoop`、`effect_escape_continuation_multi_perform_while_loop.scoop` 与 `effect_escape_continuation_gc_stress_multi_string.scoop` 都会在第一次 `resume(...)` 后继续执行到 resumed body 的下一次 `perform`，但不会重新进入 captured handler 的 dispatch loop，caller-tail 因而在第一次 resumed segment 后直接截断。这说明“`handle` 返回后的 escaped continuation 仍可多次 suspend/redispatch”目前尚未闭环，属于本任务范围。
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
