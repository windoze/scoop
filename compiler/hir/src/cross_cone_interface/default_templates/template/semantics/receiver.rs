use super::*;
use crate::{PublicDeclarationOwnerV1, SourceNominalId};

pub(super) fn validate_direct<E>(
    template: &ExportDefaultTemplateV1,
    provider: DefaultTemplateProviderShapeV1,
    callable: &CallableInterfaceRecordV1,
    outer_arity: u32,
    provider_receiver: Option<&SignatureTypeKey>,
) -> Result<(), ExportDefaultTemplateContractSemanticValidationError<E>> {
    use ExportDefaultTemplateContractSemanticValidationError as Error;
    if provider.nominal_owner_binder_arity() != outer_arity
        || provider.callable_own_binder_arity() != callable.type_parameters().len_u32()
    {
        return Err(Error::ProviderOwnerBinders);
    }
    if !matches_owner_receiver(callable, provider, provider_receiver) {
        return Err(Error::ProviderOwnerReceiver);
    }
    for (index, argument) in template.type_parameters().arguments().iter().enumerate() {
        if provider.identity_binder_at(index as u32).as_ref() != Some(argument) {
            return Err(Error::DirectMapping { index });
        }
    }
    Ok(())
}

fn matches_owner_receiver(
    callable: &CallableInterfaceRecordV1,
    provider: DefaultTemplateProviderShapeV1,
    receiver: Option<&SignatureTypeKey>,
) -> bool {
    match callable.declaration() {
        CallableTemplateOrigin::Constructor(_) | CallableTemplateOrigin::VariantConstructor(_) => {
            receiver.is_none()
        }
        CallableTemplateOrigin::Accessor(_) => false,
        CallableTemplateOrigin::Function(_) | CallableTemplateOrigin::GenericFunction(_) => {
            match (callable.owner(), receiver) {
                (PublicDeclarationOwnerV1::TopLevel, None) => true,
                (PublicDeclarationOwnerV1::Extension, _) => callable.receiver() == receiver,
                (
                    PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(owner)),
                    Some(SignatureTypeKey::Nominal(actual)),
                ) => owner == *actual,
                (
                    PublicDeclarationOwnerV1::Nominal(SourceNominalId::GenericTemplate(owner)),
                    Some(SignatureTypeKey::NominalApplication { origin, arguments }),
                ) => {
                    owner == *origin
                        && arguments.as_slice().len()
                            == provider.nominal_owner_binder_arity() as usize
                        && arguments
                            .as_slice()
                            .iter()
                            .enumerate()
                            .all(|(index, argument)| {
                                provider.identity_binder_at(index as u32).as_ref() == Some(argument)
                            })
                }
                _ => false,
            }
        }
    }
}
