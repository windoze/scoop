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

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredFunctionBody {
    pub root: RecoveredBodyRoot,
    pub blocks: Vec<RecoveredBlock>,
    pub selectors: Vec<RecoveredSelector>,
    pub calls: Vec<crate::RecoveredCall>,
}
