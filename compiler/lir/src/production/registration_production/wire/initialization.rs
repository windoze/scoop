//! Untrusted initialization registration carriers.

use super::*;

#[derive(Debug)]
pub(in crate::production::registration_production) enum DecodedStrongInitializationSchedulePlanV1 {
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
    pub(in crate::production::registration_production) unit:
        DecodedPersistentId<PersistentInitializationUnitId>,
    pub(in crate::production::registration_production) diagnostic_path: String,
    pub(in crate::production::registration_production) semantic_schedule:
        DecodedStrongInitializationSchedulePlanV1,
    pub(in crate::production::registration_production) storage_id:
        DecodedPersistentId<PersistentStaticStorageId>,
    pub(in crate::production::registration_production) failure_root_id:
        DecodedPersistentId<PersistentStaticStorageId>,
    pub(in crate::production::registration_production) initializer_id:
        DecodedPersistentId<PersistentCallableBodyId>,
    pub(in crate::production::registration_production) ensure_id:
        DecodedPersistentId<PersistentCallableBodyId>,
    pub(in crate::production::registration_production) dependencies:
        Vec<DecodedPersistentId<PersistentInitializationUnitId>>,
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
