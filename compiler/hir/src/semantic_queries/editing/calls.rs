use crate::{
    EditingCallee, FunctionId, SemanticDeclarationBinders, SemanticSourceLocation, SourceContextId,
    Span, TypeId,
};

/// The actual type environment of a completed source call.
#[derive(Clone, Debug)]
pub enum EditingCallContext {
    Body {
        function: FunctionId,
        source: SourceContextId,
    },
    Declaration(SemanticDeclarationBinders),
}

/// Applied parameter types and source bindings from one checked call expression.
#[derive(Clone, Debug)]
pub struct EditingAppliedCall {
    pub span: Span,
    pub callee_span: Option<Span>,
    pub callee_end: u32,
    pub name: Option<String>,
    pub callee: Option<EditingCallee>,
    pub context: EditingCallContext,
    pub parameters: Vec<EditingAppliedParameter>,
    pub arguments: Vec<EditingCallArgument>,
    pub result: TypeId,
}

/// A source call retained by successful compile-time evaluation.
#[derive(Clone, Debug)]
pub struct EditingStaticCall {
    pub source: scoop_identity::SourceIdentity,
    pub call: EditingAppliedCall,
}

#[derive(Clone, Debug)]
pub struct EditingAppliedParameter {
    pub name: Option<String>,
    pub value_type: TypeId,
    pub vararg_element_type: Option<TypeId>,
}

#[derive(Clone, Debug)]
pub struct EditingCallArgument {
    pub span: Span,
    pub name_span: Option<Span>,
    pub parameter: usize,
    pub trailing: bool,
}

/// The original parameter binding and applied type of a checked named argument.
#[derive(Clone, Debug)]
pub struct EditingNamedArgument {
    pub parameter: EditingAppliedParameter,
    pub index: usize,
    pub callee: Option<EditingCallee>,
    pub context: EditingCallContext,
    pub definition: Option<SemanticSourceLocation>,
}
