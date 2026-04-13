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

/// escape-continuation 从 unified source path 恢复出的通用 resume 路径。
///
/// 说明：
/// - 这些 frame 不是旧 shape-based route 的“主入口”；它们只是把
///   `SuspendSourcePath` 还原回 leaf helper 后续需要的 HIR 引用。
/// - `T2003r3d2a` 只补 metadata/resolver，不在这里恢复 dedicated emitter。
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
enum ResumeFrame<'hir> {
    IfThen {
        if_expr: &'hir hir::Expr,
        then_block_stmts: &'hir [hir::Stmt],
        resume_after_stmt: usize,
    },
    IfElse {
        if_expr: &'hir hir::Expr,
        else_block_stmts: &'hir [hir::Stmt],
        resume_after_stmt: usize,
    },
    WhenArm {
        when_expr: &'hir hir::Expr,
        arm_index: usize,
        arm_block_stmts: &'hir [hir::Stmt],
        resume_after_stmt: usize,
    },
    WhileBody {
        while_cond: &'hir hir::Expr,
        while_body: &'hir hir::Block,
        resume_after_stmt: usize,
    },
    Block {
        block: &'hir hir::Block,
        resume_after_stmt: usize,
    },
}

/// immediate-resume leaf 从 unified source path 恢复出的 resume 路径。
#[allow(dead_code)]
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

#[allow(dead_code)]
impl<'hir> ImmediateResumeFrame<'hir> {
    fn stmt_idx(&self) -> usize {
        match self {
            ImmediateResumeFrame::Block { stmt_idx, .. }
            | ImmediateResumeFrame::IfThen { stmt_idx, .. }
            | ImmediateResumeFrame::IfElse { stmt_idx, .. }
            | ImmediateResumeFrame::WhileBody { stmt_idx, .. } => *stmt_idx,
        }
    }
}

/// mixed immediate+escape / pure multi-escape 直连 site 恢复出的路径。
#[allow(dead_code)]
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

#[allow(dead_code)]
impl<'hir> MixedEscapeDirectFrame<'hir> {
    fn stmt_idx(&self) -> usize {
        match self {
            MixedEscapeDirectFrame::Block { stmt_idx, .. }
            | MixedEscapeDirectFrame::IfThen { stmt_idx, .. }
            | MixedEscapeDirectFrame::IfElse { stmt_idx, .. }
            | MixedEscapeDirectFrame::WhileBody { stmt_idx, .. } => *stmt_idx,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
struct ImmediateResumeSite<'hir> {
    top_level_stmt_idx: usize,
    decl: &'hir hir::ValDecl,
    op: &'hir hir::EffectOpRef,
    args: &'hir [hir::CallArg],
    id: hir::SymbolId,
    resume_path: Vec<ImmediateResumeFrame<'hir>>,
}

#[allow(dead_code)]
#[derive(Debug)]
struct ResolvedImmediateResumeSite<'hir> {
    arm_id: ArmPlanId,
    site: ImmediateResumeSite<'hir>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct MixedEscapeDirectSite<'hir> {
    top_level_stmt_idx: usize,
    decl: &'hir hir::ValDecl,
    args: &'hir [hir::CallArg],
    id: hir::SymbolId,
    resume_path: Vec<MixedEscapeDirectFrame<'hir>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct MixedEscapeIndirectSite<'hir> {
    top_level_stmt_idx: usize,
    decl: &'hir hir::ValDecl,
    init: &'hir hir::Expr,
    id: hir::SymbolId,
    resume_path: Vec<MixedEscapeDirectFrame<'hir>>,
}

#[allow(dead_code)]
#[derive(Debug)]
struct ResolvedPlanMixedEscapeDirectSite<'hir> {
    arm_id: ArmPlanId,
    site: MixedEscapeDirectSite<'hir>,
}

#[allow(dead_code)]
#[derive(Debug)]
struct ResolvedPlanMixedEscapeDirectSites<'hir> {
    direct_sites: Vec<ResolvedPlanMixedEscapeDirectSite<'hir>>,
    capture_ids: HashSet<hir::SymbolId>,
}

#[allow(dead_code)]
#[derive(Debug)]
struct ResolvedPlanMixedEscapeIndirectSites<'hir> {
    indirect_sites: Vec<MixedEscapeIndirectSite<'hir>>,
    capture_ids: HashSet<hir::SymbolId>,
}

#[allow(dead_code)]
#[derive(Debug)]
struct ResolvedEscapeDirectSites<'hir> {
    perform_sites: Vec<NestedPerformSite<'hir>>,
    capture_ids: HashSet<hir::SymbolId>,
}

#[allow(dead_code)]
#[derive(Debug)]
struct ResolvedEscapeIndirectSites {
    indirect_sites: Vec<IndirectPerformCallSite>,
    capture_ids: HashSet<hir::SymbolId>,
}

#[allow(dead_code)]
#[derive(Debug)]
struct ResolvedPlanImmediateEscapeSites<'hir> {
    perform_site: ImmediateResumeSite<'hir>,
    direct_sites: Vec<MixedEscapeDirectSite<'hir>>,
    indirect_sites: Vec<MixedEscapeIndirectSite<'hir>>,
}

#[allow(dead_code)]
#[derive(Debug)]
struct NestedPerformSite<'hir> {
    pc: usize,
    decl: &'hir hir::ValDecl,
    op: &'hir hir::EffectOpRef,
    args: &'hir [hir::CallArg],
    id: hir::SymbolId,
    resume_path: Vec<ResumeFrame<'hir>>,
    top_level_stmt_idx: usize,
}

#[allow(dead_code)]
#[derive(Debug)]
struct IndirectPerformCallSite {
    stmt_idx: usize,
    _result_id: hir::SymbolId,
    result_ty: TypeId,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EscapeCaptureStorageKind {
    Word,
    GcRef,
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
