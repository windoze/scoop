use super::*;
use crate::compare_default_signature_reference_targets;
use scoop_identity::SignatureTypeKey;
use scoop_wire::WirePath;

pub(super) fn validate_direct<E>(
    template: &ProtectedDefaultTemplateV1,
    provider: DefaultTemplateProviderShapeV1,
    owner_receiver: Option<&SignatureTypeKey>,
    provider_receiver: Option<&SignatureTypeKey>,
) -> Result<(), ProtectedDefaultTemplateContractSemanticError<E>> {
    use ProtectedDefaultTemplateContractSemanticError as Error;
    let path = WirePath::root();

    let matches = match (owner_receiver, provider_receiver) {
        (Some(owner), Some(provider)) => {
            compare_default_signature_reference_targets(owner, provider, &path)
                .map_err(Error::Resource)?
                .is_eq()
        }
        (None, None) => true,
        _ => false,
    };
    if !matches {
        return Err(Error::ProviderOwnerReceiver);
    }
    for (index, argument) in template.type_parameters().arguments().iter().enumerate() {
        if provider.identity_binder_at(index as u32).as_ref() != Some(argument) {
            return Err(Error::DirectMapping { index });
        }
    }
    Ok(())
}
