use crate::{SemanticSourceLocation, SourceNominalId, TypeParamId};
use scoop_identity::{PersistentAnnotationId, PersistentTypeAliasId};

/// The complete result of resolving the type syntax at this source name.
#[derive(Clone, Debug)]
pub struct EditingTypeApplication {
    pub origin: SemanticSourceLocation,
    pub ty: crate::TypeId,
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
    pub imports: Vec<SemanticSourceLocation>,
}
