use super::*;
use scoop_identity::{
    DecodedPersistentId, PersistentGenericTypeId, PersistentIdResolver, PersistentTypeId,
};
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct NominalReferences {
    concrete: HashMap<[u8; 32], PersistentTypeId>,
    generic: HashMap<[u8; 32], PersistentGenericTypeId>,
}

impl NominalReferences {
    pub(super) fn new(lowerer: &Lowerer) -> Self {
        let mut references = Self::default();
        for owner in
            lowerer
                .nominal_owners
                .keys()
                .copied()
                .chain(lowerer.dependencies.iter().flat_map(|dependencies| {
                    dependencies
                        .nominal_declarations()
                        .map(|declaration| declaration.owner())
                }))
        {
            match owner {
                hir::SourceNominalId::Concrete(id) => {
                    references.concrete.insert(*id.as_array(), id);
                }
                hir::SourceNominalId::GenericTemplate(id) => {
                    references.generic.insert(*id.as_array(), id);
                }
            }
        }
        references
    }
}

impl PersistentIdResolver<PersistentTypeId> for &NominalReferences {
    type Error = String;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentTypeId>,
    ) -> Result<PersistentTypeId, String> {
        self.concrete
            .get(id.as_array())
            .copied()
            .ok_or_else(|| "unknown nominal type reference in the current semantic view".into())
    }
}

impl PersistentIdResolver<PersistentGenericTypeId> for &NominalReferences {
    type Error = String;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentGenericTypeId>,
    ) -> Result<PersistentGenericTypeId, String> {
        self.generic
            .get(id.as_array())
            .copied()
            .ok_or_else(|| "unknown generic nominal reference in the current semantic view".into())
    }
}
