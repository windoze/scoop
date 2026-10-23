use crate::{SemanticSourceLocation, SourceNominalId, TypeParamId};
use scoop_identity::{PersistentAnnotationId, PersistentTypeAliasId};

/// The complete result of resolving the type syntax at this source name.
#[derive(Clone, Debug)]
pub struct EditingTypeApplication {
    pub origin: SemanticSourceLocation,
    pub ty: crate::TypeId,
    /// Formatted while the original declaration binders are still in scope.
    pub name: String,
}

/// A source-bound declaration and, when present, its checked application.
#[derive(Clone, Debug)]
pub struct EditingTypeDescription {
    pub target: EditingTypeIdentity,
    pub label: String,
    pub application: Option<EditingTypeApplication>,
    pub documentation: Option<crate::DocumentationSymbol>,
    pub definition: Option<SemanticSourceLocation>,
}

/// One source declaration with a checked nominal inheritance relationship.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EditingTypeImplementation {
    pub declaration: SourceNominalId,
    pub origin: SemanticSourceLocation,
}

/// Declaration identity survives transparent expansion and nominal application.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditingTypeIdentity {
    Nominal(SourceNominalId),
    Alias(PersistentTypeAliasId),
    Annotation(PersistentAnnotationId),
    Parameter(TypeParamId),
}

#[derive(Clone, Debug)]
pub struct EditingTypeReference {
    pub target: EditingTypeIdentity,
    pub origin: SemanticSourceLocation,
    pub definition: Option<SemanticSourceLocation>,
    pub is_declaration: bool,
    /// Exact imports selected by ordinary type lookup for this occurrence.
    pub imports: Vec<crate::EditingImportSource>,
}
