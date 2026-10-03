use super::*;
use crate::{
    LinkDataError,
    link_data::{link_error, same_wire},
};
use scoop_identity::{PersistentIdResolver, ValidatedIdentityGraph};

impl DecodedEntryProductionPlanV1 {
    pub(crate) fn link_source(
        &self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<EntryProductionSourceV1, LinkDataError> {
        match self {
            Self::Library => Ok(EntryProductionSourceV1::Library),
            Self::Executable(entry) => {
                let declaration = identities.resolve(entry.declaration).map_err(link_error)?;
                let signature = entry
                    .source_signature
                    .clone()
                    .resolve(identities)
                    .map_err(link_error)?;
                let ordinary = ExactOrdinaryNoArgUnitSignature::new(signature.result());
                same_wire(&signature, &ordinary, "root source signature")?;
                let source = ExecutableSourceEntryIdentity::try_new(
                    &identities
                        .canonical_record(declaration)
                        .map_err(link_error)?,
                    ordinary,
                )
                .map_err(link_error)?;
                Ok(EntryProductionSourceV1::executable(source))
            }
        }
    }
}
