//! Grammar facts for incomplete function bodies, separate from complete ASTs.

use crate::{Expr, Navigation, Span, Statement};

/// The actual opening token offset, scoped to one source revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BodyBlockId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveredBlockScope {
    Block,
    Lambda,
    AnonymousFunction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecoveredStatement {
    Complete(Box<Statement>),
    Unavailable(Span),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredBlock {
    pub id: BodyBlockId,
    pub parent: Option<BodyBlockId>,
    pub scope: RecoveredBlockScope,
    pub span: Span,
    pub closed: bool,
    pub statements: Vec<RecoveredStatement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecoveredReceiver {
    Complete(Box<Expr>),
    Unavailable(Span),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredSelector {
    pub block: Option<BodyBlockId>,
    pub receiver: RecoveredReceiver,
    pub navigation: Navigation,
    pub dot: Span,
    pub selector: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveredBodyRoot {
    Block(BodyBlockId),
    Expression(u32),
}

/// Complete syntax preceding an unfinished statement operand.
#[derive(Debug, Clone, PartialEq)]
pub enum RecoveredOperandContext {
    Initializer {
        annotation: Option<Box<crate::TypeRef>>,
    },
    Return {
        label: Option<crate::Ident>,
    },
    Throw,
    IfCondition,
    WhileCondition {
        label: Option<crate::Ident>,
    },
    ForIterable,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredOperand {
    pub block: Option<BodyBlockId>,
    pub statement_start: u32,
    pub expression_start: u32,
    pub context: RecoveredOperandContext,
}

/// A control body has a lexical scope even without an opening brace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveredControlBody {
    Block(BodyBlockId),
    Statement(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveredIfBranch {
    pub on_true: bool,
    pub body: RecoveredControlBody,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredIf {
    pub block: Option<BodyBlockId>,
    pub start: u32,
    pub condition: Box<Expr>,
    pub branches: Vec<RecoveredIfBranch>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredWhile {
    pub block: Option<BodyBlockId>,
    pub start: u32,
    pub label: Option<crate::Ident>,
    pub condition: Box<Expr>,
    pub body: RecoveredControlBody,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredFor {
    pub block: Option<BodyBlockId>,
    pub span: Span,
    pub label: Option<crate::Ident>,
    pub pattern: Box<crate::Pattern>,
    pub iterable: Box<Expr>,
    pub body: RecoveredControlBody,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecoveredDoWhilePart {
    Body(RecoveredControlBody),
    Condition {
        body: Box<crate::Block>,
        expression_start: u32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredDoWhile {
    pub block: Option<BodyBlockId>,
    pub start: u32,
    pub label: Option<crate::Ident>,
    pub part: RecoveredDoWhilePart,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredFunctionBody {
    pub root: RecoveredBodyRoot,
    pub blocks: Vec<RecoveredBlock>,
    pub selectors: Vec<RecoveredSelector>,
    pub calls: Vec<crate::RecoveredCall>,
    pub operands: Vec<RecoveredOperand>,
    pub conditionals: Vec<RecoveredIf>,
    pub while_loops: Vec<RecoveredWhile>,
    pub for_loops: Vec<RecoveredFor>,
    pub do_while_loops: Vec<RecoveredDoWhile>,
}
