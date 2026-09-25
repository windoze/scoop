use super::*;

pub(super) fn selected<'a>(
    selection: &'a impl MaterializationSelection<'a>,
    provider: ConeIdentity,
    source: scoop_identity::PersistentTypeId,
    meter: &mut BudgetMeter,
) -> Result<Option<&'a crate::ParamFreeShapeSupportExportV1>, scoop_wire::WireError> {
    meter.charge_work(selection.semantic_count() as u64, &WirePath::root())?;
    Ok(
        match selection.semantic_record(provider, LayoutAbiSemanticTargetV1::ShapeSupport(source)) {
            Some(LayoutAbiSemanticRecordV1::ShapeSupport(shape)) => Some(shape),
            _ => None,
        },
    )
}

pub(super) fn materialize<'a>(
    selection: &'a impl MaterializationSelection<'a>,
    provider: ConeIdentity,
    source: scoop_identity::PersistentTypeId,
    exact: PersistentExactTypeId,
    meter: &mut BudgetMeter,
) -> Result<ExternalTypeDescriptor, LayoutExternalMaterializationError> {
    use LayoutExternalMaterializationError as Error;
    validate_provider(selection, provider)?;
    let shape = selected(selection, provider, source, meter)?
        .ok_or(Error::MissingShapeSupport { provider, source })?;
    let roles = shape.roles();
    let definition = if exact == shape.exact() {
        roles.type_descriptor().available().copied()
    } else {
        [
            roles.boxed_value(),
            roles.coroutine_step(),
            roles.coroutine_slot(),
        ]
        .into_iter()
        .filter_map(|role| role.available())
        .find(|helper| helper.exact() == exact)
        .map(|helper| helper.descriptor())
    }
    .ok_or(Error::UnavailableShapeDescriptor { source, exact })?;
    let descriptor = materialize_type_descriptor(selection, provider, exact, meter)?;
    if definition.semantic_id() != exact
        || definition.symbol() != descriptor.expected_symbol()
        || definition.definition_plan() != descriptor.required_definition()
    {
        return Err(Error::DescriptorContract(exact));
    }
    Ok(descriptor)
}
