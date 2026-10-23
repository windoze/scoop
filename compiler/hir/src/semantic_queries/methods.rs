use crate::*;
use scoop_identity::CallableTemplateOrigin;

#[derive(Clone, Debug)]
pub struct SemanticTypeParameterUse {
    pub declaration: TypeParamDecl,
    pub applied: TypeParamDecl,
}

#[derive(Clone, Debug)]
pub struct SemanticContextParameterDescription {
    pub label: ContextParameterLabel,
    pub value_type: SemanticTypeUse,
}

/// A directly declared method; inherited lookup uses ordinary member selection.
#[derive(Clone, Debug)]
pub struct SemanticMethodDescription {
    pub identity: CallableTemplateOrigin,
    pub callable: SemanticCallable,
    pub name: String,
    pub receiver: SemanticTypeUse,
    pub owner_parameters: Vec<TypeParamDecl>,
    pub type_parameters: Vec<SemanticTypeParameterUse>,
    pub context_parameters: Vec<SemanticContextParameterDescription>,
    pub parameters: Vec<SemanticParameterDescription>,
    pub result: SemanticTypeUse,
    pub is_suspend: bool,
    pub modifiers: CallableModifiers,
    pub attributes: FunctionAttributes,
    pub release_callability: ReleaseCallability,
    pub modality: CallableModalityV1,
    pub access: SemanticMemberAccess,
    pub annotations: SemanticAnnotationTarget,
    pub origin: SemanticSourceLocation,
}
