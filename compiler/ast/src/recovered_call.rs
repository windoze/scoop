//! Source call inputs retained by the grammar without a complete call AST.

use crate::{
    BodyBlockId, CallArgumentName, CallTypeArgument, Expr, Ident, Navigation, Span, SpreadSyntax,
    TypeRef,
};

#[derive(Debug, Clone, PartialEq)]
pub enum RecoveredCallTarget {
    Named(Ident),
    Member {
        receiver: Box<Expr>,
        name: Ident,
        navigation: Navigation,
    },
    Invoke(Box<Expr>),
    Super {
        super_span: Span,
        name: Ident,
    },
    QualifiedSuper {
        super_span: Span,
        qualifier: Box<TypeRef>,
        name: Ident,
    },
    Unavailable(Span),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecoveredArgumentValue {
    Complete(Box<Expr>),
    CallableLiteral(BodyBlockId),
    Unavailable(Span),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredCallArgument {
    pub name: CallArgumentName,
    pub spread: SpreadSyntax,
    pub value: RecoveredArgumentValue,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredCallArguments {
    pub opening: Span,
    pub commas: Vec<Span>,
    pub end: u32,
    pub closed: bool,
    pub items: Vec<RecoveredCallArgument>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredCall {
    pub block: Option<BodyBlockId>,
    pub target: RecoveredCallTarget,
    pub type_args: Vec<CallTypeArgument>,
    pub arguments: RecoveredCallArguments,
    pub trailing: Option<RecoveredCallArgument>,
    pub span: Span,
}
