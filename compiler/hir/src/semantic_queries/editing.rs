//! Partial editor observations remain separate from successful HIR products.

use crate::{FunctionId, SemanticSourceLocation, Span, TypeId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditingUnavailable {
    IncompleteSyntax,
    DeclarationFailure,
    SignatureFailure,
    PendingInference,
}

#[derive(Clone, Debug)]
pub enum EditingValue<T> {
    Known(T),
    Unavailable(EditingUnavailable),
}

/// A source declaration paired with the function allocated for that declaration.
#[derive(Clone, Debug)]
pub struct EditingFunction {
    pub function: FunctionId,
    pub name: String,
    pub name_origin: SemanticSourceLocation,
    pub span: Span,
}

/// Resolved declaration types, without claiming that defaults have been checked.
#[derive(Clone, Debug)]
pub struct EditingParameter {
    pub name: String,
    pub value_type: TypeId,
    pub vararg_element_type: Option<TypeId>,
    pub declared_default: Option<Span>,
    pub origin: SemanticSourceLocation,
}
