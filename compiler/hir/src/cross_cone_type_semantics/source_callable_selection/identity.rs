use super::*;
use scoop_identity::{
    ExactTypeKey, PersistentExactTypeId, PropertyAccessorKey, PropertyOwner, SourceDeclarationKey,
};

pub(super) fn is_local(
    provider: ConeIdentity,
    identities: &ValidatedIdentityGraph,
    declaration: Origin,
    meter: &mut BudgetMeter,
) -> Result<bool, Error> {
    lookup(identities.identity_count(), meter)?;
    let key = match declaration {
        Origin::Function(id) => identities.canonical_key::<_, SourceDeclarationKey>(id)?,
        Origin::Accessor(id) => {
            let key = identities.canonical_key::<_, PropertyAccessorKey>(id)?;
            lookup(identities.identity_count(), meter)?;
            match key.owner() {
                PropertyOwner::Property(id) => {
                    identities.canonical_key::<_, SourceDeclarationKey>(id)?
                }
                PropertyOwner::ExtensionProperty(id) => {
                    identities.canonical_key::<_, SourceDeclarationKey>(id)?
                }
            }
        }
        Origin::GenericFunction(_) | Origin::Constructor(_) | Origin::VariantConstructor(_) => {
            return Ok(false);
        }
    };
    Ok(key.origin() == provider)
}

impl Selection<'_, '_, '_> {
    pub(super) fn require_property(&mut self, id: PersistentPropertyId) -> Result<(), Error> {
        lookup(self.identities.identity_count(), self.meter)?;
        if self
            .identities
            .canonical_key::<_, SourceDeclarationKey>(id)?
            .origin()
            != self.provider
        {
            return Ok(());
        }
        lookup(self.properties.len(), self.meter)?;
        if !self.properties.contains(&id) {
            self.meter
                .check_table_entries(self.properties.len() as u64 + 1, &WirePath::root())?;
            self.meter.charge_collection_slots(1, &WirePath::root())?;
            self.properties.insert(id);
        }
        Ok(())
    }

    pub(super) fn materialized_owner(
        &mut self,
        source: &CallableDeclarationRecordV1,
        types: &CrossConeTypeSemanticsSectionV1,
    ) -> Result<bool, Error> {
        let Some(crate::SourceNominalId::Concrete(owner)) = source.owner().nominal_owner() else {
            return Ok(false);
        };
        let key = ExactTypeKey::Nominal(owner);
        self.meter.charge_sha256(
            PersistentExactTypeId::hash_stream_length(&key)
                .map_err(|error| Error::Key(error.to_string()))?,
            &WirePath::root(),
        )?;
        let exact =
            PersistentExactTypeId::from_key(&key).map_err(|error| Error::Key(error.to_string()))?;
        lookup(types.inheritance().records().len(), self.meter)?;
        Ok(types.inheritance().get(exact).is_some())
    }
}
