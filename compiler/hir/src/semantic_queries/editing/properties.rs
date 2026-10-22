use crate::{
    PropertyDeclarationId, PropertyId, SemanticSourceLocation, SourceContextId, Span, TypeId,
};

/// A logical property, independently of its storage or accessor implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticProperty {
    Current(PropertyId),
    Imported(PropertyDeclarationId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditingPropertyAccess {
    Read,
    Write,
    ReadWrite,
}

#[derive(Clone, Debug)]
pub struct EditingPropertyReference {
    pub span: Span,
    pub context: SourceContextId,
    pub target: SemanticProperty,
    pub name: String,
    pub mutable: bool,
    pub value_type: TypeId,
    pub definition: Option<SemanticSourceLocation>,
    pub access: EditingPropertyAccess,
}
