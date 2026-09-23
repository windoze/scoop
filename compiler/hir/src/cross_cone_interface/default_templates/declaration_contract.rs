//! Shared, borrowed declaration inputs for default parameter validation.

use std::borrow::Cow;

use scoop_identity::{CallableTemplateOrigin, Effect, SignatureTypeKey, StructuralDefinitionPath};

use crate::{
    CanonicalBinderUseListV1, CanonicalBooleanV1, CanonicalTemplateLocalTableV1,
    CanonicalTemplateValueParametersV1, DefaultSourceTemplateV1,
    DefaultTemplateProviderParameterV1, DefaultTemplateProviderShapeV1, ExportDefaultTemplateV1,
    OptionalTemplateReceiverV1, PersistentLexicalRootV1,
};

mod errors;
mod validation;
pub use errors::DefaultTemplateDeclarationContractError;

/// Declaration facts obtained from the actual provider's checked metadata.
/// This is an input view, not a proof of inheritance or access rights.
#[derive(Debug)]
pub struct DefaultTemplateDeclarationContractV1<'a> {
    declaration: CallableTemplateOrigin,
    shape: DefaultTemplateProviderShapeV1,
    parameter: DefaultTemplateProviderParameterV1<'a>,
    default_ordinal: u32,
    receiver: Option<Cow<'a, SignatureTypeKey>>,
    execution: Effect,
}

impl<'a> DefaultTemplateDeclarationContractV1<'a> {
    pub const fn new(
        declaration: CallableTemplateOrigin,
        shape: DefaultTemplateProviderShapeV1,
        parameter: DefaultTemplateProviderParameterV1<'a>,
        default_ordinal: u32,
        receiver: Option<Cow<'a, SignatureTypeKey>>,
        execution: Effect,
    ) -> Self {
        Self {
            declaration,
            shape,
            parameter,
            default_ordinal,
            receiver,
            execution,
        }
    }
}

/// The parameter, receiver and binder fields shared by both source surfaces.
/// Borrowing this view does not validate the default body or its references.
pub struct DefaultTemplateContractViewV1<'a> {
    owner: CallableTemplateOrigin,
    position: u32,
    root: PersistentLexicalRootV1,
    definition_path: &'a StructuralDefinitionPath,
    mapping: &'a CanonicalBinderUseListV1,
    locals: &'a CanonicalTemplateLocalTableV1,
    receiver: &'a OptionalTemplateReceiverV1,
    value_parameters: &'a CanonicalTemplateValueParametersV1,
    result: &'a SignatureTypeKey,
    allows_suspend: CanonicalBooleanV1,
}

macro_rules! contract_view {
    ($($template:ty),+ $(,)?) => { $(
        impl<'a> From<&'a $template> for DefaultTemplateContractViewV1<'a> {
            fn from(template: &'a $template) -> Self {
                Self {
                    owner: template.key().owner(),
                    position: template.key().parameter_position(),
                    root: template.definition_root(),
                    definition_path: template.definition_path(),
                    mapping: template.type_parameters(),
                    locals: template.locals(),
                    receiver: template.receiver(),
                    value_parameters: template.value_parameters(),
                    result: template.result(),
                    allows_suspend: template.allows_suspend(),
                }
            }
        }
    )+ };
}
contract_view!(ExportDefaultTemplateV1, DefaultSourceTemplateV1);
