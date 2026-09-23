use super::*;

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
    let owner_binders = sources::binders(owner_sources, owner, meter, path)?;
    let provider_binders = sources::binders(provider_sources, provider, meter, path)?;
    let owner_root = PersistentLexicalRootV1::try_from(key.owner())
        .map_err(|_| Error::Declaration(key.owner()))?;
    let owner_receiver =
        owner_binders.nominal_source_receiver(owner_root, owner.owner(), meter, path)?;
    let provider_receiver =
        provider_binders.nominal_source_receiver(root, provider.owner(), meter, path)?;
    let provider_parameter = DefaultTemplateProviderParameterV1::try_new(
        provider.parameters(),
        key.parameter_position(),
    )?;
    let owner_contract = DefaultTemplateDeclarationContractV1::new(
        key.owner(),
        owner_binders,
        DefaultTemplateProviderParameterV1::try_new(owner.parameters(), key.parameter_position())?,
        default_ordinal(owner_protocol, position, meter, path)?,
        owner_receiver.as_ref().map(std::borrow::Cow::Borrowed),
        owner.effects().execution(),
    );
    let provider_contract = DefaultTemplateDeclarationContractV1::new(
        root.declaration(),
        provider_binders,
        provider_parameter,
        default_ordinal(provider_protocol, position, meter, path)?,
        provider_receiver.as_ref().map(std::borrow::Cow::Borrowed),
        provider.effects().execution(),
    );
    let mut shapes = sources::Shapes(owner_sources.members().nominals);
    DefaultTemplateContractViewV1::from(template).validate(
        &owner_contract,
        &provider_contract,
        &mut shapes,
        meter,
        path,
    )?;
    DefaultTemplateContractViewV1::from(template).validate_provider_types(
        provider_binders,
        &mut shapes,
        meter,
        path,
    )?;
    let nested_callables = template.index_nested_callables(meter, path)?;
    Ok(DeclarationFacts {
        nested_callables,
        key,
        definition_root: root,
        owner,
        provider,
        owner_binders,
        provider_binders,
        provider_parameter,
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
fn default_ordinal(
    protocol: &NominalSourceParameterProtocolV1,
    position: usize,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<u32, Error> {
    meter.charge_work(position as u64 + 1, path)?;
    Ok(protocol.parameters()[..position]
        .iter()
        .filter(|parameter| has_default(parameter.calling_kind()))
        .count() as u32)
}
