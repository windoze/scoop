//! Untrusted wire carriers for complete strong registration production.

use scoop_identity::{
    DecodedPersistentId, DecodedPersistentSymbolRequest, DigestNodeId, DigestPatchIntentId,
    ObjectDefinitionAtomId, ObjectDefinitionPlanId, PersistentCallableBodyId,
    PersistentExactTypeId, PersistentImmortalObjectId, PersistentInitializationUnitId,
    PersistentLayoutId, PersistentSafepointSiteId, PersistentScanId, PersistentStaticStorageId,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::DecodedStrongRegistrationIdentitySurfaceV1;

#[derive(Debug)]
pub struct DecodedStrongRegistrationProductionSurfaceV1 {
    pub(super) identities: DecodedStrongRegistrationIdentitySurfaceV1,
    pub(super) safepoints: Vec<DecodedStrongSafepointRegistrationPlanV1>,
    pub(super) callables: Vec<DecodedStrongCallableRegistrationPlanV1>,
    pub(super) types: Vec<DecodedStrongTypeRegistrationPlanV1>,
    pub(super) immortal_objects: Vec<DecodedStrongImmortalObjectRegistrationPlanV1>,
    pub(super) static_storages: Vec<DecodedStrongStaticStorageRegistrationPlanV1>,
    pub(super) initialization_units: Vec<DecodedStrongInitializationUnitRegistrationPlanV1>,
}

impl WireEncode for DecodedStrongRegistrationProductionSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(7)?;
        encode_field(encoder, 1, &self.identities)?;
        encode_array_field(encoder, 2, &self.safepoints)?;
        encode_array_field(encoder, 3, &self.callables)?;
        encode_array_field(encoder, 4, &self.types)?;
        encode_array_field(encoder, 5, &self.immortal_objects)?;
        encode_array_field(encoder, 6, &self.static_storages)?;
        encode_array_field(encoder, 7, &self.initialization_units)
    }
}

impl WireDecode for DecodedStrongRegistrationProductionSurfaceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(7)?;
        Ok(Self {
            identities: decoder.field(1, DecodedStrongRegistrationIdentitySurfaceV1::decode)?,
            safepoints: decode_array_field(
                decoder,
                2,
                DecodedStrongSafepointRegistrationPlanV1::decode,
            )?,
            callables: decode_array_field(
                decoder,
                3,
                DecodedStrongCallableRegistrationPlanV1::decode,
            )?,
            types: decode_array_field(decoder, 4, DecodedStrongTypeRegistrationPlanV1::decode)?,
            immortal_objects: decode_array_field(
                decoder,
                5,
                DecodedStrongImmortalObjectRegistrationPlanV1::decode,
            )?,
            static_storages: decode_array_field(
                decoder,
                6,
                DecodedStrongStaticStorageRegistrationPlanV1::decode,
            )?,
            initialization_units: decode_array_field(
                decoder,
                7,
                DecodedStrongInitializationUnitRegistrationPlanV1::decode,
            )?,
        })
    }
}

#[derive(Debug)]
pub struct DecodedStrongSafepointRegistrationPlanV1 {
    pub(super) site: DecodedPersistentId<PersistentSafepointSiteId>,
    pub(super) safepoint: u64,
    pub(super) owner: DecodedPersistentId<PersistentCallableBodyId>,
    pub(super) role: u32,
    pub(super) root_pair_count: u32,
    symbol: DecodedPersistentSymbolRequest,
    definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    registration_fingerprint_node: DecodedPersistentId<DigestNodeId>,
    normalized_stackmap_fingerprint_node: DecodedPersistentId<DigestNodeId>,
    registration_definition_patch: DecodedPersistentId<DigestPatchIntentId>,
    normalized_stackmap_patch: DecodedPersistentId<DigestPatchIntentId>,
}

impl WireEncode for DecodedStrongSafepointRegistrationPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(12)?;
        encode_field(encoder, 1, &self.site)?;
        encode_unsigned_field(encoder, 2, self.safepoint)?;
        encode_field(encoder, 3, &self.owner)?;
        encode_unsigned_field(encoder, 4, u64::from(self.role))?;
        encode_unsigned_field(encoder, 5, u64::from(self.root_pair_count))?;
        encode_field(encoder, 6, &self.symbol)?;
        encode_field(encoder, 7, &self.definition_plan)?;
        encode_field(encoder, 8, &self.primary_atom)?;
        encode_field(encoder, 9, &self.registration_fingerprint_node)?;
        encode_field(encoder, 10, &self.normalized_stackmap_fingerprint_node)?;
        encode_field(encoder, 11, &self.registration_definition_patch)?;
        encode_field(encoder, 12, &self.normalized_stackmap_patch)
    }
}

impl WireDecode for DecodedStrongSafepointRegistrationPlanV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(12)?;
        Ok(Self {
            site: decoder.field(1, DecodedPersistentId::decode)?,
            safepoint: decoder.field(2, Decoder::unsigned)?,
            owner: decoder.field(3, DecodedPersistentId::decode)?,
            role: decoder.field(4, Decoder::u32)?,
            root_pair_count: decoder.field(5, Decoder::u32)?,
            symbol: decoder.field(6, DecodedPersistentSymbolRequest::decode)?,
            definition_plan: decoder.field(7, DecodedPersistentId::decode)?,
            primary_atom: decoder.field(8, DecodedPersistentId::decode)?,
            registration_fingerprint_node: decoder.field(9, DecodedPersistentId::decode)?,
            normalized_stackmap_fingerprint_node: decoder.field(10, DecodedPersistentId::decode)?,
            registration_definition_patch: decoder.field(11, DecodedPersistentId::decode)?,
            normalized_stackmap_patch: decoder.field(12, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Debug)]
pub struct DecodedStrongCallableRegistrationPlanV1 {
    body: DecodedPersistentId<PersistentCallableBodyId>,
    symbol: DecodedPersistentSymbolRequest,
    definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    entry_symbol: DecodedPersistentSymbolRequest,
    body_definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    body_primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    registration_object_node: DecodedPersistentId<DigestNodeId>,
    body_definition_node: DecodedPersistentId<DigestNodeId>,
    registration_fingerprint_node: DecodedPersistentId<DigestNodeId>,
    registration_definition_patch: DecodedPersistentId<DigestPatchIntentId>,
    body_definition_patch: DecodedPersistentId<DigestPatchIntentId>,
}

impl WireEncode for DecodedStrongCallableRegistrationPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(12)?;
        encode_field(encoder, 1, &self.body)?;
        encode_field(encoder, 2, &self.symbol)?;
        encode_field(encoder, 3, &self.definition_plan)?;
        encode_field(encoder, 4, &self.primary_atom)?;
        encode_field(encoder, 5, &self.entry_symbol)?;
        encode_field(encoder, 6, &self.body_definition_plan)?;
        encode_field(encoder, 7, &self.body_primary_atom)?;
        encode_field(encoder, 8, &self.registration_object_node)?;
        encode_field(encoder, 9, &self.body_definition_node)?;
        encode_field(encoder, 10, &self.registration_fingerprint_node)?;
        encode_field(encoder, 11, &self.registration_definition_patch)?;
        encode_field(encoder, 12, &self.body_definition_patch)
    }
}

impl WireDecode for DecodedStrongCallableRegistrationPlanV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(12)?;
        Ok(Self {
            body: decoder.field(1, DecodedPersistentId::decode)?,
            symbol: decoder.field(2, DecodedPersistentSymbolRequest::decode)?,
            definition_plan: decoder.field(3, DecodedPersistentId::decode)?,
            primary_atom: decoder.field(4, DecodedPersistentId::decode)?,
            entry_symbol: decoder.field(5, DecodedPersistentSymbolRequest::decode)?,
            body_definition_plan: decoder.field(6, DecodedPersistentId::decode)?,
            body_primary_atom: decoder.field(7, DecodedPersistentId::decode)?,
            registration_object_node: decoder.field(8, DecodedPersistentId::decode)?,
            body_definition_node: decoder.field(9, DecodedPersistentId::decode)?,
            registration_fingerprint_node: decoder.field(10, DecodedPersistentId::decode)?,
            registration_definition_patch: decoder.field(11, DecodedPersistentId::decode)?,
            body_definition_patch: decoder.field(12, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Debug)]
pub struct DecodedStrongTypeRegistrationPlanV1 {
    exact_type: DecodedPersistentId<PersistentExactTypeId>,
    runtime_type: u64,
    symbol: DecodedPersistentSymbolRequest,
    definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    descriptor_symbol: DecodedPersistentSymbolRequest,
    descriptor_definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    descriptor_primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    layout: DecodedPersistentId<PersistentLayoutId>,
    layout_symbol: DecodedPersistentSymbolRequest,
    layout_definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    layout_primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    registration_object_node: DecodedPersistentId<DigestNodeId>,
    descriptor_definition_node: DecodedPersistentId<DigestNodeId>,
    layout_fingerprint_node: DecodedPersistentId<DigestNodeId>,
    registration_fingerprint_node: DecodedPersistentId<DigestNodeId>,
    registration_definition_patch: DecodedPersistentId<DigestPatchIntentId>,
    descriptor_definition_patch: DecodedPersistentId<DigestPatchIntentId>,
    layout_fingerprint_patch: DecodedPersistentId<DigestPatchIntentId>,
}

impl WireEncode for DecodedStrongTypeRegistrationPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(19)?;
        encode_field(encoder, 1, &self.exact_type)?;
        encode_unsigned_field(encoder, 2, self.runtime_type)?;
        encode_field(encoder, 3, &self.symbol)?;
        encode_field(encoder, 4, &self.definition_plan)?;
        encode_field(encoder, 5, &self.primary_atom)?;
        encode_field(encoder, 6, &self.descriptor_symbol)?;
        encode_field(encoder, 7, &self.descriptor_definition_plan)?;
        encode_field(encoder, 8, &self.descriptor_primary_atom)?;
        encode_field(encoder, 9, &self.layout)?;
        encode_field(encoder, 10, &self.layout_symbol)?;
        encode_field(encoder, 11, &self.layout_definition_plan)?;
        encode_field(encoder, 12, &self.layout_primary_atom)?;
        encode_field(encoder, 13, &self.registration_object_node)?;
        encode_field(encoder, 14, &self.descriptor_definition_node)?;
        encode_field(encoder, 15, &self.layout_fingerprint_node)?;
        encode_field(encoder, 16, &self.registration_fingerprint_node)?;
        encode_field(encoder, 17, &self.registration_definition_patch)?;
        encode_field(encoder, 18, &self.descriptor_definition_patch)?;
        encode_field(encoder, 19, &self.layout_fingerprint_patch)
    }
}

impl WireDecode for DecodedStrongTypeRegistrationPlanV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(19)?;
        Ok(Self {
            exact_type: decoder.field(1, DecodedPersistentId::decode)?,
            runtime_type: decoder.field(2, Decoder::unsigned)?,
            symbol: decoder.field(3, DecodedPersistentSymbolRequest::decode)?,
            definition_plan: decoder.field(4, DecodedPersistentId::decode)?,
            primary_atom: decoder.field(5, DecodedPersistentId::decode)?,
            descriptor_symbol: decoder.field(6, DecodedPersistentSymbolRequest::decode)?,
            descriptor_definition_plan: decoder.field(7, DecodedPersistentId::decode)?,
            descriptor_primary_atom: decoder.field(8, DecodedPersistentId::decode)?,
            layout: decoder.field(9, DecodedPersistentId::decode)?,
            layout_symbol: decoder.field(10, DecodedPersistentSymbolRequest::decode)?,
            layout_definition_plan: decoder.field(11, DecodedPersistentId::decode)?,
            layout_primary_atom: decoder.field(12, DecodedPersistentId::decode)?,
            registration_object_node: decoder.field(13, DecodedPersistentId::decode)?,
            descriptor_definition_node: decoder.field(14, DecodedPersistentId::decode)?,
            layout_fingerprint_node: decoder.field(15, DecodedPersistentId::decode)?,
            registration_fingerprint_node: decoder.field(16, DecodedPersistentId::decode)?,
            registration_definition_patch: decoder.field(17, DecodedPersistentId::decode)?,
            descriptor_definition_patch: decoder.field(18, DecodedPersistentId::decode)?,
            layout_fingerprint_patch: decoder.field(19, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Debug)]
pub(super) enum DecodedImmortalObjectTypeRegistrationRefV1 {
    Local(DecodedPersistentId<PersistentExactTypeId>),
    CoreExternal(DecodedPersistentId<PersistentExactTypeId>),
}

impl WireEncode for DecodedImmortalObjectTypeRegistrationRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Local(exact_type) => encode_value_sum(encoder, 1, exact_type),
            Self::CoreExternal(exact_type) => encode_value_sum(encoder, 2, exact_type),
        }
    }
}

impl WireDecode for DecodedImmortalObjectTypeRegistrationRefV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let exact_type = decoder.field(1, DecodedPersistentId::decode)?;
        match tag {
            1 => Ok(Self::Local(exact_type)),
            2 => Ok(Self::CoreExternal(exact_type)),
            _ => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Debug)]
pub struct DecodedStrongImmortalObjectRegistrationPlanV1 {
    pub(super) object: DecodedPersistentId<PersistentImmortalObjectId>,
    object_symbol: DecodedPersistentSymbolRequest,
    pub(super) object_size: u64,
    pub(super) required_alignment: u64,
    pub(super) type_registration: DecodedImmortalObjectTypeRegistrationRefV1,
    registration_symbol: DecodedPersistentSymbolRequest,
    registration_definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    registration_primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    object_definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    object_primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    type_registration_symbol: DecodedPersistentSymbolRequest,
    registration_object_node: DecodedPersistentId<DigestNodeId>,
    object_definition_node: DecodedPersistentId<DigestNodeId>,
    registration_fingerprint_node: DecodedPersistentId<DigestNodeId>,
    registration_definition_patch: DecodedPersistentId<DigestPatchIntentId>,
}

impl WireEncode for DecodedStrongImmortalObjectRegistrationPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(15)?;
        encode_field(encoder, 1, &self.object)?;
        encode_field(encoder, 2, &self.object_symbol)?;
        encode_unsigned_field(encoder, 3, self.object_size)?;
        encode_unsigned_field(encoder, 4, self.required_alignment)?;
        encode_field(encoder, 5, &self.type_registration)?;
        encode_field(encoder, 6, &self.registration_symbol)?;
        encode_field(encoder, 7, &self.registration_definition_plan)?;
        encode_field(encoder, 8, &self.registration_primary_atom)?;
        encode_field(encoder, 9, &self.object_definition_plan)?;
        encode_field(encoder, 10, &self.object_primary_atom)?;
        encode_field(encoder, 11, &self.type_registration_symbol)?;
        encode_field(encoder, 12, &self.registration_object_node)?;
        encode_field(encoder, 13, &self.object_definition_node)?;
        encode_field(encoder, 14, &self.registration_fingerprint_node)?;
        encode_field(encoder, 15, &self.registration_definition_patch)
    }
}

impl WireDecode for DecodedStrongImmortalObjectRegistrationPlanV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(15)?;
        Ok(Self {
            object: decoder.field(1, DecodedPersistentId::decode)?,
            object_symbol: decoder.field(2, DecodedPersistentSymbolRequest::decode)?,
            object_size: decoder.field(3, Decoder::unsigned)?,
            required_alignment: decoder.field(4, Decoder::unsigned)?,
            type_registration: decoder
                .field(5, DecodedImmortalObjectTypeRegistrationRefV1::decode)?,
            registration_symbol: decoder.field(6, DecodedPersistentSymbolRequest::decode)?,
            registration_definition_plan: decoder.field(7, DecodedPersistentId::decode)?,
            registration_primary_atom: decoder.field(8, DecodedPersistentId::decode)?,
            object_definition_plan: decoder.field(9, DecodedPersistentId::decode)?,
            object_primary_atom: decoder.field(10, DecodedPersistentId::decode)?,
            type_registration_symbol: decoder.field(11, DecodedPersistentSymbolRequest::decode)?,
            registration_object_node: decoder.field(12, DecodedPersistentId::decode)?,
            object_definition_node: decoder.field(13, DecodedPersistentId::decode)?,
            registration_fingerprint_node: decoder.field(14, DecodedPersistentId::decode)?,
            registration_definition_patch: decoder.field(15, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Debug)]
pub(super) enum DecodedRefScan {
    None,
    References(Vec<u64>),
    Sequence(Vec<Self>),
}

impl WireEncode for DecodedRefScan {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::None => encode_empty_sum(encoder, 1),
            Self::References(offsets) => {
                encoder.map(2)?;
                encode_unsigned_field(encoder, 0, 2)?;
                encoder.field(1)?;
                encoder.array(offsets.len() as u64)?;
                for offset in offsets {
                    encoder.unsigned(*offset)?;
                }
                Ok(())
            }
            Self::Sequence(parts) => {
                encoder.map(2)?;
                encode_unsigned_field(encoder, 0, 3)?;
                encode_array_field(encoder, 1, parts)
            }
        }
    }
}

impl WireDecode for DecodedRefScan {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let length = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                require_sum_length(decoder, length, 1)?;
                Ok(Self::None)
            }
            2 => {
                require_sum_length(decoder, length, 2)?;
                let offsets = decoder.field(1, |decoder| {
                    decoder.decode_array(|decoder, _| decoder.unsigned())
                })?;
                Ok(Self::References(offsets))
            }
            3 => {
                require_sum_length(decoder, length, 2)?;
                let parts = decoder.field(1, |decoder| {
                    decoder.decode_array(|decoder, _| Self::decode(decoder))
                })?;
                Ok(Self::Sequence(parts))
            }
            _ => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Debug)]
pub(super) struct DecodedStaticImmortalRelocationPlanV1 {
    pub(super) pointer_offset: u64,
    pub(super) target: DecodedPersistentId<PersistentImmortalObjectId>,
}

impl WireEncode for DecodedStaticImmortalRelocationPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encode_unsigned_field(encoder, 1, self.pointer_offset)?;
        encode_field(encoder, 2, &self.target)
    }
}

impl WireDecode for DecodedStaticImmortalRelocationPlanV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            pointer_offset: decoder.field(1, Decoder::unsigned)?,
            target: decoder.field(2, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Debug)]
pub(super) enum DecodedStrongStaticStorageInitialStatePlanV1 {
    ZeroedForRuntimeUnit,
    EncodedStaticValue {
        initial_template: Vec<u8>,
        immortal_relocations: Vec<DecodedStaticImmortalRelocationPlanV1>,
    },
}

impl WireEncode for DecodedStrongStaticStorageInitialStatePlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ZeroedForRuntimeUnit => encode_empty_sum(encoder, 1),
            Self::EncodedStaticValue {
                initial_template,
                immortal_relocations,
            } => {
                encoder.map(3)?;
                encode_unsigned_field(encoder, 0, 2)?;
                encoder.field(1)?;
                encoder.bytes(initial_template)?;
                encode_array_field(encoder, 2, immortal_relocations)
            }
        }
    }
}

impl WireDecode for DecodedStrongStaticStorageInitialStatePlanV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let length = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                require_sum_length(decoder, length, 1)?;
                Ok(Self::ZeroedForRuntimeUnit)
            }
            2 => {
                require_sum_length(decoder, length, 3)?;
                let initial_template = decoder.field(1, Decoder::owned_bytes)?;
                let immortal_relocations = decoder.field(2, |decoder| {
                    decoder.decode_array(|decoder, _| {
                        DecodedStaticImmortalRelocationPlanV1::decode(decoder)
                    })
                })?;
                Ok(Self::EncodedStaticValue {
                    initial_template,
                    immortal_relocations,
                })
            }
            _ => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Debug)]
enum DecodedStaticStorageRelocationTableArtifactV1 {
    SharedEmptySentinel,
    Defined(DecodedPersistentId<ObjectDefinitionAtomId>),
}

impl WireEncode for DecodedStaticStorageRelocationTableArtifactV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::SharedEmptySentinel => encode_empty_sum(encoder, 1),
            Self::Defined(atom) => encode_value_sum(encoder, 2, atom),
        }
    }
}

impl WireDecode for DecodedStaticStorageRelocationTableArtifactV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let length = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                require_sum_length(decoder, length, 1)?;
                Ok(Self::SharedEmptySentinel)
            }
            2 => {
                require_sum_length(decoder, length, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::Defined)
            }
            _ => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Debug)]
enum DecodedStrongStaticStorageInitialArtifactPlanV1 {
    ZeroedForRuntimeUnit,
    EncodedStaticValue {
        template_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
        relocation_table: DecodedStaticStorageRelocationTableArtifactV1,
    },
}

impl WireEncode for DecodedStrongStaticStorageInitialArtifactPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ZeroedForRuntimeUnit => encode_empty_sum(encoder, 1),
            Self::EncodedStaticValue {
                template_atom,
                relocation_table,
            } => {
                encoder.map(3)?;
                encode_unsigned_field(encoder, 0, 2)?;
                encode_field(encoder, 1, template_atom)?;
                encode_field(encoder, 2, relocation_table)
            }
        }
    }
}

impl WireDecode for DecodedStrongStaticStorageInitialArtifactPlanV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let length = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                require_sum_length(decoder, length, 1)?;
                Ok(Self::ZeroedForRuntimeUnit)
            }
            2 => {
                require_sum_length(decoder, length, 3)?;
                Ok(Self::EncodedStaticValue {
                    template_atom: decoder.field(1, DecodedPersistentId::decode)?,
                    relocation_table: decoder
                        .field(2, DecodedStaticStorageRelocationTableArtifactV1::decode)?,
                })
            }
            _ => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Debug)]
pub struct DecodedStrongStaticStorageRegistrationPlanV1 {
    pub(super) storage: DecodedPersistentId<PersistentStaticStorageId>,
    storage_symbol: DecodedPersistentSymbolRequest,
    pub(super) layout: DecodedPersistentId<PersistentLayoutId>,
    pub(super) scan: DecodedPersistentId<PersistentScanId>,
    pub(super) scan_program: DecodedRefScan,
    pub(super) scan_kind: u32,
    pub(super) byte_size: u64,
    pub(super) allocation_extent: u64,
    pub(super) required_alignment: u64,
    pub(super) initial_state: DecodedStrongStaticStorageInitialStatePlanV1,
    registration_symbol: DecodedPersistentSymbolRequest,
    registration_definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    registration_primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    storage_definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    storage_primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    initial_artifacts: DecodedStrongStaticStorageInitialArtifactPlanV1,
    immortal_registration_symbols: Vec<DecodedPersistentSymbolRequest>,
    layout_symbol: DecodedPersistentSymbolRequest,
    layout_definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    layout_primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    scan_symbol: DecodedPersistentSymbolRequest,
    scan_definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    scan_primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    registration_object_node: DecodedPersistentId<DigestNodeId>,
    storage_definition_node: DecodedPersistentId<DigestNodeId>,
    layout_fingerprint_node: DecodedPersistentId<DigestNodeId>,
    scan_fingerprint_node: DecodedPersistentId<DigestNodeId>,
    registration_fingerprint_node: DecodedPersistentId<DigestNodeId>,
    registration_definition_patch: DecodedPersistentId<DigestPatchIntentId>,
    layout_fingerprint_patch: DecodedPersistentId<DigestPatchIntentId>,
    scan_fingerprint_patch: DecodedPersistentId<DigestPatchIntentId>,
}

impl WireEncode for DecodedStrongStaticStorageRegistrationPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(31)?;
        encode_field(encoder, 1, &self.storage)?;
        encode_field(encoder, 2, &self.storage_symbol)?;
        encode_field(encoder, 3, &self.layout)?;
        encode_field(encoder, 4, &self.scan)?;
        encode_field(encoder, 5, &self.scan_program)?;
        encode_unsigned_field(encoder, 6, u64::from(self.scan_kind))?;
        encode_unsigned_field(encoder, 7, self.byte_size)?;
        encode_unsigned_field(encoder, 8, self.allocation_extent)?;
        encode_unsigned_field(encoder, 9, self.required_alignment)?;
        encode_field(encoder, 10, &self.initial_state)?;
        encode_field(encoder, 11, &self.registration_symbol)?;
        encode_field(encoder, 12, &self.registration_definition_plan)?;
        encode_field(encoder, 13, &self.registration_primary_atom)?;
        encode_field(encoder, 14, &self.storage_definition_plan)?;
        encode_field(encoder, 15, &self.storage_primary_atom)?;
        encode_field(encoder, 16, &self.initial_artifacts)?;
        encode_array_field(encoder, 17, &self.immortal_registration_symbols)?;
        encode_field(encoder, 18, &self.layout_symbol)?;
        encode_field(encoder, 19, &self.layout_definition_plan)?;
        encode_field(encoder, 20, &self.layout_primary_atom)?;
        encode_field(encoder, 21, &self.scan_symbol)?;
        encode_field(encoder, 22, &self.scan_definition_plan)?;
        encode_field(encoder, 23, &self.scan_primary_atom)?;
        encode_field(encoder, 24, &self.registration_object_node)?;
        encode_field(encoder, 25, &self.storage_definition_node)?;
        encode_field(encoder, 26, &self.layout_fingerprint_node)?;
        encode_field(encoder, 27, &self.scan_fingerprint_node)?;
        encode_field(encoder, 28, &self.registration_fingerprint_node)?;
        encode_field(encoder, 29, &self.registration_definition_patch)?;
        encode_field(encoder, 30, &self.layout_fingerprint_patch)?;
        encode_field(encoder, 31, &self.scan_fingerprint_patch)
    }
}

impl WireDecode for DecodedStrongStaticStorageRegistrationPlanV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(31)?;
        Ok(Self {
            storage: decoder.field(1, DecodedPersistentId::decode)?,
            storage_symbol: decoder.field(2, DecodedPersistentSymbolRequest::decode)?,
            layout: decoder.field(3, DecodedPersistentId::decode)?,
            scan: decoder.field(4, DecodedPersistentId::decode)?,
            scan_program: decoder.field(5, DecodedRefScan::decode)?,
            scan_kind: decoder.field(6, Decoder::u32)?,
            byte_size: decoder.field(7, Decoder::unsigned)?,
            allocation_extent: decoder.field(8, Decoder::unsigned)?,
            required_alignment: decoder.field(9, Decoder::unsigned)?,
            initial_state: decoder
                .field(10, DecodedStrongStaticStorageInitialStatePlanV1::decode)?,
            registration_symbol: decoder.field(11, DecodedPersistentSymbolRequest::decode)?,
            registration_definition_plan: decoder.field(12, DecodedPersistentId::decode)?,
            registration_primary_atom: decoder.field(13, DecodedPersistentId::decode)?,
            storage_definition_plan: decoder.field(14, DecodedPersistentId::decode)?,
            storage_primary_atom: decoder.field(15, DecodedPersistentId::decode)?,
            initial_artifacts: decoder
                .field(16, DecodedStrongStaticStorageInitialArtifactPlanV1::decode)?,
            immortal_registration_symbols: decoder.field(17, |decoder| {
                decoder.decode_array(|decoder, _| DecodedPersistentSymbolRequest::decode(decoder))
            })?,
            layout_symbol: decoder.field(18, DecodedPersistentSymbolRequest::decode)?,
            layout_definition_plan: decoder.field(19, DecodedPersistentId::decode)?,
            layout_primary_atom: decoder.field(20, DecodedPersistentId::decode)?,
            scan_symbol: decoder.field(21, DecodedPersistentSymbolRequest::decode)?,
            scan_definition_plan: decoder.field(22, DecodedPersistentId::decode)?,
            scan_primary_atom: decoder.field(23, DecodedPersistentId::decode)?,
            registration_object_node: decoder.field(24, DecodedPersistentId::decode)?,
            storage_definition_node: decoder.field(25, DecodedPersistentId::decode)?,
            layout_fingerprint_node: decoder.field(26, DecodedPersistentId::decode)?,
            scan_fingerprint_node: decoder.field(27, DecodedPersistentId::decode)?,
            registration_fingerprint_node: decoder.field(28, DecodedPersistentId::decode)?,
            registration_definition_patch: decoder.field(29, DecodedPersistentId::decode)?,
            layout_fingerprint_patch: decoder.field(30, DecodedPersistentId::decode)?,
            scan_fingerprint_patch: decoder.field(31, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Debug)]
pub(super) enum DecodedStrongInitializationSchedulePlanV1 {
    EagerStartup(DecodedPersistentId<PersistentCallableBodyId>),
    LazyAccess,
}

impl WireEncode for DecodedStrongInitializationSchedulePlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::EagerStartup(gateway) => encode_value_sum(encoder, 1, gateway),
            Self::LazyAccess => encode_empty_sum(encoder, 2),
        }
    }
}

impl WireDecode for DecodedStrongInitializationSchedulePlanV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let length = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                require_sum_length(decoder, length, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::EagerStartup)
            }
            2 => {
                require_sum_length(decoder, length, 1)?;
                Ok(Self::LazyAccess)
            }
            _ => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Debug)]
struct DecodedStrongInitializationStaticStorageRefPlanV1 {
    storage: DecodedPersistentId<PersistentStaticStorageId>,
    storage_symbol: DecodedPersistentSymbolRequest,
    registration_symbol: DecodedPersistentSymbolRequest,
    registration_definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    registration_primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    registration_fingerprint_node: DecodedPersistentId<DigestNodeId>,
}

impl WireEncode for DecodedStrongInitializationStaticStorageRefPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encode_field(encoder, 1, &self.storage)?;
        encode_field(encoder, 2, &self.storage_symbol)?;
        encode_field(encoder, 3, &self.registration_symbol)?;
        encode_field(encoder, 4, &self.registration_definition_plan)?;
        encode_field(encoder, 5, &self.registration_primary_atom)?;
        encode_field(encoder, 6, &self.registration_fingerprint_node)
    }
}

impl WireDecode for DecodedStrongInitializationStaticStorageRefPlanV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            storage: decoder.field(1, DecodedPersistentId::decode)?,
            storage_symbol: decoder.field(2, DecodedPersistentSymbolRequest::decode)?,
            registration_symbol: decoder.field(3, DecodedPersistentSymbolRequest::decode)?,
            registration_definition_plan: decoder.field(4, DecodedPersistentId::decode)?,
            registration_primary_atom: decoder.field(5, DecodedPersistentId::decode)?,
            registration_fingerprint_node: decoder.field(6, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Debug)]
struct DecodedStrongInitializationCallableRefPlanV1 {
    body: DecodedPersistentId<PersistentCallableBodyId>,
    entry_symbol: DecodedPersistentSymbolRequest,
    registration_symbol: DecodedPersistentSymbolRequest,
    body_definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    body_primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    body_definition_node: DecodedPersistentId<DigestNodeId>,
    registration_definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    registration_primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    registration_fingerprint_node: DecodedPersistentId<DigestNodeId>,
}

impl WireEncode for DecodedStrongInitializationCallableRefPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(9)?;
        encode_field(encoder, 1, &self.body)?;
        encode_field(encoder, 2, &self.entry_symbol)?;
        encode_field(encoder, 3, &self.registration_symbol)?;
        encode_field(encoder, 4, &self.body_definition_plan)?;
        encode_field(encoder, 5, &self.body_primary_atom)?;
        encode_field(encoder, 6, &self.body_definition_node)?;
        encode_field(encoder, 7, &self.registration_definition_plan)?;
        encode_field(encoder, 8, &self.registration_primary_atom)?;
        encode_field(encoder, 9, &self.registration_fingerprint_node)
    }
}

impl WireDecode for DecodedStrongInitializationCallableRefPlanV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(9)?;
        Ok(Self {
            body: decoder.field(1, DecodedPersistentId::decode)?,
            entry_symbol: decoder.field(2, DecodedPersistentSymbolRequest::decode)?,
            registration_symbol: decoder.field(3, DecodedPersistentSymbolRequest::decode)?,
            body_definition_plan: decoder.field(4, DecodedPersistentId::decode)?,
            body_primary_atom: decoder.field(5, DecodedPersistentId::decode)?,
            body_definition_node: decoder.field(6, DecodedPersistentId::decode)?,
            registration_definition_plan: decoder.field(7, DecodedPersistentId::decode)?,
            registration_primary_atom: decoder.field(8, DecodedPersistentId::decode)?,
            registration_fingerprint_node: decoder.field(9, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Debug)]
enum DecodedStrongInitializationRegistrationSchedulePlanV1 {
    EagerStartup {
        gateway: Box<DecodedStrongInitializationCallableRefPlanV1>,
        gateway_definition_patch: DecodedPersistentId<DigestPatchIntentId>,
    },
    LazyAccess,
}

impl WireEncode for DecodedStrongInitializationRegistrationSchedulePlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::EagerStartup {
                gateway,
                gateway_definition_patch,
            } => {
                encoder.map(3)?;
                encode_unsigned_field(encoder, 0, 1)?;
                encode_field(encoder, 1, gateway.as_ref())?;
                encode_field(encoder, 2, gateway_definition_patch)
            }
            Self::LazyAccess => encode_empty_sum(encoder, 2),
        }
    }
}

impl WireDecode for DecodedStrongInitializationRegistrationSchedulePlanV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let length = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                require_sum_length(decoder, length, 3)?;
                Ok(Self::EagerStartup {
                    gateway: Box::new(
                        decoder.field(1, DecodedStrongInitializationCallableRefPlanV1::decode)?,
                    ),
                    gateway_definition_patch: decoder.field(2, DecodedPersistentId::decode)?,
                })
            }
            2 => {
                require_sum_length(decoder, length, 1)?;
                Ok(Self::LazyAccess)
            }
            _ => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Debug)]
pub struct DecodedStrongInitializationUnitRegistrationPlanV1 {
    pub(super) unit: DecodedPersistentId<PersistentInitializationUnitId>,
    pub(super) diagnostic_path: String,
    pub(super) semantic_schedule: DecodedStrongInitializationSchedulePlanV1,
    pub(super) storage_id: DecodedPersistentId<PersistentStaticStorageId>,
    pub(super) failure_root_id: DecodedPersistentId<PersistentStaticStorageId>,
    pub(super) initializer_id: DecodedPersistentId<PersistentCallableBodyId>,
    pub(super) ensure_id: DecodedPersistentId<PersistentCallableBodyId>,
    pub(super) dependencies: Vec<DecodedPersistentId<PersistentInitializationUnitId>>,
    registration_symbol: DecodedPersistentSymbolRequest,
    registration_definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    registration_primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    cell_symbol: DecodedPersistentSymbolRequest,
    cell_definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    cell_primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    descriptor_symbol: DecodedPersistentSymbolRequest,
    descriptor_definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    descriptor_primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    diagnostic_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    storage: DecodedStrongInitializationStaticStorageRefPlanV1,
    failure_root: DecodedStrongInitializationStaticStorageRefPlanV1,
    initializer: DecodedStrongInitializationCallableRefPlanV1,
    ensure: DecodedStrongInitializationCallableRefPlanV1,
    registration_schedule: DecodedStrongInitializationRegistrationSchedulePlanV1,
    registration_object_node: DecodedPersistentId<DigestNodeId>,
    cell_definition_node: DecodedPersistentId<DigestNodeId>,
    descriptor_definition_node: DecodedPersistentId<DigestNodeId>,
    registration_fingerprint_node: DecodedPersistentId<DigestNodeId>,
    registration_definition_patch: DecodedPersistentId<DigestPatchIntentId>,
}

impl WireEncode for DecodedStrongInitializationUnitRegistrationPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(28)?;
        encode_field(encoder, 1, &self.unit)?;
        encoder.field(2)?;
        encoder.text(&self.diagnostic_path)?;
        encode_field(encoder, 3, &self.semantic_schedule)?;
        encode_field(encoder, 4, &self.storage_id)?;
        encode_field(encoder, 5, &self.failure_root_id)?;
        encode_field(encoder, 6, &self.initializer_id)?;
        encode_field(encoder, 7, &self.ensure_id)?;
        encode_array_field(encoder, 8, &self.dependencies)?;
        encode_field(encoder, 9, &self.registration_symbol)?;
        encode_field(encoder, 10, &self.registration_definition_plan)?;
        encode_field(encoder, 11, &self.registration_primary_atom)?;
        encode_field(encoder, 12, &self.cell_symbol)?;
        encode_field(encoder, 13, &self.cell_definition_plan)?;
        encode_field(encoder, 14, &self.cell_primary_atom)?;
        encode_field(encoder, 15, &self.descriptor_symbol)?;
        encode_field(encoder, 16, &self.descriptor_definition_plan)?;
        encode_field(encoder, 17, &self.descriptor_primary_atom)?;
        encode_field(encoder, 18, &self.diagnostic_atom)?;
        encode_field(encoder, 19, &self.storage)?;
        encode_field(encoder, 20, &self.failure_root)?;
        encode_field(encoder, 21, &self.initializer)?;
        encode_field(encoder, 22, &self.ensure)?;
        encode_field(encoder, 23, &self.registration_schedule)?;
        encode_field(encoder, 24, &self.registration_object_node)?;
        encode_field(encoder, 25, &self.cell_definition_node)?;
        encode_field(encoder, 26, &self.descriptor_definition_node)?;
        encode_field(encoder, 27, &self.registration_fingerprint_node)?;
        encode_field(encoder, 28, &self.registration_definition_patch)
    }
}

impl WireDecode for DecodedStrongInitializationUnitRegistrationPlanV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(28)?;
        Ok(Self {
            unit: decoder.field(1, DecodedPersistentId::decode)?,
            diagnostic_path: decoder.field(2, Decoder::owned_text)?,
            semantic_schedule: decoder
                .field(3, DecodedStrongInitializationSchedulePlanV1::decode)?,
            storage_id: decoder.field(4, DecodedPersistentId::decode)?,
            failure_root_id: decoder.field(5, DecodedPersistentId::decode)?,
            initializer_id: decoder.field(6, DecodedPersistentId::decode)?,
            ensure_id: decoder.field(7, DecodedPersistentId::decode)?,
            dependencies: decoder.field(8, |decoder| {
                decoder.decode_array(|decoder, _| DecodedPersistentId::decode(decoder))
            })?,
            registration_symbol: decoder.field(9, DecodedPersistentSymbolRequest::decode)?,
            registration_definition_plan: decoder.field(10, DecodedPersistentId::decode)?,
            registration_primary_atom: decoder.field(11, DecodedPersistentId::decode)?,
            cell_symbol: decoder.field(12, DecodedPersistentSymbolRequest::decode)?,
            cell_definition_plan: decoder.field(13, DecodedPersistentId::decode)?,
            cell_primary_atom: decoder.field(14, DecodedPersistentId::decode)?,
            descriptor_symbol: decoder.field(15, DecodedPersistentSymbolRequest::decode)?,
            descriptor_definition_plan: decoder.field(16, DecodedPersistentId::decode)?,
            descriptor_primary_atom: decoder.field(17, DecodedPersistentId::decode)?,
            diagnostic_atom: decoder.field(18, DecodedPersistentId::decode)?,
            storage: decoder.field(
                19,
                DecodedStrongInitializationStaticStorageRefPlanV1::decode,
            )?,
            failure_root: decoder.field(
                20,
                DecodedStrongInitializationStaticStorageRefPlanV1::decode,
            )?,
            initializer: decoder.field(21, DecodedStrongInitializationCallableRefPlanV1::decode)?,
            ensure: decoder.field(22, DecodedStrongInitializationCallableRefPlanV1::decode)?,
            registration_schedule: decoder.field(
                23,
                DecodedStrongInitializationRegistrationSchedulePlanV1::decode,
            )?,
            registration_object_node: decoder.field(24, DecodedPersistentId::decode)?,
            cell_definition_node: decoder.field(25, DecodedPersistentId::decode)?,
            descriptor_definition_node: decoder.field(26, DecodedPersistentId::decode)?,
            registration_fingerprint_node: decoder.field(27, DecodedPersistentId::decode)?,
            registration_definition_patch: decoder.field(28, DecodedPersistentId::decode)?,
        })
    }
}

fn encode_field(
    encoder: &mut Encoder,
    field: u32,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    value.encode(encoder)
}

fn encode_unsigned_field(
    encoder: &mut Encoder,
    field: u32,
    value: u64,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    encoder.unsigned(value)
}

fn encode_array_field<T: WireEncode>(
    encoder: &mut Encoder,
    field: u32,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

fn decode_array_field<T>(
    decoder: &mut Decoder<'_, '_>,
    field: u32,
    decode: impl Fn(&mut Decoder<'_, '_>) -> Result<T, WireError>,
) -> Result<Vec<T>, WireError> {
    decoder.field(field, |decoder| {
        decoder.decode_array(|decoder, _| decode(decoder))
    })
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_unsigned_field(encoder, 0, tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_unsigned_field(encoder, 0, tag)?;
    encode_field(encoder, 1, value)
}

fn unknown_tag(decoder: &Decoder<'_, '_>, tag: u64) -> WireError {
    WireError::new(
        scoop_wire::WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

fn require_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(WireError::new(
            scoop_wire::WireErrorKind::InvalidLength { expected, actual },
            decoder.path().clone(),
            Some(decoder.position()),
        ))
    }
}
