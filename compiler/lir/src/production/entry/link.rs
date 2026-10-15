use super::*;
use crate::{LinkDataError, link_data::link_error};
use scoop_identity::ValidatedIdentityGraph;

impl DecodedEntryProductionPlanV1 {
    pub(crate) fn link_symbol(
        &self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<Option<PersistentSymbolRequest>, LinkDataError> {
        match self {
            Self::Library => Ok(None),
            Self::Executable(entry) => entry
                .root_descriptor_symbol
                .resolve(identities)
                .map(Some)
                .map_err(link_error),
        }
    }
}
