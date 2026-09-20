use super::*;
use scoop_identity::{SignatureTypeKey, StructuralDefinitionPath};

impl DefaultSourceBodyProductionV1<'_> {
    pub const fn owner(&self) -> CallableTemplateOrigin {
        self.owner
    }
    pub const fn parameter_position(&self) -> u32 {
        self.parameter_position
    }
    pub const fn definition_root(&self) -> PersistentLexicalRootV1 {
        self.projected.root
    }
    pub fn definition_path(&self) -> &StructuralDefinitionPath {
        &self.projected.path
    }
    pub fn locals(&self) -> &CanonicalTemplateLocalTableV1 {
        &self.projected.locals
    }
    pub fn body(&self) -> &ExportDefaultBodyV1 {
        &self.projected.body
    }
    pub fn result(&self) -> &SignatureTypeKey {
        &self.projected.result
    }
    pub const fn allows_suspend(&self) -> CanonicalBooleanV1 {
        self.projected.allows_suspend
    }
    pub fn type_parameters(&self) -> &CanonicalBinderUseListV1 {
        &self.projected.type_parameters
    }
    pub fn receiver(&self) -> &OptionalTemplateReceiverV1 {
        &self.projected.receiver
    }
    pub fn value_parameters(&self) -> &CanonicalTemplateValueParametersV1 {
        &self.projected.value_parameters
    }
    pub fn definition_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.projected.definition_origin
    }
    pub fn provider_binders(&self) -> &[HirSignatureBinder] {
        &self.projected.provider_binders
    }
    pub fn source_references(&self) -> &ExportDefaultReferences {
        self.projected.references
    }
}
