//! Untrusted immortal registration carriers.

use super::*;

#[derive(Debug)]
pub(in crate::production::registration_production) enum DecodedImmortalObjectTypeRegistrationRefV1 {
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
    pub(in crate::production::registration_production) object:
        DecodedPersistentId<PersistentImmortalObjectId>,
    object_symbol: DecodedPersistentSymbolRequest,
    pub(in crate::production::registration_production) object_size: u64,
    pub(in crate::production::registration_production) required_alignment: u64,
    pub(in crate::production::registration_production) type_registration:
        DecodedImmortalObjectTypeRegistrationRefV1,
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
