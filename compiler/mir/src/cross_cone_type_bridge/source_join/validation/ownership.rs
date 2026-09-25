use super::*;
use scoop_identity::{
    GeneratedCallableKey, InitializationUnitKey, PropertyAccessorKey, PropertyOwner,
};

pub(super) fn type_record<E>(
    record: &ParamFreeMirTypeExportV1,
    expected: ConeIdentity,
    graph: &ValidatedIdentityGraph,
) -> Result<(), MirTypeBridgeSourceJoinError<E>> {
    use MirTypeBridgeSourceJoinError as Error;
    let subject = MirTypeBridgeSourceRecordV1::Type(record.exact());

    let owner = match record.origin() {
        MirTypeOriginV1::SourceNominal(nominal) => nominal_owner(*nominal, graph)?,
        MirTypeOriginV1::GeneratedNominal { role, .. } => match role {
            GeneratedNominalKey::ObjectBackingClass { object } => nominal_owner(*object, graph)?,
            GeneratedNominalKey::BoxedValue { payload } => exact_owner(*payload, subject, graph)?,
            GeneratedNominalKey::CoroutineStep { result } => exact_owner(*result, subject, graph)?,
            GeneratedNominalKey::CoroutineSlot { value } => exact_owner(*value, subject, graph)?,
            _ => return Err(Error::Ownership(subject)),
        },
    };
    if owner != expected {
        return Err(Error::Ownership(subject));
    }
    Ok(())
}

pub(super) fn callable<E>(
    record: &ParamFreeMirCallableBindingV1,
    expected: ConeIdentity,
    graph: &ValidatedIdentityGraph,
) -> Result<(), MirTypeBridgeSourceJoinError<E>> {
    use MirTypeBridgeSourceJoinError as Error;
    let subject = MirTypeBridgeSourceRecordV1::Callable(record.implementation());

    let owner = match record.origin() {
        MirCallableOriginV1::Function(id) => graph
            .canonical_key::<_, SourceDeclarationKey>(*id)
            .map_err(Error::Reference)?
            .origin(),
        MirCallableOriginV1::Constructor(id) => graph
            .canonical_key::<_, SourceDeclarationKey>(*id)
            .map_err(Error::Reference)?
            .origin(),
        MirCallableOriginV1::Accessor(id) => {
            let key = graph
                .canonical_key::<_, PropertyAccessorKey>(*id)
                .map_err(Error::Reference)?;
            property_owner(key.owner(), graph)?
        }
        MirCallableOriginV1::Generated { role, .. } => match role {
            GeneratedCallableKey::DispatchAdjust { implementor, .. } => {
                exact_owner(*implementor, subject, graph)?
            }
            GeneratedCallableKey::BoxingAdjust { payload, .. } => {
                exact_owner(*payload, subject, graph)?
            }
            GeneratedCallableKey::DerivedEquality { exact_owner: exact } => {
                exact_owner(*exact, subject, graph)?
            }
            GeneratedCallableKey::Initialization { unit, .. } => {
                let key = graph
                    .canonical_key::<_, InitializationUnitKey>(*unit)
                    .map_err(Error::Reference)?;
                match key.as_ref() {
                    InitializationUnitKey::TopLevelProperty(id) => {
                        property_owner(PropertyOwner::Property(*id), graph)?
                    }
                    InitializationUnitKey::ExtensionProperty(id) => {
                        property_owner(PropertyOwner::ExtensionProperty(*id), graph)?
                    }
                    InitializationUnitKey::Object(id) | InitializationUnitKey::Companion(id) => {
                        nominal_owner(*id, graph)?
                    }
                    InitializationUnitKey::GenericDelegatedExtensionApplication { .. } => {
                        return Err(Error::Ownership(subject));
                    }
                }
            }
            _ => return Err(Error::Ownership(subject)),
        },
    };
    if owner != expected {
        return Err(Error::Ownership(subject));
    }
    Ok(())
}

fn nominal_owner<E>(
    nominal: PersistentTypeId,
    graph: &ValidatedIdentityGraph,
) -> Result<ConeIdentity, MirTypeBridgeSourceJoinError<E>> {
    graph
        .canonical_key::<_, SourceDeclarationKey>(nominal)
        .map(|key| key.origin())
        .map_err(MirTypeBridgeSourceJoinError::Reference)
}
fn exact_owner<E>(
    exact: PersistentExactTypeId,
    subject: MirTypeBridgeSourceRecordV1,
    graph: &ValidatedIdentityGraph,
) -> Result<ConeIdentity, MirTypeBridgeSourceJoinError<E>> {
    use MirTypeBridgeSourceJoinError as Error;
    let key = graph
        .canonical_key::<_, ExactTypeKey>(exact)
        .map_err(Error::Reference)?;
    match key.as_ref() {
        ExactTypeKey::Nominal(nominal) => nominal_owner(*nominal, graph),
        _ => Err(Error::Ownership(subject)),
    }
}
fn property_owner<E>(
    owner: PropertyOwner,
    graph: &ValidatedIdentityGraph,
) -> Result<ConeIdentity, MirTypeBridgeSourceJoinError<E>> {
    use MirTypeBridgeSourceJoinError as Error;
    Ok(match owner {
        PropertyOwner::Property(id) => graph
            .canonical_key::<_, SourceDeclarationKey>(id)
            .map_err(Error::Reference)?
            .origin(),
        PropertyOwner::ExtensionProperty(id) => graph
            .canonical_key::<_, SourceDeclarationKey>(id)
            .map_err(Error::Reference)?
            .origin(),
    })
}
