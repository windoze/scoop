use super::*;

type Source<'a> = (
    &'a NominalSourceCallablePayloadV1,
    &'a DeclarationAccessSourceV1,
);

/// Accessors deliberately have no source-parameter protocol entry. Their
/// checked declaration leaf is retained by the separate protected surface.
pub(super) fn find<'a, E>(
    provider: &Exports<'a>,
    id: PersistentPropertyAccessorId,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<Option<Source<'a>>, Error<E>> {
    let mut pending = Vec::new();
    let records = provider.protected.table().records();
    meter.check_table_entries(records.len() as u64, path)?;
    meter.charge_work(records.len() as u64, path)?;
    for record in records {
        match record {
            ProtectedDeclarationInterfaceV1::Callable(value)
                if value.declaration() == CallableTemplateOrigin::Accessor(id) =>
            {
                return Ok(Some((value.payload(), value.declaration_access())));
            }
            ProtectedDeclarationInterfaceV1::NestedNominal(value) => {
                push(&mut pending, value.payload(), 1, meter, path)?;
            }
            _ => {}
        }
    }
    while let Some((payload, depth)) = pending.pop() {
        meter.check_semantic_depth(depth, path)?;
        let records = payload.source_interface().source_support().records();
        meter.check_table_entries(records.len() as u64, path)?;
        meter.charge_work(records.len() as u64, path)?;
        for record in records {
            match record {
                NestedSourceSupportV1::Callable(value)
                    if value.declaration() == CallableTemplateOrigin::Accessor(id) =>
                {
                    return Ok(Some((value.payload(), value.declaration_access())));
                }
                NestedSourceSupportV1::NestedNominal(value) => {
                    push(&mut pending, value.payload(), depth + 1, meter, path)?;
                }
                _ => {}
            }
        }
    }
    Ok(None)
}
fn push<'a>(
    pending: &mut Vec<(&'a ProtectedNestedNominalPayloadV1, u64)>,
    value: &'a ProtectedNestedNominalPayloadV1,
    depth: u64,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), scoop_wire::WireError> {
    meter.try_reserve_collection_slots(pending, 1, path)?;
    pending.push((value, depth));
    Ok(())
}
