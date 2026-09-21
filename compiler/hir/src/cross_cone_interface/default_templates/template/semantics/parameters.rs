use super::*;

pub(super) fn validate<A: DefaultTemplateRootSemanticAuthority<E>, E>(
    template: &ExportDefaultTemplateV1,
    source: &CallableSourceInterfaceV1,
    shape: DefaultTemplateProviderShapeV1,
    authority: &mut A,
) -> Result<(), ExportDefaultTemplateContractSemanticValidationError<E>> {
    use ExportDefaultTemplateContractSemanticValidationError as Error;
    let provider = authority
        .default_template_provider_parameter(template.definition_root(), template.definition_path())
        .map_err(Error::ProviderParameter)?;
    if provider.position() != template.key().parameter_position() {
        return Err(Error::ProviderParameterPosition);
    }
    if provider.parameters().len_u32() != source.parameters().len_u32() {
        return Err(Error::ProviderParameterArity);
    }
    if template.result() != provider.current().value_type() {
        return Err(Error::ProviderResultMismatch);
    }
    template
        .value_parameters()
        .validate_provider_prefix_semantics(provider, template.locals())
        .map_err(Error::ValueParameters)?;
    for (index, (parameter, published)) in provider
        .parameters()
        .parameters()
        .iter()
        .zip(source.parameters().parameters())
        .enumerate()
    {
        let mapped = template
            .type_parameters()
            .substitute_provider_type(shape, parameter.value_type())
            .map_err(Error::ResultSubstitution)?;
        if &mapped != published.value_type() {
            return Err(Error::ProviderParameterType { index });
        }
    }
    Ok(())
}
