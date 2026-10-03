use super::*;
use scoop_identity::{PropertyAccessorKey, PropertyOwner, SourceDeclarationKey};

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
}
