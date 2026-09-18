//! Closed wire schema for finalized undefined-symbol requirements.

use std::fmt;

use scoop_identity::{
    ConeIdentity, DecodedNativeLibraryBinding, DecodedPersistentId, DefinitionAtomRole,
    GeneratedBridgeUnitId, NativeExternalContractFingerprint, ObjectDefinitionAtomId,
};
use scoop_lir::{
    CBridgeTargetSupportRequirementId, RuntimeSymbolContractId, TargetEhRequirementId,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind, encode};

use super::{
    CanonicalUndefinedRelocationUseV1, CanonicalUndefinedSymbolRequirementSetV1,
    CanonicalUndefinedSymbolRequirementV1, FinalUndefinedSymbolRequirementV1,
};
use crate::SlibMemberId;
use crate::link_object::defined_owners::DecodedStrongDefinitionOwnerV1;
use crate::link_object::{
    BuiltinObjectSectionRoleV1, DecodedFixedBytesV1, RelocationTargetSlotV1,
    VerifiedDarwinArm64RelocationFormV1,
};

impl WireEncode for FinalUndefinedSymbolRequirementV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::IntraConeStrong { owner } => encode_one_field_sum(encoder, 1, owner),
            Self::CoreStrong { core, owner } => {
                encoder.map(3)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                core.encode(encoder)?;
                encoder.field(2)?;
                owner.encode(encoder)
            }
            Self::GeneratedBridge { unit } => encode_one_field_sum(encoder, 3, unit),
            Self::SourceExtern { contract, library } => {
                encoder.map(3)?;
                encode_tag(encoder, 4)?;
                encoder.field(1)?;
                contract.encode(encoder)?;
                encoder.field(2)?;
                library.encode(encoder)
            }
            Self::RuntimeAbi { contract } => encode_one_field_sum(encoder, 5, contract),
            Self::TargetEhSupport { contract } => encode_one_field_sum(encoder, 6, contract),
            Self::CBridgeTargetSupport { contract } => encode_one_field_sum(encoder, 7, contract),
        }
    }
}

impl WireEncode for CanonicalUndefinedSymbolRequirementV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.use_site().encode(encoder)?;
        encoder.field(2)?;
        self.requirement().encode(encoder)
    }
}

impl WireEncode for CanonicalUndefinedSymbolRequirementSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.requirements().len() as u64)?;
        for requirement in self.requirements() {
            requirement.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireEncode for CanonicalUndefinedRelocationUseV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(10)?;
        encoder.field(1)?;
        self.source_member().encode(encoder)?;
        encoder.field(2)?;
        self.containing_atom().encode(encoder)?;
        encoder.field(3)?;
        self.containing_atom_role().encode(encoder)?;
        encoder.field(4)?;
        encode_section_role(encoder, self.section_role())?;
        encoder.field(5)?;
        encoder.unsigned(self.offset_within_atom())?;
        encoder.field(6)?;
        encoder.unsigned(u64::from(self.width_bytes()))?;
        encoder.field(7)?;
        encode_relocation_form(encoder, self.relocation_form())?;
        encoder.field(8)?;
        encoder.unsigned(self.encoded_value())?;
        encoder.field(9)?;
        encode_target_slot(encoder, self.target_slot())?;
        encoder.field(10)?;
        encoder.bytes(self.symbol())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DecodedFinalUndefinedSymbolRequirementV1 {
    IntraConeStrong {
        owner: DecodedStrongDefinitionOwnerV1,
    },
    CoreStrong {
        core: DecodedPersistentId<ConeIdentity>,
        owner: DecodedStrongDefinitionOwnerV1,
    },
    GeneratedBridge {
        unit: DecodedPersistentId<GeneratedBridgeUnitId>,
    },
    SourceExtern {
        contract: DecodedPersistentId<NativeExternalContractFingerprint>,
        library: DecodedNativeLibraryBinding,
    },
    RuntimeAbi {
        contract: DecodedFixedBytesV1<RuntimeSymbolContractId>,
    },
    TargetEhSupport {
        contract: DecodedFixedBytesV1<TargetEhRequirementId>,
    },
    CBridgeTargetSupport {
        contract: DecodedFixedBytesV1<CBridgeTargetSupportRequirementId>,
    },
}

impl WireEncode for DecodedFinalUndefinedSymbolRequirementV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::IntraConeStrong { owner } => encode_one_field_sum(encoder, 1, owner),
            Self::CoreStrong { core, owner } => {
                encoder.map(3)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                core.encode(encoder)?;
                encoder.field(2)?;
                owner.encode(encoder)
            }
            Self::GeneratedBridge { unit } => encode_one_field_sum(encoder, 3, unit),
            Self::SourceExtern { contract, library } => {
                encoder.map(3)?;
                encode_tag(encoder, 4)?;
                encoder.field(1)?;
                contract.encode(encoder)?;
                encoder.field(2)?;
                library.encode(encoder)
            }
            Self::RuntimeAbi { contract } => encode_one_field_sum(encoder, 5, contract),
            Self::TargetEhSupport { contract } => encode_one_field_sum(encoder, 6, contract),
            Self::CBridgeTargetSupport { contract } => encode_one_field_sum(encoder, 7, contract),
        }
    }
}

impl WireDecode for DecodedFinalUndefinedSymbolRequirementV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedStrongDefinitionOwnerV1::decode)
                    .map(|owner| Self::IntraConeStrong { owner })
            }
            2 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::CoreStrong {
                    core: decoder.field(1, DecodedPersistentId::decode)?,
                    owner: decoder.field(2, DecodedStrongDefinitionOwnerV1::decode)?,
                })
            }
            3 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(|unit| Self::GeneratedBridge { unit })
            }
            4 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::SourceExtern {
                    contract: decoder.field(1, DecodedPersistentId::decode)?,
                    library: decoder.field(2, DecodedNativeLibraryBinding::decode)?,
                })
            }
            5 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedFixedBytesV1::decode)
                    .map(|contract| Self::RuntimeAbi { contract })
            }
            6 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedFixedBytesV1::decode)
                    .map(|contract| Self::TargetEhSupport { contract })
            }
            7 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedFixedBytesV1::decode)
                    .map(|contract| Self::CBridgeTargetSupport { contract })
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::link_object) struct DecodedCanonicalUndefinedRelocationUseV1 {
    source_member: DecodedFixedBytesV1<SlibMemberId>,
    containing_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    containing_atom_role: DefinitionAtomRole,
    section_role: BuiltinObjectSectionRoleV1,
    offset_within_atom: u64,
    width_bytes: u8,
    relocation_form: VerifiedDarwinArm64RelocationFormV1,
    encoded_value: u64,
    target_slot: RelocationTargetSlotV1,
    symbol: Vec<u8>,
}

impl WireEncode for DecodedCanonicalUndefinedRelocationUseV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(10)?;
        encoder.field(1)?;
        self.source_member.encode(encoder)?;
        encoder.field(2)?;
        self.containing_atom.encode(encoder)?;
        encoder.field(3)?;
        self.containing_atom_role.encode(encoder)?;
        encoder.field(4)?;
        encode_section_role(encoder, self.section_role)?;
        encoder.field(5)?;
        encoder.unsigned(self.offset_within_atom)?;
        encoder.field(6)?;
        encoder.unsigned(u64::from(self.width_bytes))?;
        encoder.field(7)?;
        encode_relocation_form(encoder, self.relocation_form)?;
        encoder.field(8)?;
        encoder.unsigned(self.encoded_value)?;
        encoder.field(9)?;
        encode_target_slot(encoder, self.target_slot)?;
        encoder.field(10)?;
        encoder.bytes(&self.symbol)
    }
}

impl WireDecode for DecodedCanonicalUndefinedRelocationUseV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(10)?;
        Ok(Self {
            source_member: decoder.field(1, DecodedFixedBytesV1::decode)?,
            containing_atom: decoder.field(2, DecodedPersistentId::decode)?,
            containing_atom_role: decoder.field(3, DefinitionAtomRole::decode)?,
            section_role: decoder.field(4, decode_section_role)?,
            offset_within_atom: decoder.field(5, Decoder::unsigned)?,
            width_bytes: decoder.field(6, decode_u8)?,
            relocation_form: decoder.field(7, decode_relocation_form)?,
            encoded_value: decoder.field(8, Decoder::unsigned)?,
            target_slot: decoder.field(9, decode_target_slot)?,
            symbol: decoder.field(10, Decoder::owned_bytes)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedCanonicalUndefinedSymbolRequirementV1 {
    use_site: DecodedCanonicalUndefinedRelocationUseV1,
    requirement: DecodedFinalUndefinedSymbolRequirementV1,
}

impl WireEncode for DecodedCanonicalUndefinedSymbolRequirementV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.use_site.encode(encoder)?;
        encoder.field(2)?;
        self.requirement.encode(encoder)
    }
}

impl WireDecode for DecodedCanonicalUndefinedSymbolRequirementV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            use_site: decoder.field(1, DecodedCanonicalUndefinedRelocationUseV1::decode)?,
            requirement: decoder.field(2, DecodedFinalUndefinedSymbolRequirementV1::decode)?,
        })
    }
}

/// Untrusted wire projection. It can only be promoted by comparing it with
/// the set rebuilt from verified object relocations and requirement registries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalUndefinedSymbolRequirementSetV1 {
    requirements: Vec<DecodedCanonicalUndefinedSymbolRequirementV1>,
}

impl DecodedCanonicalUndefinedSymbolRequirementSetV1 {
    pub fn validate_against(
        self,
        expected: &CanonicalUndefinedSymbolRequirementSetV1,
    ) -> Result<CanonicalUndefinedSymbolRequirementSetV1, UndefinedSymbolRequirementValidationError>
    {
        let actual = encode(&self).map_err(UndefinedSymbolRequirementValidationError::Encode)?;
        let expected_bytes =
            encode(expected).map_err(UndefinedSymbolRequirementValidationError::Encode)?;
        if actual != expected_bytes {
            return Err(UndefinedSymbolRequirementValidationError::ProjectionMismatch);
        }
        Ok(expected.clone())
    }
}

impl WireEncode for DecodedCanonicalUndefinedSymbolRequirementSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.requirements.len() as u64)?;
        for requirement in &self.requirements {
            requirement.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalUndefinedSymbolRequirementSetV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| {
                DecodedCanonicalUndefinedSymbolRequirementV1::decode(decoder)
            })
            .map(|requirements| Self { requirements })
    }
}

#[derive(Debug)]
pub enum UndefinedSymbolRequirementValidationError {
    ProjectionMismatch,
    Encode(scoop_wire::cbor::EncodeError),
}

impl fmt::Display for UndefinedSymbolRequirementValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid decoded undefined symbol requirement set: {self:?}"
        )
    }
}

impl std::error::Error for UndefinedSymbolRequirementValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(error) => Some(error),
            Self::ProjectionMismatch => None,
        }
    }
}

fn encode_section_role(
    encoder: &mut Encoder,
    role: BuiltinObjectSectionRoleV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.unsigned(match role {
        BuiltinObjectSectionRoleV1::Text => 1,
        BuiltinObjectSectionRoleV1::ReadOnlyData => 2,
        BuiltinObjectSectionRoleV1::CString => 3,
        BuiltinObjectSectionRoleV1::WritableData => 4,
        BuiltinObjectSectionRoleV1::ZeroFill => 5,
        BuiltinObjectSectionRoleV1::GccExceptionTable => 6,
        BuiltinObjectSectionRoleV1::LlvmStackmaps => 7,
        BuiltinObjectSectionRoleV1::CompactUnwind => 8,
        BuiltinObjectSectionRoleV1::EhFrame => 9,
    })
}

fn decode_section_role(
    decoder: &mut Decoder<'_, '_>,
) -> Result<BuiltinObjectSectionRoleV1, WireError> {
    match decoder.unsigned()? {
        1 => Ok(BuiltinObjectSectionRoleV1::Text),
        2 => Ok(BuiltinObjectSectionRoleV1::ReadOnlyData),
        3 => Ok(BuiltinObjectSectionRoleV1::CString),
        4 => Ok(BuiltinObjectSectionRoleV1::WritableData),
        5 => Ok(BuiltinObjectSectionRoleV1::ZeroFill),
        6 => Ok(BuiltinObjectSectionRoleV1::GccExceptionTable),
        7 => Ok(BuiltinObjectSectionRoleV1::LlvmStackmaps),
        8 => Ok(BuiltinObjectSectionRoleV1::CompactUnwind),
        9 => Ok(BuiltinObjectSectionRoleV1::EhFrame),
        tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
    }
}

fn encode_target_slot(
    encoder: &mut Encoder,
    slot: RelocationTargetSlotV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.unsigned(match slot {
        RelocationTargetSlotV1::Single => 1,
        RelocationTargetSlotV1::Minuend => 2,
        RelocationTargetSlotV1::Subtrahend => 3,
    })
}

fn decode_target_slot(decoder: &mut Decoder<'_, '_>) -> Result<RelocationTargetSlotV1, WireError> {
    match decoder.unsigned()? {
        1 => Ok(RelocationTargetSlotV1::Single),
        2 => Ok(RelocationTargetSlotV1::Minuend),
        3 => Ok(RelocationTargetSlotV1::Subtrahend),
        tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
    }
}

fn encode_relocation_form(
    encoder: &mut Encoder,
    form: VerifiedDarwinArm64RelocationFormV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    match form {
        VerifiedDarwinArm64RelocationFormV1::Unsigned64 => encode_empty_sum(encoder, 1),
        VerifiedDarwinArm64RelocationFormV1::Subtractor64 => encode_empty_sum(encoder, 2),
        VerifiedDarwinArm64RelocationFormV1::Branch26 => encode_empty_sum(encoder, 3),
        VerifiedDarwinArm64RelocationFormV1::Page21 { explicit_addend } => {
            encode_optional_addend_sum(encoder, 4, explicit_addend)
        }
        VerifiedDarwinArm64RelocationFormV1::PageOffset12 { explicit_addend } => {
            encode_optional_addend_sum(encoder, 5, explicit_addend)
        }
        VerifiedDarwinArm64RelocationFormV1::GotLoadPage21 => encode_empty_sum(encoder, 6),
        VerifiedDarwinArm64RelocationFormV1::GotLoadPageOffset12 => encode_empty_sum(encoder, 7),
        VerifiedDarwinArm64RelocationFormV1::PointerToGot32 => encode_empty_sum(encoder, 8),
        VerifiedDarwinArm64RelocationFormV1::TlvpLoadPage21 => encode_empty_sum(encoder, 9),
        VerifiedDarwinArm64RelocationFormV1::TlvpLoadPageOffset12 => encode_empty_sum(encoder, 10),
    }
}

fn decode_relocation_form(
    decoder: &mut Decoder<'_, '_>,
) -> Result<VerifiedDarwinArm64RelocationFormV1, WireError> {
    let fields = decoder.map()?;
    let tag = decoder.field(0, Decoder::unsigned)?;
    match tag {
        1 => closed_relocation_form(
            decoder,
            fields,
            VerifiedDarwinArm64RelocationFormV1::Unsigned64,
        ),
        2 => closed_relocation_form(
            decoder,
            fields,
            VerifiedDarwinArm64RelocationFormV1::Subtractor64,
        ),
        3 => closed_relocation_form(
            decoder,
            fields,
            VerifiedDarwinArm64RelocationFormV1::Branch26,
        ),
        4 => decode_addend_relocation_form(decoder, fields, |explicit_addend| {
            VerifiedDarwinArm64RelocationFormV1::Page21 { explicit_addend }
        }),
        5 => decode_addend_relocation_form(decoder, fields, |explicit_addend| {
            VerifiedDarwinArm64RelocationFormV1::PageOffset12 { explicit_addend }
        }),
        6 => closed_relocation_form(
            decoder,
            fields,
            VerifiedDarwinArm64RelocationFormV1::GotLoadPage21,
        ),
        7 => closed_relocation_form(
            decoder,
            fields,
            VerifiedDarwinArm64RelocationFormV1::GotLoadPageOffset12,
        ),
        8 => closed_relocation_form(
            decoder,
            fields,
            VerifiedDarwinArm64RelocationFormV1::PointerToGot32,
        ),
        9 => closed_relocation_form(
            decoder,
            fields,
            VerifiedDarwinArm64RelocationFormV1::TlvpLoadPage21,
        ),
        10 => closed_relocation_form(
            decoder,
            fields,
            VerifiedDarwinArm64RelocationFormV1::TlvpLoadPageOffset12,
        ),
        tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
    }
}

fn closed_relocation_form(
    decoder: &Decoder<'_, '_>,
    fields: u64,
    form: VerifiedDarwinArm64RelocationFormV1,
) -> Result<VerifiedDarwinArm64RelocationFormV1, WireError> {
    expect_sum_length(decoder, fields, 1)?;
    Ok(form)
}

fn decode_addend_relocation_form(
    decoder: &mut Decoder<'_, '_>,
    fields: u64,
    constructor: impl FnOnce(Option<i32>) -> VerifiedDarwinArm64RelocationFormV1,
) -> Result<VerifiedDarwinArm64RelocationFormV1, WireError> {
    expect_sum_length(decoder, fields, 2)?;
    decoder.field(1, decode_optional_addend).map(constructor)
}

fn decode_optional_addend(decoder: &mut Decoder<'_, '_>) -> Result<Option<i32>, WireError> {
    let fields = decoder.map()?;
    let tag = decoder.field(0, Decoder::unsigned)?;
    match tag {
        1 => {
            expect_sum_length(decoder, fields, 1)?;
            Ok(None)
        }
        2 => {
            expect_sum_length(decoder, fields, 2)?;
            let magnitude = decoder.field(1, Decoder::u32)?;
            i32::try_from(magnitude)
                .map(Some)
                .map_err(|_| wire_error(decoder, WireErrorKind::IntegerOutOfRange))
        }
        3 => {
            expect_sum_length(decoder, fields, 2)?;
            let magnitude = decoder.field(1, Decoder::u32)?;
            match magnitude {
                0 => Err(wire_error(decoder, WireErrorKind::IntegerOutOfRange)),
                2_147_483_648 => Ok(Some(i32::MIN)),
                magnitude => i32::try_from(magnitude)
                    .map(|value| Some(-value))
                    .map_err(|_| wire_error(decoder, WireErrorKind::IntegerOutOfRange)),
            }
        }
        tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
    }
}

fn encode_optional_addend_sum(
    encoder: &mut Encoder,
    tag: u64,
    addend: Option<i32>,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    match addend {
        Some(addend) if addend >= 0 => {
            encoder.map(2)?;
            encode_tag(encoder, 2)?;
            encoder.field(1)?;
            encoder.unsigned(u64::from(addend.unsigned_abs()))
        }
        Some(addend) => {
            encoder.map(2)?;
            encode_tag(encoder, 3)?;
            encoder.field(1)?;
            encoder.unsigned(u64::from(addend.unsigned_abs()))
        }
        None => encode_empty_sum(encoder, 1),
    }
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
}

fn encode_one_field_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn decode_u8(decoder: &mut Decoder<'_, '_>) -> Result<u8, WireError> {
    let value = decoder.unsigned()?;
    u8::try_from(value).map_err(|_| wire_error(decoder, WireErrorKind::IntegerOutOfRange))
}

fn expect_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
