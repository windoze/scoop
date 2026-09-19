use super::*;

pub(super) fn exports<E>(
    authority: MirTypeBridgeLocalAuthorityV1<'_>,
    exports: &MirTypeBridgeExportConstituentsV1,
    dependencies: &[&CrossConeMirTypeBridgeSectionV1<'_>],
    graph: &ValidatedIdentityGraph,
    types: &dyn MirTypeBridgeTypeLookupV1,
    meter: &mut BudgetMeter,
) -> Result<(), MirTypeBridgeSectionError<E>> {
    let type_authority = MirTypeBridgeAuthority {
        identities: graph,
        foundation: authority.foundation(),
    };
    for record in exports.types().records() {
        charge_record::<E>(record, meter)?;
        type_authority.validate(record)?;
    }
    let callable_authority = MirCallableBridgeAuthority {
        identities: graph,
        foundation: authority.foundation(),
        types,
    };
    for record in exports.callables().entries() {
        charge_record::<E>(record, meter)?;
        callable_authority.validate(record)?;
    }
    let count = dependencies
        .len()
        .checked_add(1)
        .ok_or(MirTypeBridgeSectionError::ArithmeticOverflow)?;
    let mut callable_tables = reserve(count, meter)?;
    callable_tables.push(exports.callables());
    callable_tables.extend(dependencies.iter().map(|section| section.callables()));
    let callables = MirTypeBridgeCallableIndexV1::try_new(&callable_tables, meter)?;
    let mut schemas = reserve(dependencies.len(), meter)?;
    schemas.extend(dependencies.iter().map(|section| section.dispatch()));
    MirDispatchSchemaAuthority {
        identities: graph,
        types,
        callables: &callables,
    }
    .validate_with_dependencies(exports.dispatch(), &schemas, meter)?;
    let object_authority = MirObjectBridgeAuthority {
        identities: graph,
        types,
        callables: &callables,
    };
    for record in exports.objects().records() {
        object_authority.validate_object(
            record.value(),
            record.backing(),
            record.unit(),
            record.ensure(),
            record.read(),
            meter,
        )?;
    }
    for record in exports.initialization_uses().records() {
        SelectedExternalInitializationUseV1::try_new(
            authority.provider(),
            graph,
            record.local_unit(),
            record.provider(),
            record.dependency_unit(),
            record.cause(),
            meter,
        )?;
    }
    Ok(())
}
fn charge_record<E>(
    record: &impl WireEncode,
    meter: &mut BudgetMeter,
) -> Result<(), MirTypeBridgeSectionError<E>> {
    meter.charge_work(
        scoop_wire::encoded_length(record).map_err(MirTypeBridgeSectionError::Encoding)?,
        &WirePath::root(),
    )?;
    Ok(())
}
