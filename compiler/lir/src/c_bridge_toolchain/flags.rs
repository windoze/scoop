use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncode, domain_separated_cbor_hash};

use super::{C_BRIDGE_CANONICAL_FLAGS_DOMAIN, write_hex};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanonicalCBridgeFlagContractV1 {
    DarwinAppleClang,
    LinuxGcc,
}

impl CanonicalCBridgeFlagContractV1 {
    pub const CURRENT: Self = Self::DarwinAppleClang;
    pub const LINUX_GCC: Self = Self::LinuxGcc;

    pub const fn flags(self) -> &'static [CanonicalCBridgeFlagV1] {
        match self {
            Self::DarwinAppleClang => &CanonicalCBridgeFlagV1::ALL,
            Self::LinuxGcc => &CanonicalCBridgeFlagV1::LINUX_GCC,
        }
    }

    pub fn fingerprint(self) -> Result<CanonicalCBridgeFlagFingerprint, HashError> {
        domain_separated_cbor_hash(C_BRIDGE_CANONICAL_FLAGS_DOMAIN, &self)
            .map(|digest| CanonicalCBridgeFlagFingerprint(*digest.as_array()))
    }
}

impl WireEncode for CanonicalCBridgeFlagContractV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.flags().len() as u64)?;
        for flag in self.flags() {
            flag.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanonicalCBridgeFlagV1 {
    ExplicitCanonicalTarget,
    ExplicitResolvedSdkRoot,
    ExplicitMinimumDeployment,
    C11,
    RelocatableObject,
    SelectedOptimization,
    NoDebugInformation,
    NoCommonSymbols,
    OmitCompilerIdentification,
    NoStackProtector,
    NoUnwindTables,
    NoAsynchronousUnwindTables,
    NoBuiltinSubstitution,
    PositionIndependent,
}

impl CanonicalCBridgeFlagV1 {
    pub const LINUX_GCC: [Self; 11] = [
        Self::C11,
        Self::RelocatableObject,
        Self::SelectedOptimization,
        Self::NoDebugInformation,
        Self::NoCommonSymbols,
        Self::OmitCompilerIdentification,
        Self::NoStackProtector,
        Self::NoUnwindTables,
        Self::NoAsynchronousUnwindTables,
        Self::NoBuiltinSubstitution,
        Self::PositionIndependent,
    ];
    pub const ALL: [Self; 13] = [
        Self::ExplicitCanonicalTarget,
        Self::ExplicitResolvedSdkRoot,
        Self::ExplicitMinimumDeployment,
        Self::C11,
        Self::RelocatableObject,
        Self::SelectedOptimization,
        Self::NoDebugInformation,
        Self::NoCommonSymbols,
        Self::OmitCompilerIdentification,
        Self::NoStackProtector,
        Self::NoUnwindTables,
        Self::NoAsynchronousUnwindTables,
        Self::NoBuiltinSubstitution,
    ];

    const fn tag(self) -> u32 {
        match self {
            Self::ExplicitCanonicalTarget => 1,
            Self::ExplicitResolvedSdkRoot => 2,
            Self::ExplicitMinimumDeployment => 3,
            Self::C11 => 4,
            Self::RelocatableObject => 5,
            Self::SelectedOptimization => 15,
            Self::NoDebugInformation => 7,
            Self::NoCommonSymbols => 8,
            Self::OmitCompilerIdentification => 9,
            Self::NoStackProtector => 10,
            Self::NoUnwindTables => 11,
            Self::NoAsynchronousUnwindTables => 12,
            Self::NoBuiltinSubstitution => 13,
            Self::PositionIndependent => 14,
        }
    }
}

impl WireEncode for CanonicalCBridgeFlagV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(u64::from(self.tag()))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalCBridgeFlagFingerprint([u8; 32]);

impl CanonicalCBridgeFlagFingerprint {
    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}

impl WireEncode for CanonicalCBridgeFlagFingerprint {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl fmt::Display for CanonicalCBridgeFlagFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}
