//! effect/continuation codegen（T0102e：从 `codegen/mod.rs` 拆分）。
//!
//! 现状说明（T2003u1）：
//! - 当前 `immediate_resume` / `escape_continuation` / `mixed` / `matrix` 仍是按源码形状拆开的过渡 lowering；
//! - effect 主线已改为“先构建统一的 suspension-aware state-machine plan，再做 never-resume /
//!   immediate-resume / escape-continuation 的 mode-specific simplification”；
//! - 设计基线见仓库文档 `docs/effect_unified_state_machine.md`。

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
    resume_value_ty: CgTy,
    resume_value_ptr: Option<PointerValue<'ctx>>,
    resume_used_ptr: PointerValue<'ctx>,
    state_ptr: PointerValue<'ctx>,
    next_state: u32,
}

/// Immediate-resume 当前阶段保留的语法形状恢复路径。
///
/// 这些枚举只服务于旧的过渡 lowering；`T2003u2` 起应改由统一 plan 中的 state/edge
/// 与 suspend site 描述恢复路径，而不是继续扩这组源码形状枚举。
#[derive(Debug, Clone, Copy)]
enum ImmediateResumeFrame<'hir> {
    Block {
        block: &'hir hir::Block,
        stmt_idx: usize,
    },
    IfThen {
        if_expr: &'hir hir::Expr,
        then_block: &'hir hir::Block,
        stmt_idx: usize,
    },
    IfElse {
        if_expr: &'hir hir::Expr,
        else_block: &'hir hir::Block,
        stmt_idx: usize,
    },
    WhileBody {
        while_cond: &'hir hir::Expr,
        while_body: &'hir hir::Block,
        stmt_idx: usize,
    },
}

impl<'hir> ImmediateResumeFrame<'hir> {
    fn set_stmt_idx(&mut self, idx: usize) {
        match self {
            ImmediateResumeFrame::Block { stmt_idx, .. }
            | ImmediateResumeFrame::IfThen { stmt_idx, .. }
            | ImmediateResumeFrame::IfElse { stmt_idx, .. }
            | ImmediateResumeFrame::WhileBody { stmt_idx, .. } => {
                *stmt_idx = idx;
            }
        }
    }

    fn stmt_idx(&self) -> usize {
        match self {
            ImmediateResumeFrame::Block { stmt_idx, .. }
            | ImmediateResumeFrame::IfThen { stmt_idx, .. }
            | ImmediateResumeFrame::IfElse { stmt_idx, .. }
            | ImmediateResumeFrame::WhileBody { stmt_idx, .. } => *stmt_idx,
        }
    }
}

/// Mixed escape-continuation 当前阶段保留的语法形状恢复路径。
///
/// 与 `ImmediateResumeFrame` 一样，这只是旧 lowering 的临时描述；统一状态机 pass
/// 落地后应收口到统一的 suspend site / resume target / cleanup edge 模型。
#[derive(Debug, Clone, Copy)]
enum MixedEscapeDirectFrame<'hir> {
    Block {
        block: &'hir hir::Block,
        stmt_idx: usize,
    },
    IfThen {
        if_expr: &'hir hir::Expr,
        then_block: &'hir hir::Block,
        stmt_idx: usize,
    },
    IfElse {
        if_expr: &'hir hir::Expr,
        else_block: &'hir hir::Block,
        stmt_idx: usize,
    },
    WhileBody {
        while_cond: &'hir hir::Expr,
        while_body: &'hir hir::Block,
        stmt_idx: usize,
    },
}

impl<'hir> MixedEscapeDirectFrame<'hir> {
    fn set_stmt_idx(&mut self, idx: usize) {
        match self {
            MixedEscapeDirectFrame::Block { stmt_idx, .. }
            | MixedEscapeDirectFrame::IfThen { stmt_idx, .. }
            | MixedEscapeDirectFrame::IfElse { stmt_idx, .. }
            | MixedEscapeDirectFrame::WhileBody { stmt_idx, .. } => {
                *stmt_idx = idx;
            }
        }
    }

    fn stmt_idx(&self) -> usize {
        match self {
            MixedEscapeDirectFrame::Block { stmt_idx, .. }
            | MixedEscapeDirectFrame::IfThen { stmt_idx, .. }
            | MixedEscapeDirectFrame::IfElse { stmt_idx, .. }
            | MixedEscapeDirectFrame::WhileBody { stmt_idx, .. } => *stmt_idx,
        }
    }
}

#[derive(Debug, Clone)]
struct MixedEscapeDirectSite<'hir> {
    top_level_stmt_idx: usize,
    decl: &'hir hir::ValDecl,
    args: &'hir [hir::CallArg],
    id: hir::SymbolId,
    resume_path: Vec<MixedEscapeDirectFrame<'hir>>,
}

#[derive(Debug, Clone)]
struct MixedEscapeIndirectSite<'hir> {
    top_level_stmt_idx: usize,
    decl: &'hir hir::ValDecl,
    init: &'hir hir::Expr,
    id: hir::SymbolId,
    resume_path: Vec<MixedEscapeDirectFrame<'hir>>,
}

#[derive(Debug, Clone)]
enum MatrixEscapeSiteKind<'hir> {
    Direct { site: MixedEscapeDirectSite<'hir> },
    Indirect { site: MixedEscapeIndirectSite<'hir> },
}

#[derive(Debug, Clone)]
struct MatrixEscapeSite<'hir> {
    stmt_idx: usize,
    decl: &'hir hir::ValDecl,
    id: hir::SymbolId,
    kind: MatrixEscapeSiteKind<'hir>,
}

#[derive(Debug, Clone, Copy)]
struct EscapeCaptureMeta {
    id: hir::SymbolId,
    hir_ty: Option<TypeId>,
    ty: CgTy,
    mutable: bool,
}

#[derive(Debug)]
struct ImmediateResumeSite<'hir> {
    top_level_stmt_idx: usize,
    decl: &'hir hir::ValDecl,
    op: &'hir hir::EffectOpRef,
    args: &'hir [hir::CallArg],
    id: hir::SymbolId,
    resume_path: Vec<ImmediateResumeFrame<'hir>>,
}

#[derive(Debug, Clone, Copy)]
struct ImmediateResumeBinderSlot<'ctx> {
    id: hir::SymbolId,
    hir_ty: TypeId,
    ty: CgTy,
    ptr: PointerValue<'ctx>,
}

#[derive(Debug, Clone, Copy)]
struct ImmediateResumeArmDispatch<'a, 'ctx> {
    binder_slots: &'a [ImmediateResumeBinderSlot<'ctx>],
    resume_used_ptr: PointerValue<'ctx>,
    arm_bb: inkwell::basic_block::BasicBlock<'ctx>,
}

#[derive(Debug, Clone, Copy)]
struct ImmediateResumeExecPlan<'hir, 'ctx> {
    handle: &'hir hir::HandleExpr,
    site: &'hir ImmediateResumeSite<'hir>,
    out_ty: CgTy,
    result_ptr: Option<PointerValue<'ctx>>,
    handler_exit: ImmediateResumeHandlerExit<'ctx>,
    finally_bb: inkwell::basic_block::BasicBlock<'ctx>,
}

#[derive(Debug, Clone, Copy)]
enum ImmediateResumeHandlerExit<'ctx> {
    None,
    PopFrame(PointerValue<'ctx>),
    SwapTop(PointerValue<'ctx>),
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

/// T1606f-2: Info about a function call site in the handle body that may indirectly perform.
struct IndirectPerformCallSite {
    /// Index of the stmt in handle.body.stmts.
    stmt_idx: usize,
    /// SymbolId of the val binding for the call result.
    _result_id: hir::SymbolId,
    /// HIR type of the call result.
    result_ty: TypeId,
}

struct IndirectEscapeContinuationPlan {
    continuation_symbol: hir::SymbolId,
    seq: u32,
    out_ty: CgTy,
    indirect_sites: Vec<IndirectPerformCallSite>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EscapeCaptureStorageKind {
    Word,
    GcRef,
}

#[derive(Debug, Clone, Copy)]
struct SiblingNonresumingArm<'hir> {
    arm: &'hir hir::HandleArm,
    op_tag: u32,
}

#[derive(Debug, Clone)]
struct SiblingNonresumingPlan<'hir> {
    raise_arm: Option<&'hir hir::HandleArm>,
    custom_arms: Vec<SiblingNonresumingArm<'hir>>,
}

impl SiblingNonresumingPlan<'_> {
    fn has_any(&self) -> bool {
        self.raise_arm.is_some() || !self.custom_arms.is_empty()
    }
}

#[derive(Debug, Clone)]
struct SiblingNonresumingDispatchBlocks<'ctx> {
    effect_dispatch_bb: Option<inkwell::basic_block::BasicBlock<'ctx>>,
    effect_dispatch_nomatch_bb: Option<inkwell::basic_block::BasicBlock<'ctx>>,
    raise_catch_bb: Option<inkwell::basic_block::BasicBlock<'ctx>>,
    custom_catch_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>>,
}

#[derive(Debug, Clone, Copy)]
struct EscapeHandleBlocks<'ctx> {
    body_bb: inkwell::basic_block::BasicBlock<'ctx>,
    dispatch_bb: Option<inkwell::basic_block::BasicBlock<'ctx>>,
    dispatch_nomatch_bb: Option<inkwell::basic_block::BasicBlock<'ctx>>,
    arm_bb: inkwell::basic_block::BasicBlock<'ctx>,
    done_bb: inkwell::basic_block::BasicBlock<'ctx>,
    finally_bb: Option<inkwell::basic_block::BasicBlock<'ctx>>,
    finally_unwind_bb: Option<inkwell::basic_block::BasicBlock<'ctx>>,
}

#[derive(Debug, Clone, Copy)]
struct MixedEscapeResumeBlocks<'ctx> {
    dispatch_bb: inkwell::basic_block::BasicBlock<'ctx>,
    state0_bb: inkwell::basic_block::BasicBlock<'ctx>,
    state1_bb: inkwell::basic_block::BasicBlock<'ctx>,
    arm_bb: inkwell::basic_block::BasicBlock<'ctx>,
    done_bb: inkwell::basic_block::BasicBlock<'ctx>,
    bad_state_bb: inkwell::basic_block::BasicBlock<'ctx>,
    finally_bb: inkwell::basic_block::BasicBlock<'ctx>,
    finally_unwind_bb: inkwell::basic_block::BasicBlock<'ctx>,
}

include!("shared.rs");
include!("scan.rs");
include!("nonresuming.rs");
include!("immediate_resume.rs");
include!("escape_continuation.rs");
include!("multi_escape.rs");
include!("mixed.rs");
include!("matrix.rs");
