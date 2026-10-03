use super::*;
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
    ) -> Result<BTreeMap<PersistentTypeId, PersistentInitializationUnitId>, Error> {
        let mut units = BTreeMap::new();
        for record in self.foundation.type_source_initialization_records() {
            let owner = match record.key() {
                InitializationUnitKey::Object(owner) | InitializationUnitKey::Companion(owner) => {
                    *owner
                }
                InitializationUnitKey::TopLevelProperty(_)
                | InitializationUnitKey::ExtensionProperty(_)
                | InitializationUnitKey::GenericDelegatedExtensionApplication { .. } => continue,
            };

            let key = self
                .identities
                .canonical_key::<_, SourceDeclarationKey>(owner)?;
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
