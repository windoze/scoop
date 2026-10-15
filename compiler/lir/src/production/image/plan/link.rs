use super::*;
use crate::{LinkDataError, link_data::link_error};
use scoop_identity::ValidatedIdentityGraph;

impl DecodedConeImagePlanV1 {
    pub(crate) fn link_symbols(
        &self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<(PersistentSymbolRequest, [ObjectDefinitionAtomId; 2]), LinkDataError> {
        Ok((
            self.symbol.resolve(identities).map_err(link_error)?,
            self.support_atoms.link_trap_atoms(identities)?,
        ))
    }
}
