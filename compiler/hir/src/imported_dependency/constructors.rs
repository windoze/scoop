//! Provider constructors retain their own identity and execution role.

use crate::*;

#[derive(Debug, Clone)]
pub struct ImportedConstructorTemplate {
    pub signature: ImportedConstructorSignature,
    pub kind: ConstructorKind,
}

impl std::ops::Deref for ImportedConstructorTemplate {
    type Target = ImportedConstructorSignature;
    fn deref(&self) -> &Self::Target {
        &self.signature
    }
}

#[derive(Debug, Clone)]
pub struct ImportedConstructorSignature {
    pub declaration: scoop_identity::PersistentConstructorId,
    pub name: String,
    pub type_parameters: Vec<TypeParamDecl>,
    pub owner: TypeId,
    pub parameters: Vec<ConstructorParameter>,
    pub no_gc_type_params: Vec<TypeParamId>,
    pub gc_free_pointee_requirements: Vec<RequiresGcFreePointee>,
    pub effects: CallableSourceEffectsV1,
    pub origin: DefinitionOrigin,
    pub evaluation_context: SourceContextId,
}
