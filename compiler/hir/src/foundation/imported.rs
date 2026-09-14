use scoop_identity::{
    ConeIdentity, HirIdentityLayer, ImportedIdentityId, ImportedIdentityMap, PersistentId,
};
use scoop_wire::WireEncode;

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

/// The HIR identity foundation after atomic import into a semantic session.
pub struct ImportedHirSet {
    canonical: CanonicalHirFoundation,
    identities: ImportedIdentityMap<HirIdentityLayer>,
}

impl ImportedHirSet {
    #[doc(hidden)]
    pub fn from_validated(
        foundation: ValidatedHirFoundation,
        identities: ImportedIdentityMap<HirIdentityLayer>,
    ) -> Self {
        Self {
            canonical: foundation.into_canonical(),
            identities,
        }
    }

    #[doc(hidden)]
    pub fn from_odr_free(
        foundation: OdrFreeHirFoundation,
        identities: ImportedIdentityMap<HirIdentityLayer>,
    ) -> Self {
        Self {
            canonical: foundation.into_canonical(),
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
}

impl WireEncode for ImportedHirSet {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.canonical.encode(encoder)
    }
}
