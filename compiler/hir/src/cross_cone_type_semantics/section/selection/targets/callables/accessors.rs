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

    path: &WirePath,
) -> Result<Option<Source<'a>>, Error<E>> {
    let mut pending = Vec::new();
    let records = provider.protected.table().records();

    for record in records {
        match record {
            ProtectedDeclarationInterfaceV1::Callable(value)
                if value.declaration() == CallableTemplateOrigin::Accessor(id) =>
            {
                return Ok(Some((value.payload(), value.declaration_access())));
            }
            ProtectedDeclarationInterfaceV1::NestedNominal(value) => {
                push(&mut pending, value.payload(), path)?;
            }
            _ => {}
        }
    }
    while let Some(payload) = pending.pop() {
        let records = payload.source_interface().source_support().records();

        for record in records {
            match record {
                NestedSourceSupportV1::Callable(value)
                    if value.declaration() == CallableTemplateOrigin::Accessor(id) =>
                {
                    return Ok(Some((value.payload(), value.declaration_access())));
                }
                NestedSourceSupportV1::NestedNominal(value) => {
                    push(&mut pending, value.payload(), path)?;
                }
                _ => {}
            }
        }
    }
    Ok(None)
}
fn push<'a>(
    pending: &mut Vec<&'a ProtectedNestedNominalPayloadV1>,
    value: &'a ProtectedNestedNominalPayloadV1,
    path: &WirePath,
) -> Result<(), scoop_wire::WireError> {
    scoop_wire::allocation::try_reserve(pending, 1, path)?;
    pending.push(value);
    Ok(())
}
