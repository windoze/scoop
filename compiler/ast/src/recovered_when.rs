//! Original when inputs retained only for an incomplete production.

use crate::{
    BodyBlockId, Expr, RecoveredControlBody, Span, WhenArmCondition, WhenCondition, WhenSubject,
};

#[derive(Debug, Clone, PartialEq)]
pub enum RecoveredWhenArm {
    Head {
        condition: WhenArmCondition,
        guard: Option<Expr>,
        body: Option<RecoveredControlBody>,
        span: Span,
    },
    Guard {
        condition: WhenArmCondition,
        expression_start: u32,
        span: Span,
    },
    Condition {
        previous: Vec<WhenCondition>,
        expression_start: u32,
        span: Span,
    },
    Unavailable(Span),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredWhen {
    pub block: Option<BodyBlockId>,
    pub start: u32,
    pub subject: WhenSubject,
    pub arms: Vec<RecoveredWhenArm>,
}
