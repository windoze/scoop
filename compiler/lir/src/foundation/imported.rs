use std::rc::Rc;

use scoop_identity::{
    ConeIdentity, ImportedIdentityId, ImportedIdentityMap, LirIdentityLayer, PersistentId,
};
use scoop_wire::WireEncode;

use super::{
    CanonicalLirFoundation, ConeLirFoundation, LirFoundationCounts, ValidatedLirFoundation,
};

/// Session-local LIR identity. It cannot be interchanged with imported HIR or
/// MIR identities even when the persistent kind is the same.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImportedLirId<I: PersistentId>(ImportedIdentityId<I>);

impl<I: PersistentId> ImportedLirId<I> {
    pub const fn persistent(self) -> I {
        self.0.persistent()
    }

    pub const fn session_index(self) -> u32 {
        self.0.into_u32()
    }
}

/// The LIR identity foundation after atomic import into a semantic session.
pub struct ImportedLirFoundation {
    canonical: Rc<CanonicalLirFoundation>,
    identities: ImportedIdentityMap<LirIdentityLayer>,
}

impl ImportedLirFoundation {
    #[doc(hidden)]
    pub fn from_validated(
        foundation: ValidatedLirFoundation,
        identities: ImportedIdentityMap<LirIdentityLayer>,
    ) -> Self {
        Self {
            canonical: Rc::new(foundation.into_canonical()),
            identities,
        }
    }

    #[doc(hidden)]
    pub fn from_odr_free(
        foundation: ConeLirFoundation,
        identities: ImportedIdentityMap<LirIdentityLayer>,
    ) -> Self {
        Self {
            canonical: foundation.into_shared(),
            identities,
        }
    }

    pub const fn origin(&self) -> ConeIdentity {
        self.identities.origin()
    }

    pub fn counts(&self) -> LirFoundationCounts {
        self.canonical.counts()
    }

    pub fn identity_count(&self) -> usize {
        self.identities.len()
    }

    pub fn identity<I: PersistentId + 'static>(&self, id: I) -> Option<ImportedLirId<I>> {
        self.identities.get(id).map(ImportedLirId)
    }
}

impl WireEncode for ImportedLirFoundation {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.canonical.encode(encoder)
    }
}
