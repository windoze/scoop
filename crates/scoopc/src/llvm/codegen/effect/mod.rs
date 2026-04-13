//! effect/continuation codegen（T0102e：从 `codegen/mod.rs` 拆分）。
//!
//! 当前 effect lowering 只允许以 unified state-machine plan / segment /
//! simplification 为主线。`T2003r3d2` 已删除旧的 shape-based resuming route 与
//! 配套 helper 壳层；后续 resuming leaf 只允许围绕 unified metadata 重新接回。

use super::*;

/// flag-based unwinding（non-resuming effect）的"捕获边界"记录。
///
/// 说明：
/// - 当前阶段 `Raise.raise` 仍有独立的 `raise_target_stack`（历史原因，T0614）；
/// - T0625 起，为最小自定义 non-resuming effect 增加同样的"最近匹配"捕获边界栈，
///   用于在一个函数内把 `perform` 直接分发到最近的 `handle` catch block。
#[derive(Debug, Clone)]
pub(super) struct EffectUnwindTarget<'ctx> {
    op_fqn: String,
    target: inkwell::basic_block::BasicBlock<'ctx>,
}

/// `-> resume` lowering（T0616）在 codegen 阶段使用的"立即恢复"上下文。
///
/// 说明：
/// - 当前实现仍是统一状态机 pass 落地前的过渡路径；
/// - 现有字段围绕"单个 distinguished immediate site"组织，后续应由统一 plan/frame layout 取代；
/// - `resume(value)` 会写入 `resume_value_ptr`、更新 `state_ptr`，并跳回 `dispatch_bb`。
#[derive(Debug, Clone, Copy)]
pub(super) struct ImmediateResumeCtx<'ctx> {
    pub(super) resume_symbol: hir::SymbolId,
    _marker: std::marker::PhantomData<&'ctx ()>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
struct EscapeCaptureMeta {
    id: hir::SymbolId,
    hir_ty: Option<TypeId>,
    ty: CgTy,
    mutable: bool,
}

/// effect / continuation 共享的双通道 payload 载体。
///
/// 说明：
/// - `word` 承载标量位模式（Int/Bool/Float/Unit）；
/// - `gc_ref` 承载 GC 引用或 boxed aggregate payload；
/// - non-resuming perform slot 与 `Continuation.resume` 都复用这套形状，
///   避免继续维护各自独立的 `Int` 特判。
#[derive(Debug, Clone, Copy)]
struct AbiPayloadTransport<'ctx> {
    word: IntValue<'ctx>,
    gc_ref: Option<PointerValue<'ctx>>,
}

include!("shared.rs");
include!("state_machine_plan.rs");
include!("state_machine_segments.rs");
include!("state_machine_simplify.rs");
include!("nonresuming.rs");
