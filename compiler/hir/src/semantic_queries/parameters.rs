use crate::*;

/// A source parameter is identified within its actual callable declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticParameterIdentity {
    pub owner: scoop_identity::CallableTemplateOrigin,
    pub index: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticDefaultReference {
    Current(ExportDefaultSourceId),
    Imported(ExportDefaultTemplateKeyV1),
}

#[derive(Clone, Debug)]
pub struct SemanticDefaultUse {
    pub definition: SemanticDefaultReference,
    /// Ordered definition-binder arguments in the queried application.
    pub arguments: Vec<TypeId>,
}

#[derive(Clone, Debug)]
pub enum SemanticVarargOmission {
    EmptyArray,
    Default(SemanticDefaultUse),
}

#[derive(Clone, Debug)]
pub enum SemanticParameterCalling {
    Required,
    Default(SemanticDefaultUse),
    Vararg {
        element_type: SemanticTypeUse,
        omission: SemanticVarargOmission,
    },
}

#[derive(Clone, Debug)]
pub struct SemanticParameterDescription {
    pub identity: SemanticParameterIdentity,
    pub name: String,
    pub value_type: SemanticTypeUse,
    pub calling: SemanticParameterCalling,
    pub target: StaticParameterTarget,
    pub annotations: SemanticAnnotationTarget,
    pub origin: SemanticSourceLocation,
}
