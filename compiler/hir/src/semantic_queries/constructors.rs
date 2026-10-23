use crate::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SemanticConstructor {
    CurrentStruct(StructConstructorId),
    CurrentClass(ClassConstructorId),
    Imported(scoop_identity::PersistentConstructorId),
}

/// Constructor execution uses the same portable fragments as published templates.
#[derive(Clone, Debug)]
pub struct SemanticConstructorBody {
    pub declaration: std::sync::Arc<ExportConstructorInitializationV1>,
    pub common: std::sync::Arc<[ExportCommonInitializationStepV1]>,
}

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
