//! Structured MIR construction form to the output MIR CFG.

use la_arena::Arena;
use scoop_hir::concrete::Span;
use scoop_hir::concrete::{StructuralDefinitionSiteRole, SyntheticLocalRole};
use scoop_mir as mir;

use crate::structured as smir;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct UnwindScopeId(u32);

#[derive(Clone)]
struct UnwindTarget {
    owner: UnwindScopeId,
    pad: mir::BlockId,
    continuation: mir::BlockId,
    handles_in_function: bool,
    context: CoroutinePendingContext,
    managed_exception: Option<mir::LocalId>,
}

#[derive(Clone, Default)]
struct CoroutinePendingContext(Vec<mir::CoroutinePendingSourceTransfer>);

impl CoroutinePendingContext {
    fn with_transfer(&self, transfer: mir::CoroutinePendingSourceTransfer) -> Self {
        let mut chain = self.0.clone();
        chain.push(transfer);
        Self(chain)
    }

    fn to_mir(&self) -> mir::CoroutinePendingContext {
        mir::CoroutinePendingContext::from_transfers(self.0.clone()).unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CleanupDepth(usize);

impl CleanupDepth {
    const ROOT: Self = Self(0);
}

#[derive(Clone, Copy)]
struct CleanupCursor {
    next: CleanupDepth,
    stop: CleanupDepth,
}

impl CleanupCursor {
    fn new(next: CleanupDepth, stop: CleanupDepth) -> Self {
        assert!(
            stop <= next,
            "a cleanup cursor stops at a prefix of its active stack"
        );
        Self { next, stop }
    }

    fn take_next(&mut self) -> Option<usize> {
        if self.next == self.stop {
            return None;
        }
        self.next.0 -= 1;
        Some(self.next.0)
    }
}

#[derive(Clone, Copy)]
enum NormalCleanup<'a> {
    Finally {
        owner: UnwindScopeId,
        body: &'a [smir::Statement],
    },
    EndCatch {
        owner: UnwindScopeId,
    },
}

#[derive(Clone)]
struct ResumeTarget {
    block: mir::BlockId,
    cleanup_depth: CleanupDepth,
    context: CoroutinePendingContext,
}

#[derive(Clone)]
struct LoopExitTarget {
    block: mir::BlockId,
    cleanup_depth: CleanupDepth,
    context: CoroutinePendingContext,
}

#[derive(Clone)]
struct LoopHeaderTarget {
    block: mir::BlockId,
    cleanup_depth: CleanupDepth,
    context: CoroutinePendingContext,
}

#[derive(Clone)]
struct LoopTarget {
    source: smir::LoopId,
    exit: LoopExitTarget,
    header: LoopHeaderTarget,
    break_reachable: bool,
}

#[derive(Clone)]
enum ReturnPayload {
    Unit,
    Value(mir::Expr),
}

#[derive(Clone)]
struct ManagedThrowPayload {
    exception: mir::Expr,
    unwind: Option<UnwindTarget>,
    cleanup_depth: CleanupDepth,
}

enum PendingTransfer {
    Fallthrough(ResumeTarget),
    Return(ReturnPayload),
    Break(LoopExitTarget),
    Continue(LoopHeaderTarget),
    ManagedThrow(ManagedThrowPayload),
}

impl PendingTransfer {
    fn cleanup_depth(&self) -> CleanupDepth {
        match self {
            Self::Fallthrough(target) => target.cleanup_depth,
            Self::Return(_) => CleanupDepth::ROOT,
            Self::Break(target) => target.cleanup_depth,
            Self::Continue(target) => target.cleanup_depth,
            Self::ManagedThrow(transfer) => transfer.cleanup_depth,
        }
    }

    fn destination_context(&self) -> CoroutinePendingContext {
        match self {
            Self::Fallthrough(target) => target.context.clone(),
            Self::Return(_) => CoroutinePendingContext::default(),
            Self::Break(target) => target.context.clone(),
            Self::Continue(target) => target.context.clone(),
            Self::ManagedThrow(transfer) => transfer
                .unwind
                .as_ref()
                .map_or_else(CoroutinePendingContext::default, |target| {
                    target.context.clone()
                }),
        }
    }

    fn source_transfer(&self) -> mir::CoroutinePendingSourceTransfer {
        match self {
            Self::Fallthrough(target) => mir::CoroutinePendingSourceTransfer::Fallthrough(
                mir::CoroutineCleanupFallthroughTarget::new(target.block),
            ),
            Self::Return(ReturnPayload::Unit) => {
                mir::CoroutinePendingSourceTransfer::Return(mir::CoroutinePendingSourceReturn::Unit)
            }
            Self::Return(ReturnPayload::Value(value)) => {
                mir::CoroutinePendingSourceTransfer::Return(
                    mir::CoroutinePendingSourceReturn::value(value.clone())
                        .expect("a pending return payload is an immutable stable local"),
                )
            }
            Self::Break(target) => mir::CoroutinePendingSourceTransfer::Break(
                mir::CoroutineLoopExitTarget::new(target.block),
            ),
            Self::Continue(target) => mir::CoroutinePendingSourceTransfer::Continue(
                mir::CoroutineLoopHeaderTarget::new(target.block),
            ),
            Self::ManagedThrow(transfer) => mir::CoroutinePendingSourceTransfer::ManagedThrow(
                mir::CoroutinePendingSourceManagedThrow::checked(
                    transfer.exception.clone(),
                    transfer
                        .unwind
                        .as_ref()
                        .map(|target| mir::CoroutineUnwindTarget::new(target.pad)),
                )
                .expect("a pending managed exception is an exact stable local"),
            ),
        }
    }
}

pub(crate) struct LoweredBody {
    pub(crate) body: mir::Body,
    pub(crate) generated_values: Vec<GeneratedLocalValue>,
    /// Calls in structured evaluation order, with their locations in the emitted CFG.
    pub(crate) call_sites: Vec<CallSite>,
}

#[derive(Clone, Copy)]
pub(crate) struct CallSite {
    pub(crate) block: mir::BlockId,
    pub(crate) statement: usize,
}

pub(crate) struct GeneratedLocalValue {
    pub(crate) local: mir::LocalId,
    pub(crate) site_role: StructuralDefinitionSiteRole,
    pub(crate) role: SyntheticLocalRole,
}

pub(crate) fn lower(
    body: smir::Body,
    return_ty: mir::Type,
    enums: &Arena<mir::EnumDef>,
) -> LoweredBody {
    let smir::Body {
        locals,
        statements,
        coroutine_eh,
    } = body;
    let mut blocks = Arena::new();
    let entry = blocks.alloc(mir::BasicBlock {
        name: "entry".to_string(),
        statements: Vec::new(),
        terminator: mir::Terminator::Unreachable,
        unwind: None,
    });
    let mut lowerer = CfgLowerer {
        locals,
        blocks,
        entry,
        current: entry,
        current_sealed: false,
        block_count: 0,
        hidden_count: 0,
        generated_values: Vec::new(),
        call_sites: Vec::new(),
        return_ty,
        try_stack: Vec::new(),
        unwind_scope_count: 0,
        normal_cleanups: Vec::new(),
        loop_targets: Vec::new(),
        loop_header_polls: Vec::new(),
        active_pending: CoroutinePendingContext::default(),
        coroutine_eh,
        enums,
    };
    lowerer.lower_statements(&statements);
    if !lowerer.current_sealed {
        let terminator = if lowerer.return_ty == mir::Type::Unit {
            mir::Terminator::Return { value: None }
        } else {
            mir::Terminator::Unreachable
        };
        lowerer.seal(terminator);
    }
    assert!(
        lowerer.try_stack.is_empty()
            && lowerer.normal_cleanups.is_empty()
            && lowerer.loop_targets.is_empty()
            && lowerer.active_pending.0.is_empty(),
        "structured control scopes are balanced before MIR CFG construction completes"
    );
    LoweredBody {
        body: mir::Body {
            locals: lowerer.locals,
            blocks: lowerer.blocks,
            entry: lowerer.entry,
            loop_header_polls: lowerer.loop_header_polls,
        },
        generated_values: lowerer.generated_values,
        call_sites: lowerer.call_sites,
    }
}

struct CfgLowerer<'a> {
    locals: Arena<mir::Local>,
    blocks: Arena<mir::BasicBlock>,
    entry: mir::BlockId,
    current: mir::BlockId,
    current_sealed: bool,
    block_count: usize,
    hidden_count: usize,
    generated_values: Vec<GeneratedLocalValue>,
    call_sites: Vec<CallSite>,
    return_ty: mir::Type,
    try_stack: Vec<UnwindTarget>,
    unwind_scope_count: u32,
    normal_cleanups: Vec<NormalCleanup<'a>>,
    loop_targets: Vec<LoopTarget>,
    loop_header_polls: Vec<mir::LoopHeaderPollTarget>,
    active_pending: CoroutinePendingContext,
    coroutine_eh: Option<smir::CoroutineEhMode>,
    enums: &'a Arena<mir::EnumDef>,
}

mod control;
mod expression;
mod patterns;
mod transfers;

fn synthetic_span() -> Span {
    Span { start: 0, end: 0 }
}
