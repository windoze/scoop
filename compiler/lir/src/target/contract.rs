use std::fmt;

use scoop_identity::{TargetProfileId, TargetProfileWireId};
use scoop_wire::{Encoder, HashError, WireEncode, domain_separated_cbor_hash};

use super::{
    BackendScalarKind, InternalPointerCarrier, LirTargetProfile, PointerNullEncoding,
    PointerRepresentation, ScalarLayout,
};

const TARGET_PROFILE_DOMAIN: &str = "scoop-target-profile-contract-v1";
const MAXIMUM_MANAGED_OBJECT_SIZE: u64 = i64::MAX as u64;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ByteOrder {
    Little,
}

impl WireEncode for ByteOrder {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(1)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ScoopAbiClassifier {
    ScalarInterfaceAndSmallValueParts,
}

impl WireEncode for ScoopAbiClassifier {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(3)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CAbiLoweringProfile {
    ScalarDirectOrSystemCBridge,
}

impl WireEncode for CAbiLoweringProfile {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(2)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NativeSymbolNormalization {
    MachOExternalUnderscore,
    ElfIdentity,
}

impl NativeSymbolNormalization {
    /// Normalize a canonical compiler-generated logical symbol to the bytes
    /// used by the target object format. Unlike ordinary C identifiers, a C
    /// `asm` label names these bytes directly and therefore must consume this
    /// target projection explicitly.
    pub fn compiler_generated_object_symbol(self, logical_symbol: &str) -> String {
        match self {
            Self::MachOExternalUnderscore => format!("_{logical_symbol}"),
            Self::ElfIdentity => logical_symbol.to_owned(),
        }
    }
}

impl WireEncode for NativeSymbolNormalization {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::MachOExternalUnderscore => 1,
            Self::ElfIdentity => 2,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::NativeSymbolNormalization;

    #[test]
    fn macho_normalizes_compiler_generated_symbols_for_raw_asm_labels() {
        assert_eq!(
            NativeSymbolNormalization::MachOExternalUnderscore
                .compiler_generated_object_symbol("scoop$1$cb$abc"),
            "_scoop$1$cb$abc"
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TargetProfileContract {
    profile: LirTargetProfile,
}

impl TargetProfileContract {
    pub(super) const fn new(profile: LirTargetProfile) -> Self {
        Self { profile }
    }

    pub const fn canonical_triple(self) -> &'static str {
        self.profile.id().canonical_triple()
    }

    pub const fn byte_order(self) -> ByteOrder {
        ByteOrder::Little
    }

    pub const fn stack_alignment_bytes(self) -> u64 {
        16
    }

    pub const fn maximum_managed_alignment(self) -> u64 {
        16
    }

    pub const fn maximum_managed_object_size(self) -> u64 {
        MAXIMUM_MANAGED_OBJECT_SIZE
    }

    pub const fn scoop_abi_classifier(self) -> ScoopAbiClassifier {
        ScoopAbiClassifier::ScalarInterfaceAndSmallValueParts
    }

    pub const fn c_abi_lowering(self) -> CAbiLoweringProfile {
        CAbiLoweringProfile::ScalarDirectOrSystemCBridge
    }

    pub const fn native_symbol_normalization(self) -> NativeSymbolNormalization {
        match self.profile.id() {
            TargetProfileId::DarwinAarch64 => NativeSymbolNormalization::MachOExternalUnderscore,
            TargetProfileId::LinuxX86_64Gnu | TargetProfileId::LinuxX86_64Musl => {
                NativeSymbolNormalization::ElfIdentity
            }
        }
    }
}

impl WireEncode for TargetProfileContract {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(15)?;
        encoder.field(1)?;
        encoder.text(self.canonical_triple())?;
        encoder.field(2)?;
        encoder.text(self.profile.canonical_llvm_data_layout())?;
        encoder.field(3)?;
        self.profile.id().object_format().encode(encoder)?;
        encoder.field(4)?;
        self.byte_order().encode(encoder)?;
        encoder.field(5)?;
        encode_scalar_layouts(encoder, self.profile)?;
        encoder.field(6)?;
        encode_plain_pointer_layout(encoder, self.profile.managed_pointer_layout())?;
        encoder.field(7)?;
        encode_qualified_pointer_layout(encoder, self.profile.data_pointer())?;
        encoder.field(8)?;
        encode_qualified_pointer_layout(encoder, self.profile.code_pointer())?;
        encoder.field(9)?;
        encode_plain_pointer_layout(encoder, self.profile.metadata_pointer_layout())?;
        encoder.field(10)?;
        encoder.unsigned(self.stack_alignment_bytes())?;
        encoder.field(11)?;
        encoder.unsigned(self.maximum_managed_alignment())?;
        encoder.field(12)?;
        encoder.unsigned(self.maximum_managed_object_size())?;
        encoder.field(13)?;
        self.scoop_abi_classifier().encode(encoder)?;
        encoder.field(14)?;
        self.c_abi_lowering().encode(encoder)?;
        encoder.field(15)?;
        self.native_symbol_normalization().encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TargetProfileFingerprint([u8; 32]);

impl TargetProfileFingerprint {
    pub fn from_profile(profile: LirTargetProfile) -> Result<Self, HashError> {
        let input = TargetProfileFingerprintInput {
            id: profile.wire_id(),
            contract: profile.contract(),
        };
        domain_separated_cbor_hash(TARGET_PROFILE_DOMAIN, &input)
            .map(|digest| Self(*digest.as_array()))
    }

    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}

impl WireEncode for TargetProfileFingerprint {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl fmt::Display for TargetProfileFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

struct TargetProfileFingerprintInput {
    id: TargetProfileWireId,
    contract: TargetProfileContract,
}

impl WireEncode for TargetProfileFingerprintInput {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.id.encode(encoder)?;
        encoder.field(2)?;
        self.contract.encode(encoder)
    }
}

fn encode_scalar_layouts(
    encoder: &mut Encoder,
    profile: LirTargetProfile,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(5)?;
    for (tag, kind) in [
        (1, BackendScalarKind::I1),
        (2, BackendScalarKind::I8),
        (3, BackendScalarKind::I16),
        (4, BackendScalarKind::I32),
        (5, BackendScalarKind::I64),
    ] {
        let layout = profile.scalar_layout(kind);
        encoder.map(3)?;
        encoder.field(1)?;
        encoder.unsigned(tag)?;
        encoder.field(2)?;
        encoder.unsigned(layout.size_bytes())?;
        encoder.field(3)?;
        encoder.unsigned(layout.alignment_bytes())?;
    }
    Ok(())
}

fn encode_plain_pointer_layout(
    encoder: &mut Encoder,
    layout: ScalarLayout,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(1)?;
    encoder.unsigned(layout.size_bytes())?;
    encoder.field(2)?;
    encoder.unsigned(layout.alignment_bytes())
}

fn encode_qualified_pointer_layout(
    encoder: &mut Encoder,
    representation: PointerRepresentation,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(4)?;
    encoder.field(1)?;
    encoder.unsigned(representation.layout().size_bytes())?;
    encoder.field(2)?;
    encoder.unsigned(representation.layout().alignment_bytes())?;
    encoder.field(3)?;
    encoder.unsigned(match representation.null_encoding() {
        PointerNullEncoding::AllZeroBits => 1,
    })?;
    encoder.field(4)?;
    encoder.unsigned(match representation.carrier() {
        InternalPointerCarrier::BitPreservingU64 => 1,
    })
}
