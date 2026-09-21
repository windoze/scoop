use scoop_identity::{SignatureTypeKey, StructuralDefinitionPath};

use crate::DefaultSourceReferencesV1;
use crate::{
    CanonicalBinderUseListV1, CanonicalBooleanV1, CanonicalTemplateLocalTableV1,
    CanonicalTemplateValueParametersV1, ExportDefaultBodyV1, ExportDefinitionSourceV1,
    OptionalTemplateReceiverV1, PersistentLexicalRootV1, ProtectedDefaultTemplateKeyV1,
};

mod decode;
mod errors;
mod indexed;
mod origins;
mod resources;
mod validation;
pub use decode::*;
pub use errors::*;
pub use indexed::*;
pub use origins::*;

/// Independent source body and occurrences. Decoding grants no default authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultSourceTemplateV1 {
    key: ProtectedDefaultTemplateKeyV1,
    definition_root: PersistentLexicalRootV1,
    definition_path: StructuralDefinitionPath,
    locals: CanonicalTemplateLocalTableV1,
    body: ExportDefaultBodyV1,
    result: SignatureTypeKey,
    allows_suspend: CanonicalBooleanV1,
    type_parameters: CanonicalBinderUseListV1,
    receiver: OptionalTemplateReceiverV1,
    value_parameters: CanonicalTemplateValueParametersV1,
    references: DefaultSourceReferencesV1,
    definition_origin: ExportDefinitionSourceV1,
}
impl DefaultSourceTemplateV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        key: ProtectedDefaultTemplateKeyV1,
        definition_root: PersistentLexicalRootV1,
        definition_path: StructuralDefinitionPath,
        locals: CanonicalTemplateLocalTableV1,
        body: ExportDefaultBodyV1,
        result: SignatureTypeKey,
        allows_suspend: CanonicalBooleanV1,
        type_parameters: CanonicalBinderUseListV1,
        receiver: OptionalTemplateReceiverV1,
        value_parameters: CanonicalTemplateValueParametersV1,
        references: DefaultSourceReferencesV1,
        definition_origin: ExportDefinitionSourceV1,
        meter: &mut scoop_wire::BudgetMeter,
    ) -> Result<Self, DefaultSourceTemplateBuildError> {
        let template = Self {
            key,
            definition_root,
            definition_path,
            locals,
            body,
            result,
            allows_suspend,
            type_parameters,
            receiver,
            value_parameters,
            references,
            definition_origin,
        };
        template.validate_shape(meter)?;
        template
            .index_locals(meter)
            .map_err(DefaultSourceTemplateBuildError::Index)?;
        Ok(template)
    }

    pub const fn key(&self) -> ProtectedDefaultTemplateKeyV1 {
        self.key
    }
    pub const fn definition_root(&self) -> PersistentLexicalRootV1 {
        self.definition_root
    }
    pub const fn definition_path(&self) -> &StructuralDefinitionPath {
        &self.definition_path
    }
    pub const fn locals(&self) -> &CanonicalTemplateLocalTableV1 {
        &self.locals
    }
    pub const fn body(&self) -> &ExportDefaultBodyV1 {
        &self.body
    }
    pub const fn result(&self) -> &SignatureTypeKey {
        &self.result
    }
    pub const fn allows_suspend(&self) -> CanonicalBooleanV1 {
        self.allows_suspend
    }
    pub const fn type_parameters(&self) -> &CanonicalBinderUseListV1 {
        &self.type_parameters
    }
    pub const fn receiver(&self) -> &OptionalTemplateReceiverV1 {
        &self.receiver
    }
    pub const fn value_parameters(&self) -> &CanonicalTemplateValueParametersV1 {
        &self.value_parameters
    }
    pub const fn references(&self) -> &DefaultSourceReferencesV1 {
        &self.references
    }
    pub const fn definition_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.definition_origin
    }
}
