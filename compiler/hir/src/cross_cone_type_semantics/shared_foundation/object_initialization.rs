use super::*;
use crate::SourceNominalId;
use scoop_identity::{
    InitializationUnitKey, PersistentInitializationUnitId, SourceDeclarationKey,
    SourceDeclarationKind,
};
use std::collections::BTreeMap;

impl SharedTypeMetadataV1<'_> {
    /// Indexes existing source unit keys by their typed object owner. The
    /// temporary index includes source-only units and grants no materialization.
    pub fn object_initialization_units(
        self,
    ) -> Result<BTreeMap<SourceNominalId, PersistentInitializationUnitId>, Error> {
        let mut units = BTreeMap::new();
        for record in self.foundation.type_source_initialization_records() {
            let owner = match record.key() {
                InitializationUnitKey::Object(owner) | InitializationUnitKey::Companion(owner) => {
                    SourceNominalId::Concrete(*owner)
                }
                InitializationUnitKey::GenericCompanionTemplate(owner) => {
                    SourceNominalId::GenericTemplate(*owner)
                }
                InitializationUnitKey::TopLevelProperty(_)
                | InitializationUnitKey::ExtensionProperty(_)
                | InitializationUnitKey::GenericCompanionApplication { .. }
                | InitializationUnitKey::GenericDelegatedExtensionApplication { .. } => continue,
            };

            let key = match owner {
                SourceNominalId::Concrete(id) => self
                    .identities
                    .canonical_key::<_, SourceDeclarationKey>(id)?,
                SourceNominalId::GenericTemplate(id) => self
                    .identities
                    .canonical_key::<_, SourceDeclarationKey>(id)?,
            };
            if key.origin() != self.provider
                || key.declaration_kind() != SourceDeclarationKind::Object
            {
                return Err(Error::ObjectInitializationOwner(owner));
            }

            if units.insert(owner, record.id()).is_some() {
                return Err(Error::DuplicateObjectInitialization(owner));
            }
        }
        Ok(units)
    }
}
