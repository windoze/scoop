use super::*;
use crate::{LinkDataError, link_data::link_error};
use scoop_identity::ValidatedIdentityGraph;

impl DecodedEntryProductionPlanV1 {
    pub(crate) fn link_source(
        &self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<EntryProductionSourceV1, LinkDataError> {
        match self {
            Self::Library => Ok(EntryProductionSourceV1::Library),
            Self::Executable(entry) => {
                let declaration = entry.declaration.resolve(identities).map_err(link_error)?;
                let signature = entry
                    .source_signature
                    .clone()
                    .resolve(identities)
                    .map_err(link_error)?;
                let source = match declaration {
                    scoop_identity::ExecutableEntryDeclaration::Source(id) => {
                        ExecutableEntryIdentity::try_new(
                            &identities.canonical_record(id).map_err(link_error)?,
                            signature,
                        )
                    }
                    scoop_identity::ExecutableEntryDeclaration::Generated(id) => {
                        ExecutableEntryIdentity::for_test(
                            &identities.canonical_record(id).map_err(link_error)?,
                            signature,
                        )
                    }
                }
                .map_err(link_error)?;
                Ok(EntryProductionSourceV1::executable(source))
            }
        }
    }
}
