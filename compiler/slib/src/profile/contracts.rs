use super::*;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SectionLocation {
    Manifest,
    Hir,
    Mir,
    Lir,
}

impl fmt::Display for SectionLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Manifest => "Manifest",
            Self::Hir => "HIR",
            Self::Mir => "MIR",
            Self::Lir => "LIR",
        })
    }
}

impl WireEncode for SectionLocation {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Manifest => 1,
            Self::Hir => 2,
            Self::Mir => 3,
            Self::Lir => 4,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FingerprintSink {
    Hir,
    Mir,
    Lir,
    Code,
    RuntimeImage,
    EnvelopeOnly,
    LinkValidationOnly,
}

impl FingerprintSink {
    const fn bit(self) -> u8 {
        match self {
            Self::Hir => 0x01,
            Self::Mir => 0x02,
            Self::Lir => 0x04,
            Self::Code => 0x08,
            Self::RuntimeImage => 0x10,
            Self::EnvelopeOnly => 0x20,
            Self::LinkValidationOnly => 0x40,
        }
    }
}

impl WireEncode for FingerprintSink {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Hir => 1,
            Self::Mir => 2,
            Self::Lir => 3,
            Self::Code => 4,
            Self::RuntimeImage => 5,
            Self::EnvelopeOnly => 6,
            Self::LinkValidationOnly => 7,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FingerprintSinkSet(u8);

impl FingerprintSinkSet {
    pub const HIR: Self = Self(FingerprintSink::Hir.bit());
    pub const MIR: Self = Self(FingerprintSink::Mir.bit());
    pub const LIR: Self = Self(FingerprintSink::Lir.bit());
    pub const CODE: Self = Self(FingerprintSink::Code.bit());
    pub const RUNTIME_IMAGE: Self = Self(FingerprintSink::RuntimeImage.bit());
    pub const ENVELOPE_ONLY: Self = Self(FingerprintSink::EnvelopeOnly.bit());
    pub const LINK_VALIDATION_ONLY: Self = Self(FingerprintSink::LinkValidationOnly.bit());

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, sink: FingerprintSink) -> bool {
        self.0 & sink.bit() != 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityContract {
    capability: CapabilityId,
    location: SectionLocation,
    required_for: MemberPurposeSet,
    sinks: FingerprintSinkSet,
}

impl CapabilityContract {
    pub const fn capability(&self) -> &CapabilityId {
        &self.capability
    }

    pub const fn location(&self) -> SectionLocation {
        self.location
    }

    pub const fn required_for(&self) -> MemberPurposeSet {
        self.required_for
    }

    pub const fn sinks(&self) -> FingerprintSinkSet {
        self.sinks
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CapabilityContractRegistry;

impl CapabilityContractRegistry {
    pub fn contract(capability: &CapabilityId) -> Option<CapabilityContract> {
        let (location, required_for, sinks) = match (
            capability.namespace(),
            capability.name(),
            capability.major_version(),
        ) {
            ("org.scoop-lang.hir", "identity-foundation", 2) => (
                SectionLocation::Hir,
                MemberPurposeSet::COMPILE,
                FingerprintSinkSet::HIR,
            ),
            ("org.scoop-lang.mir", "identity-foundation", 1) => (
                SectionLocation::Mir,
                MemberPurposeSet::COMPILE,
                FingerprintSinkSet::MIR,
            ),
            ("org.scoop-lang.lir", "identity-foundation", 1) => (
                SectionLocation::Lir,
                MemberPurposeSet::COMPILE,
                FingerprintSinkSet::LIR,
            ),
            ("org.scoop-lang.manifest", "single-cone-production", 1) => (
                SectionLocation::Manifest,
                MemberPurposeSet::LINK,
                FingerprintSinkSet::CODE
                    .union(FingerprintSinkSet::RUNTIME_IMAGE)
                    .union(FingerprintSinkSet::LINK_VALIDATION_ONLY),
            ),
            ("org.scoop-lang.hir", "core-bootstrap-interface", 3) => (
                SectionLocation::Hir,
                MemberPurposeSet::COMPILE,
                FingerprintSinkSet::HIR,
            ),
            ("org.scoop-lang.hir", "cross-cone-interface", 3) => (
                SectionLocation::Hir,
                MemberPurposeSet::COMPILE,
                FingerprintSinkSet::HIR,
            ),
            ("org.scoop-lang.hir", "cross-cone-type-semantics", 1) => (
                SectionLocation::Hir,
                MemberPurposeSet::COMPILE,
                FingerprintSinkSet::HIR,
            ),
            ("org.scoop-lang.mir", "cross-cone-type-bridge", 1) => (
                SectionLocation::Mir,
                MemberPurposeSet::COMPILE,
                FingerprintSinkSet::MIR,
            ),
            ("org.scoop-lang.lir", "cross-cone-layout-abi", 2) => (
                SectionLocation::Lir,
                MemberPurposeSet::COMPILE,
                FingerprintSinkSet::LIR,
            ),
            ("org.scoop-lang.lir", "cross-cone-layout-link-closure", 2) => (
                SectionLocation::Lir,
                MemberPurposeSet::LINK,
                FingerprintSinkSet::CODE.union(FingerprintSinkSet::LINK_VALIDATION_ONLY),
            ),
            ("org.scoop-lang.mir", "core-bootstrap-bridge", 1) => (
                SectionLocation::Mir,
                MemberPurposeSet::COMPILE,
                FingerprintSinkSet::MIR,
            ),
            ("org.scoop-lang.mir", "cross-cone-param-free-bridge", 1) => (
                SectionLocation::Mir,
                MemberPurposeSet::COMPILE,
                FingerprintSinkSet::MIR,
            ),
            ("org.scoop-lang.lir", "cross-cone-param-free-bridge", 1) => (
                SectionLocation::Lir,
                MemberPurposeSet::COMPILE,
                FingerprintSinkSet::LIR,
            ),
            ("org.scoop-lang.lir", "cross-cone-link-closure", 1) => (
                SectionLocation::Lir,
                MemberPurposeSet::LINK,
                FingerprintSinkSet::CODE.union(FingerprintSinkSet::LINK_VALIDATION_ONLY),
            ),
            ("org.scoop-lang.lir", "strong-production", 5 | 6) => (
                SectionLocation::Lir,
                MemberPurposeSet::COMPILE_AND_LINK,
                FingerprintSinkSet::LIR
                    .union(FingerprintSinkSet::CODE)
                    .union(FingerprintSinkSet::RUNTIME_IMAGE),
            ),
            ("org.scoop-lang.lir", "link-identity-closure", 1) => (
                SectionLocation::Lir,
                MemberPurposeSet::LINK,
                FingerprintSinkSet::LINK_VALIDATION_ONLY,
            ),
            _ => return None,
        };
        Some(CapabilityContract {
            capability: capability.clone(),
            location,
            required_for,
            sinks,
        })
    }
}
