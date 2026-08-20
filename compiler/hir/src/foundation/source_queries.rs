//! Queries over the complete canonical HIR source and native metadata.

use scoop_identity::{PersistentSourceContextId, SourceContextKey, SourceIdentity};

use super::CanonicalHirFoundation;

impl CanonicalHirFoundation {
    /// Returns the canonical source record for an exact source identity.
    pub fn source_record(&self, source: &SourceIdentity) -> Option<&crate::SourceRecord> {
        self.sources
            .binary_search_by(|record| record.identity().cmp(source))
            .ok()
            .map(|index| &self.sources[index])
    }

    /// Returns all canonical source records retained by this artifact.
    #[doc(hidden)]
    pub fn source_records(&self) -> &[crate::SourceRecord] {
        &self.sources
    }

    /// Returns the canonical key for an exact source-context identity.
    pub fn source_context_key(
        &self,
        context: PersistentSourceContextId,
    ) -> Option<&SourceContextKey> {
        self.source_contexts
            .binary_search_by_key(&context, |record| record.id())
            .ok()
            .map(|index| self.source_contexts[index].key())
    }

    /// Iterates every canonical source-context identity declared by this
    /// artifact together with its validated key.
    #[doc(hidden)]
    pub fn source_context_records(
        &self,
    ) -> impl ExactSizeIterator<Item = (PersistentSourceContextId, &SourceContextKey)> {
        self.source_contexts
            .iter()
            .map(|record| (record.id(), record.key()))
    }

    #[doc(hidden)]
    pub fn source_native_contracts(&self) -> &[scoop_identity::SourceNativeExternalContractRecord] {
        &self.source_native_contracts
    }

    #[doc(hidden)]
    pub fn native_boundary_types(&self) -> &[crate::NativeBoundaryTypeDefinitionRecord] {
        &self.native_boundary_types
    }
}
