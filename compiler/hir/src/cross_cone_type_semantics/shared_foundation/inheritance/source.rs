use scoop_identity::PersistentGenericTypeId;

use super::*;
use crate::NominalRepresentationShapeV1;

pub(super) fn collect<'a>(
    context: &mut Context<'a>,
    provider: CheckedSharedTypeFoundationV1<'a>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let metadata = provider.metadata;
    let path = WirePath::root();
    for declaration in metadata.public.nominal_interfaces().all_records() {
        let owner = declaration.declaration();
        meter.charge_work(
            1 + u64::from(metadata.identities.identity_count().max(1).ilog2()),
            &path,
        )?;
        let key = match owner {
            SourceNominalId::Concrete(owner) => metadata
                .identities
                .canonical_key::<PersistentTypeId, SourceDeclarationKey>(owner)?,
            SourceNominalId::GenericTemplate(owner) => {
                metadata
                    .identities
                    .canonical_key::<PersistentGenericTypeId, SourceDeclarationKey>(owner)?
            }
        };
        let access = declaration_access(metadata, declaration, &key, meter)?;
        let length = scoop_wire::encoded_length(access.definition_origin())
            .map_err(|error| Error::Key(error.to_string()))?;
        meter.charge_owned_bytes(length, &path)?;
        meter.charge_collection_slots(2, &path)?;
        meter.charge_work(
            (length + 1).saturating_mul(1 + u64::from(context.sources.len().max(1).ilog2())),
            &path,
        )?;
        context.origins.insert(access.definition_origin().clone());
        if context
            .sources
            .insert(owner, Source { key, access })
            .is_some()
        {
            return Err(Error::InheritanceSource(owner));
        }
    }
    for representation in provider.representations.table().records() {
        if let NominalRepresentationShapeV1::Object { backing_class, .. } = representation.shape() {
            let types = MetadataTypes {
                current: metadata,
                dependencies: &[],
            };
            let exact = types.nominal_exact(*backing_class, meter)?;
            let key = types.key(exact, meter)?;
            let generated = metadata
                .identities
                .canonical_key::<PersistentTypeId, GeneratedNominalKey>(*backing_class)?;
            meter.charge_collection_slots(3, &path)?;
            context.exacts.insert(exact, key);
            context.generated.insert(*backing_class, generated);
            context
                .objects
                .insert(representation.owner(), representation);
        }
    }
    Ok(())
}
