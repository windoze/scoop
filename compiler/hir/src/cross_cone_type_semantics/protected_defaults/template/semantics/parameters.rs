use super::*;
use crate::{ProtectedSourceParameterV1, compare_default_signature_reference_targets};
use scoop_wire::WirePath;

pub(super) fn validate<A: ProtectedDefaultRootSemanticAuthority<E>, E>(
    template: &ProtectedDefaultTemplateV1,
    published: &[ProtectedSourceParameterV1],
    shape: DefaultTemplateProviderShapeV1,
    authority: &mut A,
) -> Result<(), ProtectedDefaultTemplateContractSemanticError<E>> {
    use ProtectedDefaultTemplateContractSemanticError as Error;
    let path = WirePath::root();
    let provider = authority
        .protected_default_provider_parameter(
            template.definition_root(),
            template.definition_path(),
        )
        .map_err(Error::Foundation)?;

    if provider.position() != template.key().parameter_position() {
        return Err(Error::ProviderParameterPosition);
    }
    if provider.parameters().parameters().len() != published.len() {
        return Err(Error::ProviderParameterArity);
    }
    if !compare_default_signature_reference_targets(
        template.result(),
        provider.current().value_type(),
        &path,
    )
    .map_err(Error::Resource)?
    .is_eq()
    {
        return Err(Error::ProviderResultMismatch);
    }
    template
        .value_parameters()
        .validate_provider_prefix_types(
            provider.prefix().iter().map(|p| p.value_type()),
            template.locals(),
        )
        .map_err(Error::ValueParameters)?;
    for (index, (parameter, published)) in provider
        .parameters()
        .parameters()
        .iter()
        .zip(published)
        .enumerate()
    {
        let mapped = template
            .type_parameters()
            .substitute_provider_type(shape, parameter.value_type())
            .map_err(Error::ResultSubstitution)?;
        if !compare_default_signature_reference_targets(&mapped, published.value_type(), &path)
            .map_err(Error::Resource)?
            .is_eq()
        {
            return Err(Error::ProviderParameterType { index });
        }
    }
    Ok(())
}
