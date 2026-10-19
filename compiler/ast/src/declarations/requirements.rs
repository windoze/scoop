use crate::{Annotation, CallTypeArgument, Ident, Span, SpreadSyntax, TypeRef};

/// A declaration-only block of typed virtual calls, never executable statements.
#[derive(Debug, Clone, PartialEq)]
pub struct MemberRequirements {
    pub parameters: Vec<RequirementParameter>,
    pub calls: Vec<MemberRequirement>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RequirementParameter {
    pub name: Ident,
    pub ty: TypeRef,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemberRequirement {
    pub annotations: Vec<Annotation>,
    pub suspend_span: Option<Span>,
    pub call: RequiredCall,
    pub arguments: Vec<RequirementArgument>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RequiredCall {
    Method {
        receiver: Ident,
        companion: bool,
        name: Ident,
        type_arguments: Vec<CallTypeArgument>,
        result: TypeRef,
    },
    Constructor {
        parameter: Ident,
        result: Option<TypeRef>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RequirementArgument {
    pub name: Option<Ident>,
    pub spread: SpreadSyntax,
    pub value: RequirementArgumentValue,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RequirementArgumentValue {
    Parameter(Ident),
    Inline { name: Ident, ty: TypeRef },
}
