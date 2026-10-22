//! Exception-region inputs with an explicit boundary for each unfinished block.

use crate::{Block, BodyBlockId, Ident, Span, TypeRef};

#[derive(Debug, Clone, PartialEq)]
pub enum RecoveredTryBlock {
    Complete(Box<Block>),
    Incomplete(BodyBlockId),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecoveredCatch {
    Clause {
        name: Ident,
        ty: TypeRef,
        body: RecoveredTryBlock,
        span: Span,
    },
    Unavailable(Span),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredTry {
    pub block: Option<BodyBlockId>,
    pub start: u32,
    pub body: RecoveredTryBlock,
    pub catches: Vec<RecoveredCatch>,
    pub finally_body: Option<BodyBlockId>,
}
