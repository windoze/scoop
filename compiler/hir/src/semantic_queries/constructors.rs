use crate::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticConstructorKind {
    Primary,
    Secondary,
    Variant(scoop_identity::PersistentEnumVariantId),
}

#[derive(Clone, Debug)]
pub struct SemanticConstructorDescription {
    pub identity: scoop_identity::CallableTemplateOrigin,
    pub kind: SemanticConstructorKind,
    pub parameters: Vec<SemanticParameterDescription>,
    pub result: SemanticTypeUse,
    pub access: SemanticMemberAccess,
    pub annotations: SemanticAnnotationTarget,
    pub origin: SemanticSourceLocation,
}
