//! Access metadata for callable requirements.
use super::*;
use crate::{DeclarationAccessSourceV1, SourceNominalId};
use scoop_identity::SourceDeclarationKey;
use scoop_wire::WirePath;
mod contracts;

impl SharedTypeMetadataV1<'_> {
    pub(crate) fn callable_declaration_access(
        self,
        source: &crate::CallableDeclarationRecordV1,
    ) -> Result<DeclarationAccessSourceV1, Error> {
        contracts::callable_access(self, source)
    }
}
