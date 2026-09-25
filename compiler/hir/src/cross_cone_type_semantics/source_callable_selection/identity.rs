use super::*;
use scoop_identity::{
    ExactTypeKey, PersistentExactTypeId, PropertyAccessorKey, PropertyOwner, SourceDeclarationKey,
};

pub(super) fn is_local(
    provider: ConeIdentity,
    identities: &ValidatedIdentityGraph,
    declaration: Origin,
) -> Result<bool, Error> {
    let key = match declaration {
        Origin::Function(id) => identities.canonical_key::<_, SourceDeclarationKey>(id)?,
        Origin::Accessor(id) => {
            let key = identities.canonical_key::<_, PropertyAccessorKey>(id)?;

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

impl Selection<'_, '_> {
    pub(super) fn require_property(&mut self, id: PersistentPropertyId) -> Result<(), Error> {
        if self
            .identities
            .canonical_key::<_, SourceDeclarationKey>(id)?
            .origin()
            != self.provider
        {
            return Ok(());
        }

        if !self.properties.contains(&id) {
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

        let exact =
            PersistentExactTypeId::from_key(&key).map_err(|error| Error::Key(error.to_string()))?;

        Ok(types.inheritance().get(exact).is_some())
    }
}
