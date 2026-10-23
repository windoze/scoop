use crate::{
    PropertyDeclarationId, PropertyId, SemanticSourceLocation, SourceContextId, Span, TypeId,
};

/// A logical property, independently of its storage or accessor implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticProperty {
    Current(PropertyId),
    Imported(PropertyDeclarationId),
}

/// A constant read retained by its original successful evaluator.
#[derive(Clone, Debug)]
pub struct EditingConstantReference {
    pub origin: SemanticSourceLocation,
    pub target: SemanticProperty,
    pub imports: Vec<crate::EditingImportSource>,
}

/// Complete declaration types without evaluating the property's implementation.
#[derive(Clone, Debug)]
pub struct EditingPropertyDeclaration {
    pub property: SemanticProperty,
    pub name: String,
    pub mutable: bool,
    pub value_type: TypeId,
    pub receiver: Option<TypeId>,
    pub binders: crate::SemanticDeclarationBinders,
    pub origin: Option<SemanticSourceLocation>,
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
