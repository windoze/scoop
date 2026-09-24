use super::*;
use scoop_identity::{AccessorRole, PropertyOwner};

pub(super) fn collect<'a>(
    data: &mut Data<'a>,
    schemas: &mut SchemaDeclarations<'_>,
    provider: CheckedSharedTypeFoundationV1<'a>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let metadata = provider.metadata;
    for source in metadata.public.callable_interfaces().all_declarations() {
        meter.charge_work(1, &WirePath::root())?;
        let Some(SourceNominalId::Concrete(owner)) = source.owner().nominal_owner() else {
            continue;
        };
        if source.declared_visibility() == DeclaredVisibilityV1::Private
            || !source.type_parameters().is_empty()
            || !matches!(
                source.declaration(),
                CallableTemplateOrigin::Function(_) | CallableTemplateOrigin::Accessor(_)
            )
        {
            continue;
        }
        let key = ExactTypeKey::Nominal(owner);
        meter.charge_sha256(
            PersistentExactTypeId::hash_stream_length(&key)
                .map_err(|e| Error::Key(e.to_string()))?,
            &WirePath::root(),
        )?;
        let owner = PersistentExactTypeId::from_key(&key).map_err(|e| Error::Key(e.to_string()))?;
        contracts::lookup(schemas.orders.len(), meter)?;
        if !schemas.orders.contains_key(&owner) {
            continue;
        }
        let declaration = identity(schemas, metadata, source.declaration(), meter)?;
        let access = contracts::callable_access(metadata, source, meter)?;
        let length = scoop_wire::encoded_length(access.definition_origin())
            .map_err(|e| Error::Key(e.to_string()))?;
        meter.charge_owned_bytes(length, &WirePath::root())?;
        meter.charge_collection_slots(2, &WirePath::root())?;
        meter.charge_work(
            (length + 1).saturating_mul(1 + u64::from(data.members.len().max(1).ilog2())),
            &WirePath::root(),
        )?;
        data.origins.insert(access.definition_origin().clone());
        if data
            .members
            .insert(
                declaration,
                Member {
                    owner,
                    metadata,
                    source,
                    access,
                },
            )
            .is_some()
        {
            return Err(Error::SlotCallable(declaration));
        }
    }
    Ok(())
}

fn identity(
    schemas: &mut SchemaDeclarations<'_>,
    metadata: SharedTypeMetadataV1<'_>,
    declaration: CallableTemplateOrigin,
    meter: &mut BudgetMeter,
) -> Result<Declaration, Error> {
    contracts::lookup(metadata.identities.identity_count(), meter)?;
    match declaration {
        CallableTemplateOrigin::Function(id) => {
            let key = metadata
                .identities
                .canonical_key::<_, SourceDeclarationKey>(id)?;
            meter.charge_collection_slots(1, &WirePath::root())?;
            schemas.functions.insert(id, key);
            Ok(Declaration::Function(id))
        }
        CallableTemplateOrigin::Accessor(id) => {
            let accessor = metadata
                .identities
                .canonical_key::<_, PropertyAccessorKey>(id)?;
            let PropertyOwner::Property(property) = accessor.owner() else {
                return Err(Error::CallableContract(declaration));
            };
            contracts::lookup(metadata.identities.identity_count(), meter)?;
            let key = metadata
                .identities
                .canonical_key::<_, SourceDeclarationKey>(property)?;
            let declaration = match accessor.role() {
                AccessorRole::Getter => Declaration::Getter(id),
                AccessorRole::Setter => Declaration::Setter(id),
            };
            meter.charge_collection_slots(2, &WirePath::root())?;
            schemas.properties.insert(property, key);
            schemas.accessors.insert(id, accessor);
            Ok(declaration)
        }
        _ => Err(Error::CallableContract(declaration)),
    }
}
