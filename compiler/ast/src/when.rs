//! Source distinction between ordinary conditions and explicit match patterns.

use crate::{Block, Expr, NonEmptyVec, Pattern, Span, TypeRef, ValDecl};

#[derive(Debug, Clone, PartialEq)]
pub struct When {
    pub subject: WhenSubject,
    pub arms: Vec<WhenArm>,
    pub else_body: Option<Block>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WhenSubject {
    Absent,
    Expression(Expr),
    Declaration(Box<ValDecl>),
}

impl WhenSubject {
    pub fn initializer(&self) -> Option<&Expr> {
        match self {
            Self::Absent => None,
            Self::Expression(value) => Some(value),
            Self::Declaration(value) => Some(&value.init),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhenArm {
    pub condition: WhenArmCondition,
    pub guard: Option<Expr>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WhenArmCondition {
    Case(Pattern),
    Conditions(NonEmptyVec<WhenCondition>),
    /// A guarded fallback; an unconditional else is stored on `When`.
    Else,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WhenCondition {
    Expression(Expr),
    Is {
        ty: TypeRef,
        negated: bool,
        span: Span,
    },
    In {
        collection: Expr,
        negated: bool,
        span: Span,
    },
}

impl WhenCondition {
    pub fn span(&self) -> Span {
        match self {
            Self::Expression(value) => value.span(),
            Self::Is { span, .. } | Self::In { span, .. } => *span,
        }
    }
}
