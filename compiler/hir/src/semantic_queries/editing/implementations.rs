use crate::{DefaultCallableDeclarationV1, PropertyDeclarationId, SemanticSourceLocation};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditingImplementationMember {
    Callable(DefaultCallableDeclarationV1),
    Property(PropertyDeclarationId),
}

#[derive(Clone, Debug)]
pub struct EditingMemberImplementation {
    pub declaration: EditingImplementationMember,
    pub origin: SemanticSourceLocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditingMemberImplementationRelation {
    pub requirement: EditingImplementationMember,
    pub implementation: EditingImplementationMember,
}

/// Abstract declarations connect relations; only concrete source implementations
/// appear in `implementations`. No callable body is required to read this graph.
#[derive(Clone, Debug, Default)]
pub struct EditingMemberImplementationIndex {
    pub relations: Vec<EditingMemberImplementationRelation>,
    pub implementations: Vec<EditingMemberImplementation>,
    pub unavailable: Vec<SemanticSourceLocation>,
}
