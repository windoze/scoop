use std::rc::Rc;

use scoop_identity::{
    ConeIdentity, ImportedIdentityId, ImportedIdentityMap, MirIdentityLayer, PersistentId,
};
use scoop_wire::WireEncode;

use super::{
    CanonicalMirFoundation, MirFoundationCounts, OdrFreeMirFoundation, ValidatedMirFoundation,
};

/// Session-local MIR identity. It cannot be interchanged with imported HIR or
/// LIR identities even when the persistent kind is the same.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImportedMirId<I: PersistentId>(ImportedIdentityId<I>);

impl<I: PersistentId> ImportedMirId<I> {
    pub const fn persistent(self) -> I {
        self.0.persistent()
    }

    pub const fn session_index(self) -> u32 {
        self.0.into_u32()
    }
}

/// The MIR identity foundation after atomic import into a semantic session.
pub struct ImportedMirFoundation {
    canonical: Rc<CanonicalMirFoundation>,
    identities: ImportedIdentityMap<MirIdentityLayer>,
}

impl ImportedMirFoundation {
    #[doc(hidden)]
    pub fn from_validated(
        foundation: ValidatedMirFoundation,
        identities: ImportedIdentityMap<MirIdentityLayer>,
    ) -> Self {
        Self {
            canonical: Rc::new(foundation.into_canonical()),
            identities,
        }
    }

    #[doc(hidden)]
    pub fn from_odr_free(
        foundation: OdrFreeMirFoundation,
        identities: ImportedIdentityMap<MirIdentityLayer>,
    ) -> Self {
        Self {
            canonical: foundation.into_shared(),
            identities,
        }
    }

    pub const fn origin(&self) -> ConeIdentity {
        self.identities.origin()
    }

    pub fn counts(&self) -> MirFoundationCounts {
        self.canonical.counts()
    }

    pub fn identity_count(&self) -> usize {
        self.identities.len()
    }

    pub fn identity<I: PersistentId + 'static>(&self, id: I) -> Option<ImportedMirId<I>> {
        self.identities.get(id).map(ImportedMirId)
    }
}

impl WireEncode for ImportedMirFoundation {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.canonical.encode(encoder)
    }
}

#[cfg(test)]
mod tests;
