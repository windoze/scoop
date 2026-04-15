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

### T3003b [TODO] 暴露 `handle -> unified lowering contract` 的生产 builder 与 crate 内访问面
- 描述：在 payload 完整后，把 production 侧构建入口与读取接口显式化。builder 只允许从 `handle` 与必需的 codegen 上下文构造 unified lowering contract；下游 emitter 只消费 state machine 与必需的类型 / 符号 / ABI 上下文。
- 目标：
  - 提供生产可调用的统一 builder，把 `handle` 收口为单一的 unified lowering contract。
  - 明确 crate 内 contract 读取面：state table、state edge、suspend edge、cleanup、frame layout、dispatch、capture / payload / slot 元数据都只能从 contract 读取。
  - 这层 API / contract 不再暴露对 HIR shape、scanner 结果、旧分类器或 flag-based unwind 机制的依赖。
- 验收：
  - 统一 LLVM lowering 入口能够只接受 state machine 及必需的类型 / 符号 / ABI 上下文。
  - 下游 emitter 所需的结构化输入都可从 unified lowering contract 读取，无需旁路回看原始 `handle` HIR。
- 依赖：T3003a

### T3003R [TODO] Review：确认 LLVM lowering 输入面只剩 state machine
- 描述：审查统一 lowering 入口与其依赖链，确认没有旁路输入；若发现旁路输入或旧依赖链残留，本任务需要直接修复并在修复后重新审查。
- 目标：
  - 确认 LLVM lowering 主线不再读取源码路径、旧 scanner 输出、旧 mode 选择结果。
  - 确认所有 effect lowering 所需结构信息都来自 state machine 合同。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“LLVM lowering 的主输入只有 state machine”。
- 依赖：T3003b

### T3004 [TODO] 实现 heap-allocated full state machine 的 LLVM lowering 主体
- 描述：基于统一合同实现真正的 LLVM lowering，当前阶段只做 full machine，不做任何 simplification。effect 传播（perform suspend、handler resume、raise/unwind）全部由 state machine 驱动，不使用 flag-based unwind。
- 目标：
  - 从 state machine 发射 frame、`pc` dispatch、state block、suspend/resume、cleanup/unwind、handler stack 交互与 payload transport。
  - 统一支持当前 state machine 能表达的合法边，不再按源码形状拆专用 emitter，不使用 `emit_effect_unwind_if_active` 作为传播机制。
  - 默认使用 heap-allocated full state machine，保持语义完整优先。
- 验收：
  - LLVM emitter 能从 state machine 构建完整控制流与运行时交互骨架。
  - effect 传播完全由 state machine 状态转移驱动，不依赖 flag-based unwind。
- 依赖：T3003R

### T3004R [TODO] Review：确认 full-state-machine LLVM emitter 不含 shape-based 选路
- 描述：在接主入口之前，先检查 emitter 主体本身是否纯粹消费 state machine；若发现 shape-based 选路或等价旁路，本任务需要直接修复并在修复后重新审查。
- 目标：
  - 确认 emitter 不根据源码结构、旧 site 分类或 arm 形状切换不同发射流程。
  - 确认 emitter 内部所有分支都来自 state machine 语义边，而非代码形状推断。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“full-state-machine LLVM emitter 只按 state machine 语义发射”。
- 依赖：T3004

### T3005 [TODO] 将统一 state-machine LLVM lowering 接回 effect codegen 主入口
- 描述：把统一 emitter 接到当前生产入口，替换 `codegen_perform_expr` / `codegen_handle_expr` 的占位错误。同时移除 `mod.rs` 中 flag-based unwind 调用点（`emit_effect_unwind_if_active` × 7、`raise_target_stack`、`fun_ty_effects_is_pure` 门控），使 effect 传播完全由统一 state machine 接管。
- 目标：
  - `handle` / `perform` 入口统一走新的 state-machine lowering，替换 `UnsupportedMainBody` 占位。
  - 移除 `mod.rs` 中 7 处 `emit_effect_unwind_if_active` 调用与配套的 `fun_ty_effects_is_pure` 门控、`raise_target_stack` 栈。
  - 生产路径中不再存在”先判源码形状再选路”或”先用 flag 检查再 unwind”的结构。
- 验收：
  - effect codegen 主入口只走统一 state-machine LLVM lowering。
  - `mod.rs` 中不再有 flag-based unwind 调用点。
- 依赖：T3004R

### T3005R [TODO] Review：确认 effect codegen 主入口只接统一 state-machine lowering，无 flag-based unwind 残留
- 描述：主入口接通后，再做一轮生产代码审查，防止旧入口残留成隐式 fallback、flag-based unwind 残留为隐式传播路径；若发现双轨或隐藏回退，本任务需要直接修复并在修复后重新审查。
- 目标：
  - 确认统一 lowering 已成为唯一主路径。
  - 确认不存在”失败时退回旧 shape-based emitter”的隐藏逻辑。
  - 确认 `emit_effect_unwind_if_active`、`raise_target_stack` 不再出现在生产调用路径中。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录”effect codegen 主入口没有旧 fallback / 双轨 / flag-based unwind 残留”。
- 依赖：T3005

### T3006 [TODO] 用定向测试补齐统一 LLVM lowering 覆盖，并修复暴露出的合同缺口
- 描述：基于当前统一主线补齐测试。若暴露出 plan builder / state machine / lowering 合同缺口，先修实现，再继续扩大覆盖。
- 目标：
  - 为 state-machine-driven LLVM lowering 建立定向单测与 representative fixture。
  - 覆盖完整状态机语义：dispatch、resume、cleanup、nested handle、indirect call-site suspension、payload transport 等。
  - 发现不支持的合法 state machine 时，优先补合同或 emitter，而不是新增 shape-based 快捷分支。
- 验收：
  - 定向 LLVM codegen 测试与代表性 fixture 能稳定覆盖当前统一主线。
  - 暴露出的缺口已经在统一合同 / emitter 内修复，而不是回退到 shape-based 方案。
- 依赖：T3005R

### T3006R [TODO] Review：确认测试补齐后生产代码仍然零 shape-based logic
- 描述：在阶段性收口前做一次完整审查，确认“为过测试临时加的例外”没有污染主线；若发现这类例外已进入生产代码，本任务需要直接修复并在修复后重新审查。
- 目标：
  - 确认新增测试修复没有把 shape-based 逻辑重新带回生产代码。
  - 确认 effect LLVM codegen 仍然满足“只从 state machine 出发”的总约束。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录“当前 effect LLVM codegen 生产代码中不存在 shape-based logic”。
- 依赖：T3006

### T3007 [TODO] 删除统一主线接管后剩余的 legacy effect codegen 死代码
- 描述：在统一主线可工作后，继续删除剩余 legacy effect codegen 文件、helper 与不再可达的过渡分支，避免仓库再次回到旧路线。包括 T3005 移除 flag-based unwind 生产调用后残留的 `emit_effect_unwind_if_active`、`emit_effect_is_active_i1`、`fun_ty_effects_is_pure` 函数定义、`raise_target_stack` 字段及其配套 runtime ABI 声明（如已无其他消费者）。
- 目标：
  - 删除不再被生产路径引用的 legacy effect codegen 文件与 helper。
  - 删除 flag-based unwind 相关函数定义与 runtime ABI 声明（若 sysroot intrinsic 仍需 `is_active` / `set_active` / `clear` 等少量 ABI，保留对应声明，仅删除纯服务于 flag-based unwind 的部分）。
  - 清理旧文档 / 注释 / 命名中仍把旧 shape-based 方案或 flag-based unwind 当作现行主线的残留。
  - 保持 effect codegen 仓库形态与当前统一架构一致。
- 验收：
  - 生产代码目录中只保留当前统一主线所需的 effect codegen 实现。
  - 已删除的 legacy 路径与 flag-based unwind 机制不再可能被重新接回主入口。
- 依赖：T3006R

### T3007R [TODO] Review：确认仓库中的 effect codegen 生产实现只剩统一主线
- 描述：最终审查，确认 cleanup 真正完成，而不是”旧代码还在，只是暂时不用”；若发现 legacy 残留（shape-based 逻辑或 flag-based unwind 机制）仍可回流生产路径，本任务需要直接修复并在修复后重新审查。
- 目标：
  - 确认 effect codegen 生产实现只剩统一主线。
  - 确认 flag-based unwind 相关代码已完全清除或仅保留 sysroot intrinsic 消费的最小 ABI 子集。
  - 确认后续 LLVM 阶段可以直接在统一主线上继续推进，不再受旧 shape-based 代码或 flag-based unwind 干扰。
- 验收：
  - 若审查发现问题，相关生产代码已在本任务内修复，并已完成修复后的复审。
  - 审查结论明确记录”effect codegen 生产实现只剩统一主线，无 shape-based legacy 或 flag-based unwind 残留”。
- 依赖：T3007

> 以下四个主题从 `TODO-3.md` 顺延迁入，按原顺序重编号为 `T31`～`T34`，仅对与当前 `T30` 主线直接相关的依赖与表述做最小更新。

## T31：前端 `do` block / closure 消歧与 block 语义收口

### T3101 [TODO] Parser / AST：引入显式 `do { ... }` block，并将裸 `{}` 固定为 closure
- 描述：当前 parser 在普通表达式位置把 `{ ... }` 同时暴露给 local block、lambda 与 trailing lambda 相关路径，导致 `val a = { println("hello") }` 之类写法无法从语法上判断是“把 closure 赋给 `a`”还是“先执行 block 再把结果赋给 `a`”。现行实现里部分 effect fixtures 只能借 `@Safe { ... }` 绕过该缺口，但这不是规范想要的语义。Scoop 改采 Swift 风格：普通局部 block 必须由 `do` 引入。
- 目标：
  - statement-position / expression-position 的普通局部 block 统一写作 `do { ... }`；parser 不再把裸 `{ ... }` 解析为 plain block。
  - 没有 `do` 的 `{ ... }` 统一按 closure 解析；`callee { ... }`、`callee(args) { ... }`、multiple trailing lambdas 继续按调用后缀 / closure 规则工作。
  - parser / AST 为 `DoBlock` 与 `Closure` 保留稳定且可区分的形状，避免后续阶段继续依赖 `@Safe` 或上下文猜测。
- 验收：
  - 新增 parser fixtures：`val f = { 1 }` 为 closure、`val x = do { 1 }` 为 block、`foo { 1 }` 仍为 trailing lambda、`foo(do { 1 })` 为普通实参、缺少 `do` 时的稳定诊断或按 closure 分流。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T3102 [TODO] Typecheck / HIR：收口 `do` block 的 expression statement 与 tail value 语义
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
