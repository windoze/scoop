//! Complete callable headers with an unfinished literal body.

use crate::{BodyBlockId, Ident, LambdaParam, Param, Span, TypeRef};

#[derive(Debug, Clone, PartialEq)]
pub enum RecoveredCallableHeader {
    Lambda {
        parameters: Option<Vec<LambdaParam>>,
    },
    AnonymousFunction {
        receiver: Option<Box<TypeRef>>,
        parameters: Vec<Param>,
        return_ty: Option<Box<TypeRef>>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredCallableLiteral {
    pub block: Option<BodyBlockId>,
    pub span: Span,
    pub label: Option<Ident>,
    pub is_suspend: bool,
    pub header: RecoveredCallableHeader,
    pub body: BodyBlockId,
}
