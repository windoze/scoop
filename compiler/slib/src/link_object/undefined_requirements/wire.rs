//! Closed wire schema for finalized undefined-symbol requirements.

use scoop_wire::{Encoder, WireEncode};

use super::{
    CanonicalUndefinedRelocationUseV1, CanonicalUndefinedSymbolRequirementSetV1,
    CanonicalUndefinedSymbolRequirementV1, FinalUndefinedSymbolRequirementV1,
};
use crate::link_object::{
    BuiltinObjectSectionRoleV1, RelocationTargetSlotV1, VerifiedObjectRelocationFormV1,
};

impl WireEncode for FinalUndefinedSymbolRequirementV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::OdrMember { member } => encode_one_field_sum(encoder, 9, member),
            Self::IntraConeStrong { owner } => encode_one_field_sum(encoder, 1, owner),
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
            Self::Native { symbol } => encode_native_requirement(encoder, symbol),
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
        BuiltinObjectSectionRoleV1::ThreadLocalData => 10,
        BuiltinObjectSectionRoleV1::ThreadLocalZeroFill => 11,
        BuiltinObjectSectionRoleV1::ThreadLocalVariables => 12,
        BuiltinObjectSectionRoleV1::ObjectMetadata => 13,
        BuiltinObjectSectionRoleV1::WindowsUnwindTable => 14,
        BuiltinObjectSectionRoleV1::WindowsUnwindInfo => 15,
    })
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

fn encode_relocation_form(
    encoder: &mut Encoder,
    form: VerifiedObjectRelocationFormV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    match form {
        VerifiedObjectRelocationFormV1::CoffAmd64 { kind, width } => {
            encoder.map(3)?;
            encode_tag(encoder, 12)?;
            encoder.field(1)?;
            encoder.unsigned(u64::from(kind))?;
            encoder.field(2)?;
            encoder.unsigned(u64::from(width))
        }
        VerifiedObjectRelocationFormV1::ElfX86_64Rela {
            kind,
            addend,
            width,
        } => {
            encoder.map(4)?;
            encode_tag(encoder, 11)?;
            encoder.field(1)?;
            encoder.unsigned(u64::from(kind))?;
            encoder.field(2)?;
            encoder.unsigned(u64::from(width))?;
            encoder.field(3)?;
            encoder.unsigned(addend as u64)
        }
        VerifiedObjectRelocationFormV1::Unsigned32 => encode_empty_sum(encoder, 13),
        VerifiedObjectRelocationFormV1::Subtractor32 => encode_empty_sum(encoder, 14),
        VerifiedObjectRelocationFormV1::X86_64PcRelative32 { kind } => {
            encoder.map(2)?;
            encode_tag(encoder, 15)?;
            encoder.field(1)?;
            encoder.unsigned(u64::from(kind as u8))
        }
        VerifiedObjectRelocationFormV1::Unsigned64 => encode_empty_sum(encoder, 1),
        VerifiedObjectRelocationFormV1::Subtractor64 => encode_empty_sum(encoder, 2),
        VerifiedObjectRelocationFormV1::Branch26 => encode_empty_sum(encoder, 3),
        VerifiedObjectRelocationFormV1::Page21 { explicit_addend } => {
            encode_optional_addend_sum(encoder, 4, explicit_addend)
        }
        VerifiedObjectRelocationFormV1::PageOffset12 { explicit_addend } => {
            encode_optional_addend_sum(encoder, 5, explicit_addend)
        }
        VerifiedObjectRelocationFormV1::GotLoadPage21 => encode_empty_sum(encoder, 6),
        VerifiedObjectRelocationFormV1::GotLoadPageOffset12 => encode_empty_sum(encoder, 7),
        VerifiedObjectRelocationFormV1::PointerToGot32 => encode_empty_sum(encoder, 8),
        VerifiedObjectRelocationFormV1::TlvpLoadPage21 => encode_empty_sum(encoder, 9),
        VerifiedObjectRelocationFormV1::TlvpLoadPageOffset12 => encode_empty_sum(encoder, 10),
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

fn encode_native_requirement(
    encoder: &mut Encoder,
    symbol: &[u8],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, 10)?;
    encoder.field(1)?;
    encoder.bytes(symbol)
}
