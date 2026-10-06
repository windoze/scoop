//! Typed platform helpers used by generated C and LLVM object emission.

use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{CBridgeToolchainProfileId, TargetProfileWireId};
use scoop_wire::{Encoder, HashError, WireEncode, domain_separated_cbor_hash};

use crate::{
    CBridgeToolchainFingerprint, CBridgeToolchainProfileV1, LirTargetProfile,
    TargetProfileFingerprint,
};

const C_BRIDGE_TARGET_SUPPORT_DOMAIN: &str = "scoop-c-bridge-target-support-v1";

/// Platform helpers whose machine contracts are fixed by the selected target.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CBridgeTargetSupportV1 {
    Memcpy,
    TlvBootstrap,
    TlsGetAddr,
    Fmodf,
    Fmod,
}

impl CBridgeTargetSupportV1 {
    pub const ALL: [Self; 5] = [
        Self::Memcpy,
        Self::TlvBootstrap,
        Self::TlsGetAddr,
        Self::Fmodf,
        Self::Fmod,
    ];

    pub const fn for_target(target: LirTargetProfile) -> &'static [Self] {
        match target.id() {
            crate::TargetProfileId::DarwinAarch64 => {
                &[Self::Memcpy, Self::TlvBootstrap, Self::Fmodf, Self::Fmod]
            }
            crate::TargetProfileId::LinuxX86_64Gnu | crate::TargetProfileId::LinuxX86_64Musl => {
                &[Self::Memcpy, Self::TlsGetAddr, Self::Fmodf, Self::Fmod]
            }
        }
    }

    pub const fn logical_symbol(self) -> &'static str {
        match self {
            Self::Memcpy => "memcpy",
            Self::TlvBootstrap => "_tlv_bootstrap",
            Self::TlsGetAddr => "__tls_get_addr",
            Self::Fmodf => "fmodf",
            Self::Fmod => "fmod",
        }
    }
}

impl WireEncode for CBridgeTargetSupportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Memcpy => 1,
            Self::TlvBootstrap => 2,
            Self::TlsGetAddr => 3,
            Self::Fmodf => 4,
            Self::Fmod => 5,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CBridgeTargetSupportRequirementId([u8; 32]);

impl CBridgeTargetSupportRequirementId {
    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}

impl WireEncode for CBridgeTargetSupportRequirementId {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl fmt::Display for CBridgeTargetSupportRequirementId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CBridgeTargetSupportRequirementV1 {
    id: CBridgeTargetSupportRequirementId,
    target: TargetProfileWireId,
    target_fingerprint: TargetProfileFingerprint,
    profile_id: CBridgeToolchainProfileId,
    profile_fingerprint: CBridgeToolchainFingerprint,
    support: CBridgeTargetSupportV1,
}

impl CBridgeTargetSupportRequirementV1 {
    pub fn current(
        target: LirTargetProfile,
        profile: &CBridgeToolchainProfileV1,
        support: CBridgeTargetSupportV1,
    ) -> Result<Self, CBridgeTargetSupportRegistryError> {
        if !CBridgeTargetSupportV1::for_target(target).contains(&support) {
            return Err(
                CBridgeTargetSupportRegistryError::UnsupportedTargetSupport { target, support },
            );
        }
        let target_id = target.wire_id();
        let target_fingerprint = target.fingerprint()?;
        if profile.contract().target() != &target_id {
            return Err(CBridgeTargetSupportRegistryError::ProfileTargetMismatch);
        }
        if profile.contract().target_fingerprint() != target_fingerprint {
            return Err(CBridgeTargetSupportRegistryError::ProfileTargetFingerprintMismatch);
        }
        let input = CBridgeTargetSupportRequirementInputV1 {
            target: target_id,
            target_fingerprint,
            profile_id: profile.id().clone(),
            profile_fingerprint: profile.fingerprint(),
            support,
        };
        let digest = domain_separated_cbor_hash(C_BRIDGE_TARGET_SUPPORT_DOMAIN, &input)?;
        Ok(Self {
            id: CBridgeTargetSupportRequirementId(*digest.as_array()),
            target: input.target,
            target_fingerprint: input.target_fingerprint,
            profile_id: input.profile_id,
            profile_fingerprint: input.profile_fingerprint,
            support,
        })
    }

    pub const fn id(&self) -> CBridgeTargetSupportRequirementId {
        self.id
    }

    pub const fn target(&self) -> &TargetProfileWireId {
        &self.target
    }

    pub const fn target_fingerprint(&self) -> TargetProfileFingerprint {
        self.target_fingerprint
    }

    pub const fn profile_id(&self) -> &CBridgeToolchainProfileId {
        &self.profile_id
    }

    pub const fn profile_fingerprint(&self) -> CBridgeToolchainFingerprint {
        self.profile_fingerprint
    }

    pub const fn support(&self) -> CBridgeTargetSupportV1 {
        self.support
    }

    pub fn object_symbol(&self, target: LirTargetProfile) -> Vec<u8> {
        target
            .contract()
            .native_symbol_normalization()
            .compiler_generated_object_symbol(self.support.logical_symbol())
            .into_bytes()
    }
}

impl WireEncode for CBridgeTargetSupportRequirementV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        CBridgeTargetSupportRequirementInputV1 {
            target: self.target.clone(),
            target_fingerprint: self.target_fingerprint,
            profile_id: self.profile_id.clone(),
            profile_fingerprint: self.profile_fingerprint,
            support: self.support,
        }
        .encode(encoder)
    }
}

struct CBridgeTargetSupportRequirementInputV1 {
    target: TargetProfileWireId,
    target_fingerprint: TargetProfileFingerprint,
    profile_id: CBridgeToolchainProfileId,
    profile_fingerprint: CBridgeToolchainFingerprint,
    support: CBridgeTargetSupportV1,
}

impl WireEncode for CBridgeTargetSupportRequirementInputV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.target.encode(encoder)?;
        encoder.field(2)?;
        self.target_fingerprint.encode(encoder)?;
        encoder.field(3)?;
        self.profile_id.encode(encoder)?;
        encoder.field(4)?;
        self.profile_fingerprint.encode(encoder)?;
        encoder.field(5)?;
        self.support.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CBridgeTargetSupportRegistryV1 {
    target: LirTargetProfile,
    profile_id: CBridgeToolchainProfileId,
    profile_fingerprint: CBridgeToolchainFingerprint,
    by_object_symbol: BTreeMap<Vec<u8>, CBridgeTargetSupportRequirementV1>,
}

impl CBridgeTargetSupportRegistryV1 {
    pub fn current(
        target: LirTargetProfile,
        profile: &CBridgeToolchainProfileV1,
    ) -> Result<Self, CBridgeTargetSupportRegistryError> {
        let mut by_object_symbol = BTreeMap::new();
        for &support in CBridgeTargetSupportV1::for_target(target) {
            let requirement = CBridgeTargetSupportRequirementV1::current(target, profile, support)?;
            let object_symbol = requirement.object_symbol(target);
            if by_object_symbol
                .insert(object_symbol.clone(), requirement)
                .is_some()
            {
                return Err(CBridgeTargetSupportRegistryError::DuplicateObjectSymbol(
                    object_symbol,
                ));
            }
        }
        Ok(Self {
            target,
            profile_id: profile.id().clone(),
            profile_fingerprint: profile.fingerprint(),
            by_object_symbol,
        })
    }

    pub const fn target(&self) -> LirTargetProfile {
        self.target
    }

    pub const fn profile_id(&self) -> &CBridgeToolchainProfileId {
        &self.profile_id
    }

    pub const fn profile_fingerprint(&self) -> CBridgeToolchainFingerprint {
        self.profile_fingerprint
    }

    pub fn requirements(
        &self,
    ) -> impl ExactSizeIterator<Item = &CBridgeTargetSupportRequirementV1> {
        self.by_object_symbol.values()
    }

    pub fn requirement_for_object_symbol(
        &self,
        symbol: &[u8],
    ) -> Option<&CBridgeTargetSupportRequirementV1> {
        self.by_object_symbol.get(symbol)
    }
}

#[derive(Debug)]
pub enum CBridgeTargetSupportRegistryError {
    UnsupportedTargetSupport {
        target: LirTargetProfile,
        support: CBridgeTargetSupportV1,
    },
    ProfileTargetMismatch,
    ProfileTargetFingerprintMismatch,
    DuplicateObjectSymbol(Vec<u8>),
    Hash(HashError),
}

impl From<HashError> for CBridgeTargetSupportRegistryError {
    fn from(error: HashError) -> Self {
        Self::Hash(error)
    }
}

impl fmt::Display for CBridgeTargetSupportRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid C-bridge target support registry: {self:?}"
        )
    }
}

impl std::error::Error for CBridgeTargetSupportRegistryError {}

#[cfg(test)]
mod tests;
