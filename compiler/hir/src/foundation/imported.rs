use scoop_identity::{
    ConeIdentity, ExportBindingKey, HirIdentityLayer, ImportedIdentityId, ImportedIdentityMap,
    PersistentExportBindingId, PersistentGenericTypeId, PersistentId, PersistentTypeId,
};
use scoop_wire::WireEncode;
use std::rc::Rc;

use super::{
    CanonicalHirFoundation, HirFoundationCounts, OdrFreeHirFoundation, ValidatedHirFoundation,
};

/// Session-local HIR identity. Its type is distinct from current HIR ids and
/// from imported MIR/LIR ids.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImportedHirId<I: PersistentId>(ImportedIdentityId<I>);

impl<I: PersistentId> ImportedHirId<I> {
    pub const fn persistent(self) -> I {
        self.0.persistent()
    }

    pub const fn session_index(self) -> u32 {
        self.0.into_u32()
    }
}

/// A source nominal reference together with its actual defining Cone. The
/// provider comes from the validated source key, not the importing artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImportedHirNominal<I: PersistentId> {
    identity: ImportedHirId<I>,
    provider: ConeIdentity,
}

impl<I: PersistentId> ImportedHirNominal<I> {
    pub const fn identity(self) -> ImportedHirId<I> {
        self.identity
    }

    pub const fn persistent(self) -> I {
        self.identity.persistent()
    }

    pub const fn session_index(self) -> u32 {
        self.identity.session_index()
    }

    pub const fn provider(self) -> ConeIdentity {
        self.provider
    }
}

/// The complete HIR identity foundation after atomic import into a semantic
/// session. It is artifact metadata, not a semantic lookup capability.
pub struct ImportedHirFoundation {
    canonical: Rc<CanonicalHirFoundation>,
    identities: ImportedIdentityMap<HirIdentityLayer>,
}

impl ImportedHirFoundation {
    #[doc(hidden)]
    pub fn from_validated(
        foundation: ValidatedHirFoundation,
        identities: ImportedIdentityMap<HirIdentityLayer>,
    ) -> Self {
        Self {
            canonical: Rc::new(foundation.into_canonical()),
            identities,
        }
    }

    #[doc(hidden)]
    pub fn from_odr_free(
        foundation: OdrFreeHirFoundation,
        identities: ImportedIdentityMap<HirIdentityLayer>,
    ) -> Self {
        Self {
            canonical: foundation.into_shared(),
            identities,
        }
    }

    #[doc(hidden)]
    pub fn from_shared(
        canonical: Rc<CanonicalHirFoundation>,
        identities: ImportedIdentityMap<HirIdentityLayer>,
    ) -> Self {
        Self {
            canonical,
            identities,
        }
    }

    pub const fn origin(&self) -> ConeIdentity {
        self.identities.origin()
    }

    pub fn counts(&self) -> HirFoundationCounts {
        self.canonical.counts()
    }

    pub fn identity_count(&self) -> usize {
        self.identities.len()
    }

    pub fn identity<I: PersistentId + 'static>(&self, id: I) -> Option<ImportedHirId<I>> {
        self.identities.get(id).map(ImportedHirId)
    }

    pub fn source_nominal(
        &self,
        id: PersistentTypeId,
    ) -> Option<ImportedHirNominal<PersistentTypeId>> {
        let (_, source) = self.canonical.source_type_by_bytes(id.as_array())?;
        Some(ImportedHirNominal {
            identity: self.identity(id)?,
            provider: source.origin(),
        })
    }

    pub fn generic_nominal(
        &self,
        id: PersistentGenericTypeId,
    ) -> Option<ImportedHirNominal<PersistentGenericTypeId>> {
        let (_, source) = self.canonical.generic_type_by_bytes(id.as_array())?;
        Some(ImportedHirNominal {
            identity: self.identity(id)?,
            provider: source.origin(),
        })
    }

    pub(crate) fn canonical_for_semantic_authority(&self) -> &CanonicalHirFoundation {
        &self.canonical
    }

    /// Returns source metadata already authenticated as part of this
    /// provider's HIR foundation.
    pub fn source_record(
        &self,
        source: &scoop_identity::SourceIdentity,
    ) -> Option<&crate::SourceRecord> {
        self.canonical
            .sources
            .binary_search_by(|record| record.identity().cmp(source))
            .ok()
            .map(|index| &self.canonical.sources[index])
    }

    /// Returns the canonical key for one provider-owned source context.
    pub fn source_context_key(
        &self,
        context: scoop_identity::PersistentSourceContextId,
    ) -> Option<&scoop_identity::SourceContextKey> {
        self.canonical
            .source_contexts
            .binary_search_by_key(&context, |record| record.id())
            .ok()
            .map(|index| self.canonical.source_contexts[index].key())
    }

    pub(crate) fn semantic_world_export_binding_key(
        &self,
        id: PersistentExportBindingId,
    ) -> Option<&ExportBindingKey> {
        self.canonical.export_binding_key(id)
    }

    pub(crate) fn native_boundary_type(
        &self,
        owner: crate::NativeBoundaryNominalOwner,
    ) -> Option<&crate::NativeBoundaryTypeDefinitionRecord> {
        self.canonical
            .native_boundary_types
            .binary_search_by(|record| record.owner().compare_sort_key(owner))
            .ok()
            .map(|index| &self.canonical.native_boundary_types[index])
    }
}

impl WireEncode for ImportedHirFoundation {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.canonical.encode(encoder)
    }
}
