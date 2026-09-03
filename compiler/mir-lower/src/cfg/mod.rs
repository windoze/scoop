//! Structured MIR construction form to the output MIR CFG.

use la_arena::Arena;
use scoop_ast::Span;
use scoop_mir as mir;

use crate::structured as smir;

#[derive(Clone, Copy)]
struct UnwindTarget {
    pad: mir::BlockId,
    continuation: mir::BlockId,
    handles_in_function: bool,
}

#[derive(Clone)]
enum ReturnCleanup<'a> {
    Finally {
        owner_unwind: mir::BlockId,
        body: &'a [smir::Statement],
    },
    EndCatch {
        cleanup_pad: mir::BlockId,
    },
}

pub(crate) fn lower(body: smir::Body, return_ty: mir::Type) -> mir::Body {
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
        return_cleanups: Vec::new(),
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
    mir::Body {
        locals: lowerer.locals,
        blocks: lowerer.blocks,
        entry: lowerer.entry,
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
    return_cleanups: Vec<ReturnCleanup<'a>>,
}

mod control;
mod expression;

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
