//! Untrusted static storage registration carriers.

use super::*;

#[derive(Debug)]
pub(in crate::production::registration_production) struct DecodedStaticImmortalRelocationPlanV1 {
    pub(in crate::production::registration_production) pointer_offset: u64,
    pub(in crate::production::registration_production) target:
        DecodedPersistentId<PersistentImmortalObjectId>,
}

impl WireEncode for DecodedStaticImmortalRelocationPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encode_unsigned_field(encoder, 1, self.pointer_offset)?;
        encode_field(encoder, 2, &self.target)
    }
}

impl WireDecode for DecodedStaticImmortalRelocationPlanV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            pointer_offset: decoder.field(1, Decoder::unsigned)?,
            target: decoder.field(2, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Debug)]
pub(in crate::production::registration_production) enum DecodedStrongStaticStorageInitialStatePlanV1
{
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
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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
    pub(in crate::production::registration_production) semantic:
        DecodedStrongStaticStorageSemanticProjectionV1,
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
        encoder.map(32)?;
        self.semantic.encode_fields(encoder)?;
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
        encode_field(encoder, 31, &self.scan_fingerprint_patch)?;
        encode_field(encoder, 32, &self.semantic.layout_provider)
    }
}

impl WireDecode for DecodedStrongStaticStorageRegistrationPlanV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(32)?;
        let storage = decoder.field(1, DecodedPersistentId::decode)?;
        let storage_symbol = decoder.field(2, DecodedPersistentSymbolRequest::decode)?;
        let layout = decoder.field(3, DecodedPersistentId::decode)?;
        let scan = decoder.field(4, DecodedPersistentId::decode)?;
        let scan_program = decoder.field(5, DecodedRefScan::decode)?;
        let scan_kind = decoder.field(6, Decoder::u32)?;
        let byte_size = decoder.field(7, Decoder::unsigned)?;
        let allocation_extent = decoder.field(8, Decoder::unsigned)?;
        let required_alignment = decoder.field(9, Decoder::unsigned)?;
        let initial_state =
            decoder.field(10, DecodedStrongStaticStorageInitialStatePlanV1::decode)?;
        let registration_symbol = decoder.field(11, DecodedPersistentSymbolRequest::decode)?;
        let registration_definition_plan = decoder.field(12, DecodedPersistentId::decode)?;
        let registration_primary_atom = decoder.field(13, DecodedPersistentId::decode)?;
        let storage_definition_plan = decoder.field(14, DecodedPersistentId::decode)?;
        let storage_primary_atom = decoder.field(15, DecodedPersistentId::decode)?;
        let initial_artifacts =
            decoder.field(16, DecodedStrongStaticStorageInitialArtifactPlanV1::decode)?;
        let immortal_registration_symbols = decoder.field(17, |decoder| {
            decoder.decode_array(|decoder, _| DecodedPersistentSymbolRequest::decode(decoder))
        })?;
        let layout_symbol = decoder.field(18, DecodedPersistentSymbolRequest::decode)?;
        let layout_definition_plan = decoder.field(19, DecodedPersistentId::decode)?;
        let layout_primary_atom = decoder.field(20, DecodedPersistentId::decode)?;
        let scan_symbol = decoder.field(21, DecodedPersistentSymbolRequest::decode)?;
        let scan_definition_plan = decoder.field(22, DecodedPersistentId::decode)?;
        let scan_primary_atom = decoder.field(23, DecodedPersistentId::decode)?;
        let registration_object_node = decoder.field(24, DecodedPersistentId::decode)?;
        let storage_definition_node = decoder.field(25, DecodedPersistentId::decode)?;
        let layout_fingerprint_node = decoder.field(26, DecodedPersistentId::decode)?;
        let scan_fingerprint_node = decoder.field(27, DecodedPersistentId::decode)?;
        let registration_fingerprint_node = decoder.field(28, DecodedPersistentId::decode)?;
        let registration_definition_patch = decoder.field(29, DecodedPersistentId::decode)?;
        let layout_fingerprint_patch = decoder.field(30, DecodedPersistentId::decode)?;
        let scan_fingerprint_patch = decoder.field(31, DecodedPersistentId::decode)?;
        let layout_provider = decoder.field(32, DecodedPersistentId::decode)?;
        Ok(Self {
            semantic: DecodedStrongStaticStorageSemanticProjectionV1 {
                storage,
                storage_symbol,
                layout,
                scan,
                scan_program,
                scan_kind,
                byte_size,
                allocation_extent,
                required_alignment,
                initial_state,
                layout_provider,
            },
            registration_symbol,
            registration_definition_plan,
            registration_primary_atom,
            storage_definition_plan,
            storage_primary_atom,
            initial_artifacts,
            immortal_registration_symbols,
            layout_symbol,
            layout_definition_plan,
            layout_primary_atom,
            scan_symbol,
            scan_definition_plan,
            scan_primary_atom,
            registration_object_node,
            storage_definition_node,
            layout_fingerprint_node,
            scan_fingerprint_node,
            registration_fingerprint_node,
            registration_definition_patch,
            layout_fingerprint_patch,
            scan_fingerprint_patch,
        })
    }
}

mod projection;
pub use projection::*;
