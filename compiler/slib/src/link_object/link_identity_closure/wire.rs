//! Strict untrusted wire projection for the Link identity closure.

use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{
    DecodedPersistentId, DefinitionAtomRole, DigestPatchIntentId, GeneratedBridgeUnitId,
    ObjectDefinitionAtomId, ObjectDefinitionPlanId, PersistentId,
};
use scoop_lir::{DigestFinalizationPlanV1, ProducerUnitPartitionV1};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind, encode};

mod digest_inputs;
mod errors;
pub use errors::*;
mod final_objects;
pub use final_objects::LinkFinalObjectProjectionError;
mod materializations;
mod object_projections;
mod resources;
mod symbol_projections;

use super::{LinkIdentityClosureBuildError, LinkIdentityClosureSectionV1};
use crate::SlibMemberId;
use crate::link_object::{
    CanonicalDefinedLinkSymbolOwnerSetV1, CanonicalGeneratedBridgeObjectUnitSetV1,
    CanonicalScoopLirObjectUnitSetV1, CanonicalUndefinedSymbolRequirementSetV1,
    DecodedCanonicalDefinedLinkSymbolOwnerSetV1, DecodedCanonicalUndefinedSymbolRequirementSetV1,
    DecodedCodeLinkObjectMemberSetV1, DecodedFixedBytesV1, DefinedLinkSymbolOwnerValidationError,
    LinkObjectMemberSetPlanError, ObjectUnitSetError, PlannedLinkObjectMemberSetV1,
    ProvisionalDigestPatchSiteV1, UndefinedSymbolRequirementValidationError,
    VerifiedCodeFingerprintV1, VerifiedCodeFingerprintV2, VerifiedScoopLirDigestPatchSiteSetV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecodedLinkObjectMaterializationV1 {
    ScoopLir {
        member: DecodedFixedBytesV1<SlibMemberId>,
        units: Vec<DecodedPersistentId<ObjectDefinitionPlanId>>,
    },
    GeneratedCBridge {
        member: DecodedFixedBytesV1<SlibMemberId>,
        units: Vec<DecodedPersistentId<GeneratedBridgeUnitId>>,
    },
}

impl WireEncode for DecodedLinkObjectMaterializationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::ScoopLir { .. } => 1,
            Self::GeneratedCBridge { .. } => 2,
        })?;
        encoder.field(1)?;
        match self {
            Self::ScoopLir { member, .. } | Self::GeneratedCBridge { member, .. } => {
                member.encode(encoder)?;
            }
        }
        encoder.field(2)?;
        match self {
            Self::ScoopLir { units, .. } => encode_array(encoder, units),
            Self::GeneratedCBridge { units, .. } => encode_array(encoder, units),
        }
    }
}

impl WireDecode for DecodedLinkObjectMaterializationV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let member = decoder.field(1, DecodedFixedBytesV1::decode)?;
        match tag {
            1 => Ok(Self::ScoopLir {
                member,
                units: decoder.field(2, decode_persistent_id_array)?,
            }),
            2 => Ok(Self::GeneratedCBridge {
                member,
                units: decoder.field(2, decode_persistent_id_array)?,
            }),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DecodedDefinitionAtomRangeProjectionV1 {
    atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    atom_role: DefinitionAtomRole,
    section_ordinal: u8,
    start: u64,
    end: u64,
    padding_end: u64,
}

impl WireEncode for DecodedDefinitionAtomRangeProjectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.atom.encode(encoder)?;
        encoder.field(2)?;
        self.atom_role.encode(encoder)?;
        encoder.field(3)?;
        encoder.unsigned(u64::from(self.section_ordinal))?;
        encoder.field(4)?;
        encoder.unsigned(self.start)?;
        encoder.field(5)?;
        encoder.unsigned(self.end)?;
        encoder.field(6)?;
        encoder.unsigned(self.padding_end)
    }
}

impl WireDecode for DecodedDefinitionAtomRangeProjectionV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            atom: decoder.field(1, DecodedPersistentId::decode)?,
            atom_role: decoder.field(2, DefinitionAtomRole::decode)?,
            section_ordinal: decoder.field(3, decode_nonzero_u8)?,
            start: decoder.field(4, Decoder::unsigned)?,
            end: decoder.field(5, Decoder::unsigned)?,
            padding_end: decoder.field(6, Decoder::unsigned)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedObjectDefinitionIndexV1 {
    member: DecodedFixedBytesV1<SlibMemberId>,
    definition: DecodedPersistentId<ObjectDefinitionPlanId>,
    primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    primary_symbol_table_index: u32,
    atoms: Vec<DecodedDefinitionAtomRangeProjectionV1>,
}

impl WireEncode for DecodedObjectDefinitionIndexV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.member.encode(encoder)?;
        encoder.field(2)?;
        self.definition.encode(encoder)?;
        encoder.field(3)?;
        self.primary_atom.encode(encoder)?;
        encoder.field(4)?;
        encoder.unsigned(u64::from(self.primary_symbol_table_index))?;
        encoder.field(5)?;
        encode_array(encoder, &self.atoms)
    }
}

impl WireDecode for DecodedObjectDefinitionIndexV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        Ok(Self {
            member: decoder.field(1, DecodedFixedBytesV1::decode)?,
            definition: decoder.field(2, DecodedPersistentId::decode)?,
            primary_atom: decoder.field(3, DecodedPersistentId::decode)?,
            primary_symbol_table_index: decoder.field(4, Decoder::u32)?,
            atoms: decoder.field(5, |decoder| {
                decoder.decode_array(|decoder, _| {
                    DecodedDefinitionAtomRangeProjectionV1::decode(decoder)
                })
            })?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DecodedMaterializedPatchSiteV1 {
    intent: DecodedPersistentId<DigestPatchIntentId>,
    member: DecodedFixedBytesV1<SlibMemberId>,
    checked_offset: u64,
}

impl WireEncode for DecodedMaterializedPatchSiteV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.intent.encode(encoder)?;
        encoder.field(2)?;
        self.member.encode(encoder)?;
        encoder.field(3)?;
        encoder.unsigned(self.checked_offset)
    }
}

impl WireDecode for DecodedMaterializedPatchSiteV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            intent: decoder.field(1, DecodedPersistentId::decode)?,
            member: decoder.field(2, DecodedFixedBytesV1::decode)?,
            checked_offset: decoder.field(3, Decoder::unsigned)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DecodedImageOwnerProjectionV1 {
    member: DecodedFixedBytesV1<SlibMemberId>,
    definition: DecodedPersistentId<ObjectDefinitionPlanId>,
    primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    primary_symbol_table_index: u32,
    checked_offset: u64,
    byte_size: u64,
}

impl WireEncode for DecodedImageOwnerProjectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.member.encode(encoder)?;
        encoder.field(2)?;
        self.definition.encode(encoder)?;
        encoder.field(3)?;
        self.primary_atom.encode(encoder)?;
        encoder.field(4)?;
        encoder.unsigned(u64::from(self.primary_symbol_table_index))?;
        encoder.field(5)?;
        encoder.unsigned(self.checked_offset)?;
        encoder.field(6)?;
        encoder.unsigned(self.byte_size)
    }
}

impl WireDecode for DecodedImageOwnerProjectionV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            member: decoder.field(1, DecodedFixedBytesV1::decode)?,
            definition: decoder.field(2, DecodedPersistentId::decode)?,
            primary_atom: decoder.field(3, DecodedPersistentId::decode)?,
            primary_symbol_table_index: decoder.field(4, Decoder::u32)?,
            checked_offset: decoder.field(5, Decoder::unsigned)?,
            byte_size: decoder.field(6, Decoder::unsigned)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DecodedEntryOwnerProjectionV1 {
    member: DecodedFixedBytesV1<SlibMemberId>,
    definition: DecodedPersistentId<ObjectDefinitionPlanId>,
    checked_offset: u64,
}

impl WireEncode for DecodedEntryOwnerProjectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.member.encode(encoder)?;
        encoder.field(2)?;
        self.definition.encode(encoder)?;
        encoder.field(3)?;
        encoder.unsigned(self.checked_offset)
    }
}

impl WireDecode for DecodedEntryOwnerProjectionV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            member: decoder.field(1, DecodedFixedBytesV1::decode)?,
            definition: decoder.field(2, DecodedPersistentId::decode)?,
            checked_offset: decoder.field(3, Decoder::unsigned)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DecodedEntryOwnerBranchV1 {
    Library,
    Executable(DecodedEntryOwnerProjectionV1),
}

impl WireEncode for DecodedEntryOwnerBranchV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Library => {
                encoder.map(1)?;
                encode_tag(encoder, 1)
            }
            Self::Executable(entry) => {
                encoder.map(2)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                entry.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedEntryOwnerBranchV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Library)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedEntryOwnerProjectionV1::decode)
                    .map(Self::Executable)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

/// Canonically decoded closure fields without any authority to promote the
/// carried identities or fingerprints.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedLinkIdentityClosureSectionV1 {
    materializations: Vec<DecodedLinkObjectMaterializationV1>,
    definition_indexes: Vec<DecodedObjectDefinitionIndexV1>,
    patch_sites: Vec<DecodedMaterializedPatchSiteV1>,
    defined_symbols: DecodedCanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_symbols: DecodedCanonicalUndefinedSymbolRequirementSetV1,
    verified_link_objects: DecodedCodeLinkObjectMemberSetV1,
    image_owner: DecodedImageOwnerProjectionV1,
    entry_owner: DecodedEntryOwnerBranchV1,
}

/// A decoded closure whose materialization array has been rebuilt from one
/// typed producer partition. The remaining closure fields are still
/// untrusted until the complete Code proof is available.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializationCheckedLinkIdentityClosureSectionV1 {
    decoded: DecodedLinkIdentityClosureSectionV1,
    member_plan: PlannedLinkObjectMemberSetV1,
}

/// A decoded closure whose digest patch inputs were matched to the validated
/// digest plan and checked materialization plan. Physical object validation is
/// still required before these inputs become verified patch sites.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DigestPatchInputCheckedLinkIdentityClosureSectionV1 {
    decoded: DecodedLinkIdentityClosureSectionV1,
    member_plan: PlannedLinkObjectMemberSetV1,
    provisional_patch_sites: Vec<ProvisionalDigestPatchSiteV1>,
}

/// A decoded closure whose first three fields are canonical projections of
/// the checked member plan and exact verified object/digest proofs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectProjectionCheckedLinkIdentityClosureSectionV1 {
    decoded: DecodedLinkIdentityClosureSectionV1,
    member_plan: PlannedLinkObjectMemberSetV1,
}

/// A decoded closure whose object projections and both canonical link-symbol
/// tables were independently rebuilt and matched byte-for-byte.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymbolProjectionCheckedLinkIdentityClosureSectionV1 {
    decoded: DecodedLinkIdentityClosureSectionV1,
    member_plan: PlannedLinkObjectMemberSetV1,
}

impl MaterializationCheckedLinkIdentityClosureSectionV1 {
    pub const fn member_plan(&self) -> &PlannedLinkObjectMemberSetV1 {
        &self.member_plan
    }

    pub fn validate_digest_patch_inputs(
        self,
        digest_plan: &DigestFinalizationPlanV1,
    ) -> Result<
        DigestPatchInputCheckedLinkIdentityClosureSectionV1,
        LinkDigestPatchInputValidationError,
    > {
        let sites = self
            .decoded
            .replay_digest_patch_inputs(&self.member_plan, digest_plan)?;

        Ok(DigestPatchInputCheckedLinkIdentityClosureSectionV1 {
            decoded: self.decoded,
            member_plan: self.member_plan,
            provisional_patch_sites: sites,
        })
    }

    pub fn validate(
        self,
        code: &VerifiedCodeFingerprintV1,
    ) -> Result<LinkIdentityClosureSectionV1, LinkIdentityClosureSectionValidationError> {
        let expected = LinkIdentityClosureSectionV1::from_verified_code(code)
            .map_err(LinkIdentityClosureSectionValidationError::Expected)?;
        validate_against(self.decoded, &expected)
    }
}

impl DigestPatchInputCheckedLinkIdentityClosureSectionV1 {
    pub const fn member_plan(&self) -> &PlannedLinkObjectMemberSetV1 {
        &self.member_plan
    }

    pub fn provisional_patch_sites(&self) -> &[ProvisionalDigestPatchSiteV1] {
        &self.provisional_patch_sites
    }

    pub fn validate_object_projections(
        self,
        patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    ) -> Result<
        ObjectProjectionCheckedLinkIdentityClosureSectionV1,
        LinkObjectProjectionValidationError,
    > {
        self.decoded
            .replay_object_projections(&self.member_plan, patch_sites)?;
        Ok(ObjectProjectionCheckedLinkIdentityClosureSectionV1 {
            decoded: self.decoded,
            member_plan: self.member_plan,
        })
    }

    pub fn validate(
        self,
        code: &VerifiedCodeFingerprintV1,
    ) -> Result<LinkIdentityClosureSectionV1, LinkIdentityClosureSectionValidationError> {
        let expected = LinkIdentityClosureSectionV1::from_verified_code(code)
            .map_err(LinkIdentityClosureSectionValidationError::Expected)?;
        validate_against(self.decoded, &expected)
    }
}

impl ObjectProjectionCheckedLinkIdentityClosureSectionV1 {
    pub const fn member_plan(&self) -> &PlannedLinkObjectMemberSetV1 {
        &self.member_plan
    }

    pub fn validate_symbol_projections(
        self,
        defined_symbols: &CanonicalDefinedLinkSymbolOwnerSetV1,
        undefined_symbols: &CanonicalUndefinedSymbolRequirementSetV1,
    ) -> Result<
        SymbolProjectionCheckedLinkIdentityClosureSectionV1,
        LinkSymbolProjectionValidationError,
    > {
        self.decoded
            .replay_symbol_projections(defined_symbols, undefined_symbols)?;
        Ok(SymbolProjectionCheckedLinkIdentityClosureSectionV1 {
            decoded: self.decoded,
            member_plan: self.member_plan,
        })
    }

    pub fn validate(
        self,
        code: &VerifiedCodeFingerprintV1,
    ) -> Result<LinkIdentityClosureSectionV1, LinkIdentityClosureSectionValidationError> {
        let expected = LinkIdentityClosureSectionV1::from_verified_code(code)
            .map_err(LinkIdentityClosureSectionValidationError::Expected)?;
        validate_against(self.decoded, &expected)
    }
}

impl SymbolProjectionCheckedLinkIdentityClosureSectionV1 {
    pub const fn member_plan(&self) -> &PlannedLinkObjectMemberSetV1 {
        &self.member_plan
    }

    pub fn validate(
        self,
        code: &VerifiedCodeFingerprintV1,
    ) -> Result<LinkIdentityClosureSectionV1, LinkIdentityClosureSectionValidationError> {
        let expected = LinkIdentityClosureSectionV1::from_verified_code(code)
            .map_err(LinkIdentityClosureSectionValidationError::Expected)?;
        validate_against(self.decoded, &expected)
    }
}

impl DecodedLinkIdentityClosureSectionV1 {
    pub fn validate_materializations(
        self,
        target: scoop_lir::LirTargetProfile,
        partition: &ProducerUnitPartitionV1,
    ) -> Result<
        MaterializationCheckedLinkIdentityClosureSectionV1,
        LinkObjectMaterializationValidationError,
    > {
        let member_plan = self.replay_materializations(target, partition)?;
        Ok(MaterializationCheckedLinkIdentityClosureSectionV1 {
            decoded: self,
            member_plan,
        })
    }

    /// Rebuilds the entire closure from the same verified Code/object proof
    /// and only promotes the decoded payload after exact byte equality.
    pub fn validate(
        self,
        code: &VerifiedCodeFingerprintV1,
    ) -> Result<LinkIdentityClosureSectionV1, LinkIdentityClosureSectionValidationError> {
        let expected = LinkIdentityClosureSectionV1::from_verified_code(code)
            .map_err(LinkIdentityClosureSectionValidationError::Expected)?;
        validate_against(self, &expected)
    }

    /// Rebuilds the unchanged wire projection from a Strong V2 layout Code
    /// proof and promotes it only after exact canonical equality.
    pub fn validate_layout(
        self,
        code: &VerifiedCodeFingerprintV2,
    ) -> Result<LinkIdentityClosureSectionV1, LinkIdentityClosureSectionValidationError> {
        let expected = LinkIdentityClosureSectionV1::from_verified_layout_code(code)
            .map_err(LinkIdentityClosureSectionValidationError::Expected)?;
        validate_against(self, &expected)
    }
}

impl WireEncode for DecodedLinkIdentityClosureSectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
        encoder.field(1)?;
        encode_array(encoder, &self.materializations)?;
        encoder.field(2)?;
        encode_array(encoder, &self.definition_indexes)?;
        encoder.field(3)?;
        encode_array(encoder, &self.patch_sites)?;
        encoder.field(4)?;
        self.defined_symbols.encode(encoder)?;
        encoder.field(5)?;
        self.undefined_symbols.encode(encoder)?;
        encoder.field(6)?;
        self.verified_link_objects.encode(encoder)?;
        encoder.field(7)?;
        self.image_owner.encode(encoder)?;
        encoder.field(8)?;
        self.entry_owner.encode(encoder)
    }
}

impl WireDecode for DecodedLinkIdentityClosureSectionV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(8)?;
        Ok(Self {
            materializations: decoder.field(1, |decoder| {
                decoder
                    .decode_array(|decoder, _| DecodedLinkObjectMaterializationV1::decode(decoder))
            })?,
            definition_indexes: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| DecodedObjectDefinitionIndexV1::decode(decoder))
            })?,
            patch_sites: decoder.field(3, |decoder| {
                decoder.decode_array(|decoder, _| DecodedMaterializedPatchSiteV1::decode(decoder))
            })?,
            defined_symbols: decoder
                .field(4, DecodedCanonicalDefinedLinkSymbolOwnerSetV1::decode)?,
            undefined_symbols: decoder
                .field(5, DecodedCanonicalUndefinedSymbolRequirementSetV1::decode)?,
            verified_link_objects: decoder.field(6, DecodedCodeLinkObjectMemberSetV1::decode)?,
            image_owner: decoder.field(7, DecodedImageOwnerProjectionV1::decode)?,
            entry_owner: decoder.field(8, DecodedEntryOwnerBranchV1::decode)?,
        })
    }
}

fn validate_against(
    actual: DecodedLinkIdentityClosureSectionV1,
    expected: &LinkIdentityClosureSectionV1,
) -> Result<LinkIdentityClosureSectionV1, LinkIdentityClosureSectionValidationError> {
    let actual = encode(&actual).map_err(LinkIdentityClosureSectionValidationError::Encode)?;
    let expected_bytes =
        encode(expected).map_err(LinkIdentityClosureSectionValidationError::Encode)?;
    if actual != expected_bytes {
        return Err(LinkIdentityClosureSectionValidationError::ProjectionMismatch);
    }
    Ok(expected.clone())
}

fn decode_persistent_id_array<I: PersistentId>(
    decoder: &mut Decoder<'_>,
) -> Result<Vec<DecodedPersistentId<I>>, WireError> {
    decoder.decode_array(|decoder, _| DecodedPersistentId::decode(decoder))
}

fn decode_nonzero_u8(decoder: &mut Decoder<'_>) -> Result<u8, WireError> {
    let value = decoder.unsigned()?;
    let value =
        u8::try_from(value).map_err(|_| wire_error(decoder, WireErrorKind::IntegerOutOfRange))?;
    if value == 0 {
        return Err(wire_error(decoder, WireErrorKind::IntegerOutOfRange));
    }
    Ok(value)
}

fn encode_array(
    encoder: &mut Encoder,
    values: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

struct WireArray<'values, T>(&'values [T]);

impl<T: WireEncode> WireEncode for WireArray<'_, T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, self.0)
    }
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn expect_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
pub(crate) mod tests;
