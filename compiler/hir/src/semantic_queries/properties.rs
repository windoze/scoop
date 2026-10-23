use crate::*;

#[derive(Clone, Debug)]
pub struct SemanticMemberAccess {
    pub visibility: DeclaredVisibilityV1,
    pub domain: AccessDomain,
    /// Includes the ordinary protected-receiver restriction in this context.
    pub accessible: bool,
}

#[derive(Clone, Debug)]
pub struct SemanticAccessorDescription {
    pub identity: scoop_identity::PersistentPropertyAccessorId,
    pub callable: SemanticCallable,
    pub access: SemanticMemberAccess,
    pub implementation: PropertyAccessorImplementationV1,
    pub annotations: SemanticAnnotationTarget,
    pub origin: SemanticSourceLocation,
}

#[derive(Clone, Debug)]
pub struct SemanticPropertyDescription {
    pub identity: scoop_identity::PersistentPropertyId,
    pub name: String,
    pub value_type: SemanticTypeUse,
    pub visibility: DeclaredVisibilityV1,
    pub lookup_domain: AccessDomain,
    pub getter: SemanticAccessorDescription,
    pub setter: Option<SemanticAccessorDescription>,
    pub overrides: Vec<scoop_identity::PersistentPropertyId>,
    pub annotations: SemanticAnnotationTarget,
    pub origin: SemanticSourceLocation,
}
