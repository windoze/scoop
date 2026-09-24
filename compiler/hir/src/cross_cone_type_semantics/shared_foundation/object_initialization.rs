use super::*;
use scoop_identity::{
    InitializationUnitKey, PersistentInitializationUnitId, SourceDeclarationKey,
    SourceDeclarationKind,
};
use scoop_wire::WirePath;
use std::collections::BTreeMap;

impl SharedTypeMetadataV1<'_> {
    /// Indexes existing source unit keys by their typed object owner. The
    /// temporary index includes source-only units and grants no materialization.
    pub fn object_initialization_units(
        self,
        meter: &mut BudgetMeter,
    ) -> Result<BTreeMap<PersistentTypeId, PersistentInitializationUnitId>, Error> {
        let mut units = BTreeMap::new();
        for record in self
            .foundation
            .as_canonical()
            .type_source_initialization_records()
        {
            meter.charge_work(1, &WirePath::root())?;
            let owner = match record.key() {
                InitializationUnitKey::Object(owner) | InitializationUnitKey::Companion(owner) => {
                    *owner
                }
                InitializationUnitKey::TopLevelProperty(_)
                | InitializationUnitKey::ExtensionProperty(_)
                | InitializationUnitKey::GenericDelegatedExtensionApplication { .. } => continue,
            };
            meter.charge_work(
                u64::from(self.identities.identity_count().max(1).ilog2()) + 1,
                &WirePath::root(),
            )?;
            let key = self
                .identities
                .canonical_key::<_, SourceDeclarationKey>(owner)?;
            if key.origin() != self.provider
                || key.declaration_kind() != SourceDeclarationKind::Object
            {
                return Err(Error::ObjectInitializationOwner(owner));
            }
            meter.check_table_entries(units.len() as u64 + 1, &WirePath::root())?;
            meter.charge_collection_slots(1, &WirePath::root())?;
            meter.charge_work(u64::from(units.len().max(1).ilog2()) + 1, &WirePath::root())?;
            if units.insert(owner, record.id()).is_some() {
                return Err(Error::DuplicateObjectInitialization(owner));
            }
        }
        Ok(units)
    }
}
