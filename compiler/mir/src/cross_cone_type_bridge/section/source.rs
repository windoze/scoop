use super::*;
use scoop_identity::InitializationCallableRole;

/// The stage projects this contract from independent checked source input.
/// Candidate selected records and candidate unit inventories are not sources.
pub trait MirTypeBridgeSectionSourceAuthorityV1<E>:
    MirTypeBridgeSourceSemanticAuthorityV1<E>
{
    fn committed_external_uses(&self) -> Result<&[MirTypeBridgeDependencyV1], E>;
    fn local_initialization_units(&self) -> Result<&[PersistentInitializationUnitId], E>;
    fn initialization_signature(
        &self,
        unit: PersistentInitializationUnitId,
        role: InitializationCallableRole,
    ) -> Result<&MirBridgeCallableSignatureV1, E>;
}

/// Both paths require the same semantic replay. Only the producer path also
/// carries the sealer's proof of actual local MIR bodies.
#[derive(Clone, Copy)]
pub enum MirTypeBridgeLocalAuthorityV1<'a> {
    Producer {
        provider: ConeIdentity,
        input: &'a crate::SingleConeStrongMirInput,
        ordinary: &'a crate::CrossConeMirBridgeSectionV1,
    },
    Reader {
        provider: ConeIdentity,
        foundation: &'a crate::OdrFreeMirFoundation,
        production: &'a crate::CoreBootstrapBridgeSectionV1,
        ordinary: &'a crate::CrossConeMirBridgeSectionV1,
    },
}
impl<'a> MirTypeBridgeLocalAuthorityV1<'a> {
    pub const fn provider(self) -> ConeIdentity {
        match self {
            Self::Producer { provider, .. } | Self::Reader { provider, .. } => provider,
        }
    }
    pub const fn foundation(self) -> &'a crate::OdrFreeMirFoundation {
        match self {
            Self::Producer { input, .. } => input.foundation(),
            Self::Reader { foundation, .. } => foundation,
        }
    }
    pub const fn production(self) -> &'a crate::CoreBootstrapBridgeSectionV1 {
        match self {
            Self::Producer { input, .. } => input.production(),
            Self::Reader { production, .. } => production,
        }
    }
    pub const fn ordinary(self) -> &'a crate::CrossConeMirBridgeSectionV1 {
        match self {
            Self::Producer { ordinary, .. } | Self::Reader { ordinary, .. } => ordinary,
        }
    }
}
