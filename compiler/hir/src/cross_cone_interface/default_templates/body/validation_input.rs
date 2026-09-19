use crate::{
    CanonicalBooleanV1, CanonicalTemplateLocalTableV1, CanonicalTemplateValueParametersV1,
    ExportDefaultBodyV1, ExportDefaultTemplateV1, OptionalTemplateReceiverV1,
};
use scoop_identity::{SignatureTypeKey, StructuralDefinitionPath};

mod authorities;
pub(crate) use authorities::*;

/// Borrowed semantic leaves shared by the body validators. This projection
/// contains no lookup authority, default key or reference transport schema.
#[derive(Clone, Copy)]
pub(crate) struct DefaultBodyValidationInputV1<'a> {
    body: &'a ExportDefaultBodyV1,
    locals: &'a CanonicalTemplateLocalTableV1,
    receiver: &'a OptionalTemplateReceiverV1,
    value_parameters: &'a CanonicalTemplateValueParametersV1,
    result: &'a SignatureTypeKey,
    allows_suspend: CanonicalBooleanV1,
    definition_path: &'a StructuralDefinitionPath,
}
impl<'a> DefaultBodyValidationInputV1<'a> {
    pub(crate) const fn new(
        body: &'a ExportDefaultBodyV1,
        locals: &'a CanonicalTemplateLocalTableV1,
        receiver: &'a OptionalTemplateReceiverV1,
        value_parameters: &'a CanonicalTemplateValueParametersV1,
        result: &'a SignatureTypeKey,
        allows_suspend: CanonicalBooleanV1,
        definition_path: &'a StructuralDefinitionPath,
    ) -> Self {
        Self {
            body,
            locals,
            receiver,
            value_parameters,
            result,
            allows_suspend,
            definition_path,
        }
    }
    pub(crate) const fn body(self) -> &'a ExportDefaultBodyV1 {
        self.body
    }
    pub(crate) const fn locals(self) -> &'a CanonicalTemplateLocalTableV1 {
        self.locals
    }
    pub(crate) const fn receiver(self) -> &'a OptionalTemplateReceiverV1 {
        self.receiver
    }
    pub(crate) const fn value_parameters(self) -> &'a CanonicalTemplateValueParametersV1 {
        self.value_parameters
    }
    pub(crate) const fn result(self) -> &'a SignatureTypeKey {
        self.result
    }
    pub(crate) const fn allows_suspend(self) -> CanonicalBooleanV1 {
        self.allows_suspend
    }
    pub(crate) const fn definition_path(self) -> &'a StructuralDefinitionPath {
        self.definition_path
    }
}
impl<'a> From<&'a ExportDefaultTemplateV1> for DefaultBodyValidationInputV1<'a> {
    fn from(template: &'a ExportDefaultTemplateV1) -> Self {
        Self::new(
            template.body(),
            template.locals(),
            template.receiver(),
            template.value_parameters(),
            template.result(),
            template.allows_suspend(),
            template.definition_path(),
        )
    }
}
