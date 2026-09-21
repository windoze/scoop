use super::*;
use scoop_identity::{Effect, StructuralDefinitionSiteRole};

pub(super) fn validate<'d>(
    owner_sources: &'d BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
    provider_sources: &'d BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
    template: &'d DefaultSourceTemplateV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<DeclarationFacts<'d>, Error> {
    let key = template.key();
    let root = template.definition_root();
    let position = key.parameter_position() as usize;
    let owner = sources::payload(owner_sources, key.owner(), meter, path)?;
    let provider = sources::payload(provider_sources, root.declaration(), meter, path)?;
    let owner_protocol = protocol(owner_sources, key.owner(), position, meter, path)?;
    let provider_protocol = protocol(provider_sources, root.declaration(), position, meter, path)?;
    if owner_protocol.parameters().len() != provider_protocol.parameters().len() {
        return Err(Error::ParameterArity);
    }
    meter.charge_work(
        position as u64 + template.definition_path().segments().len() as u64 + 1,
        path,
    )?;
    let ordinal = provider_protocol.parameters()[..position]
        .iter()
        .filter(|p| has_default(p.calling_kind()))
        .count();
    if !matches!(template.definition_path().segments(), [segment]
        if segment.site_role() == StructuralDefinitionSiteRole::DefaultValue
            && segment.ordinal() as usize == ordinal)
    {
        return Err(Error::DefinitionPath);
    }
    let owner_binders = sources::binders(owner_sources, owner, meter, path)?;
    let provider_binders = sources::binders(provider_sources, provider, meter, path)?;
    let mapping = template.type_parameters();
    if mapping.len_u32() != provider_binders.binder_arity() {
        return Err(Error::MappingArity);
    }
    let scope = owner_binders.signature_scope();
    let mut shapes = sources::Shapes(owner_sources.members().nominals);
    for (index, argument) in mapping.arguments().iter().enumerate() {
        scope.validate_signature_semantics_metered(argument, &mut shapes, meter, path)?;
        if key.owner() == root.declaration()
            && provider_binders.identity_binder_at(index as u32).as_ref() != Some(argument)
        {
            return Err(Error::DirectMapping { index });
        }
    }
    for (index, (source, target)) in provider_protocol
        .parameters()
        .iter()
        .zip(owner_protocol.parameters())
        .enumerate()
    {
        let mapped = mapping.substitute_provider_type_metered(
            provider_binders,
            source.shape().value_type(),
            meter,
            path,
        )?;
        if !same(&mapped, target.shape().value_type(), meter, path)? {
            return Err(Error::ParameterType { index });
        }
    }
    let provider_receiver =
        provider_binders.nominal_source_receiver(root, provider.owner(), meter, path)?;
    template.receiver().validate_provider_semantics_metered(
        provider_receiver.as_ref(),
        template.locals(),
        meter,
        path,
    )?;
    template
        .value_parameters()
        .validate_provider_prefix_types_metered(
            provider_protocol.parameters()[..position]
                .iter()
                .map(|p| p.shape().value_type()),
            template.locals(),
            meter,
            path,
        )?;
    if !same(
        template.result(),
        provider_protocol.parameters()[position]
            .shape()
            .value_type(),
        meter,
        path,
    )? {
        return Err(Error::ResultType);
    }
    let expected = CanonicalBooleanV1::from(provider.effects().execution() == Effect::Suspend);
    if template.allows_suspend() != expected
        || owner.effects().execution() != provider.effects().execution()
    {
        return Err(Error::SuspendPermission);
    }
    super::envelope::validate(template, provider_binders, &mut shapes, meter, path)?;
    let nested_callables = template.index_nested_callables(meter, path)?;
    Ok(DeclarationFacts {
        nested_callables,
        key,
        definition_root: root,
        owner,
        provider,
        owner_binders,
        provider_binders,
        provider_parameter: DefaultTemplateProviderParameterV1::try_new(
            provider.parameters(),
            key.parameter_position(),
        )?,
        provider_receiver,
    })
}
fn protocol<'d>(
    source: &'d BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
    declaration: CallableTemplateOrigin,
    position: usize,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<&'d NominalSourceParameterProtocolV1, Error> {
    sources::query(source.table().records().len(), meter, path)?;
    let protocol = source.protocol(declaration)?;
    if !protocol
        .parameters()
        .get(position)
        .is_some_and(|p| has_default(p.calling_kind()))
    {
        return Err(Error::DefaultParameter {
            declaration,
            position,
        });
    }
    Ok(protocol)
}
fn has_default(kind: ProtectedParameterCallingKindV1) -> bool {
    matches!(
        kind,
        ProtectedParameterCallingKindV1::Default | ProtectedParameterCallingKindV1::VarargDefault
    )
}
fn same(
    left: &SignatureTypeKey,
    right: &SignatureTypeKey,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<bool, Error> {
    Ok(compare_default_signature_reference_targets(left, right, meter, path)?.is_eq())
}
