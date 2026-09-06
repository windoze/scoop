//! Structured MIR construction form to the output MIR CFG.

use la_arena::Arena;
use scoop_ast::Span;
use scoop_mir as mir;

use crate::structured as smir;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct UnwindScopeId(u32);

#[derive(Clone, Copy)]
struct UnwindTarget {
    owner: UnwindScopeId,
    pad: mir::BlockId,
    continuation: mir::BlockId,
    handles_in_function: bool,
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

#[derive(Clone, Copy)]
struct ResumeTarget {
    block: mir::BlockId,
    cleanup_depth: CleanupDepth,
}

#[derive(Clone, Copy)]
struct LoopExitTarget {
    block: mir::BlockId,
    cleanup_depth: CleanupDepth,
}

#[derive(Clone, Copy)]
struct LoopHeaderTarget {
    block: mir::BlockId,
    cleanup_depth: CleanupDepth,
}

#[derive(Clone, Copy)]
struct LoopTarget {
    source: smir::LoopId,
    exit: LoopExitTarget,
    header: LoopHeaderTarget,
    break_reachable: bool,
}

enum ReturnPayload {
    Unit,
    Value(mir::Expr),
}

enum PendingTransfer {
    Fallthrough(ResumeTarget),
    Return(ReturnPayload),
    Break(LoopExitTarget),
    Continue(LoopHeaderTarget),
}

impl PendingTransfer {
    fn cleanup_depth(&self) -> CleanupDepth {
        match self {
            Self::Fallthrough(target) => target.cleanup_depth,
            Self::Return(_) => CleanupDepth::ROOT,
            Self::Break(target) => target.cleanup_depth,
            Self::Continue(target) => target.cleanup_depth,
        }
    }
}

pub(crate) fn lower(
    body: smir::Body,
    return_ty: mir::Type,
    enums: &Arena<mir::EnumDef>,
) -> mir::Body {
    let smir::Body { locals, statements } = body;
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
        return_ty,
        try_stack: Vec::new(),
        unwind_scope_count: 0,
        normal_cleanups: Vec::new(),
        loop_targets: Vec::new(),
        loop_header_polls: Vec::new(),
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
            && lowerer.loop_targets.is_empty(),
        "structured control scopes are balanced before MIR CFG construction completes"
    );
    mir::Body {
        locals: lowerer.locals,
        blocks: lowerer.blocks,
        entry: lowerer.entry,
        loop_header_polls: lowerer.loop_header_polls,
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
    return_ty: mir::Type,
    try_stack: Vec<UnwindTarget>,
    unwind_scope_count: u32,
    normal_cleanups: Vec<NormalCleanup<'a>>,
    loop_targets: Vec<LoopTarget>,
    loop_header_polls: Vec<mir::LoopHeaderPollTarget>,
    enums: &'a Arena<mir::EnumDef>,
}

mod control;
mod expression;
mod patterns;
mod transfers;

fn trap_message(expr: &smir::Expr) -> Option<mir::StringConstId> {
    let smir::ExprKind::Call(call) = &expr.kind else {
        return None;
    };
    if call.target.callee != mir::Callee::Runtime(mir::RuntimeFn::Trap) {
        return None;
    }
    let [argument] = call.args.as_slice() else {
        panic!("the trap intrinsic always carries one string constant")
    };
    let smir::ExprKind::StringConst(message) = argument.kind else {
        panic!("the trap intrinsic always carries one string constant")
    };
    Some(message)
}

fn synthetic_span() -> Span {
    Span { start: 0, end: 0 }
}
