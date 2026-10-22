//! Partial editor observations remain separate from successful HIR products.

use crate::{FunctionId, SemanticSourceLocation, Span, TypeId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditingUnavailable {
    IncompleteSyntax,
    DeclarationFailure,
    SignatureFailure,
    PendingInference,
    BodyFailure,
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

/// A local reference observed by the ordinary name resolver.
#[derive(Clone, Debug)]
pub struct EditingLocalReference {
    pub binding: crate::BindingId,
    pub name: String,
    pub definition: SemanticSourceLocation,
}

/// Facts about an existing receiver before its missing selector.
#[derive(Clone, Debug)]
pub struct EditingReceiver {
    pub span: Span,
    pub dot: Span,
    pub selector: Span,
    pub safe_navigation: bool,
    pub value_type: EditingValue<TypeId>,
    pub local: Option<EditingLocalReference>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditingLocalRole {
    Declaration,
    Read,
    Write,
    ReadWrite,
}

#[derive(Clone, Debug)]
pub struct EditingExpression {
    pub span: Span,
    pub context: crate::SourceContextId,
    pub value_type: TypeId,
    pub expected_type: EditingValue<Option<TypeId>>,
}

#[derive(Clone, Debug)]
pub struct EditingLocalOccurrence {
    pub span: Span,
    pub context: crate::SourceContextId,
    pub reference: EditingLocalReference,
    pub value_type: TypeId,
    pub role: EditingLocalRole,
}

/// Source observations from a checked body, without claiming a successful Module.
#[derive(Clone, Debug, Default)]
pub struct EditingBodyFacts {
    pub locals: Vec<EditingLocalOccurrence>,
    pub expressions: Vec<EditingExpression>,
    pub callables: Vec<EditingCallableReference>,
}

#[derive(Clone, Debug)]
pub struct EditingCallableReference {
    pub span: Span,
    pub target: crate::SemanticCallable,
    pub definition: Option<SemanticSourceLocation>,
}
