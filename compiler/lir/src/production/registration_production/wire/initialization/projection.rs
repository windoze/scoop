use super::*;

#[derive(Debug)]
pub struct DecodedStrongInitializationUnitSemanticProjectionV1 {
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
}
impl DecodedStrongInitializationUnitSemanticProjectionV1 {
    pub(super) fn decode_fields(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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
        })
    }
    pub(super) fn encode_fields(
        &self,
        encoder: &mut Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_field(encoder, 1, &self.unit)?;
        encoder.field(2)?;
        encoder.text(&self.diagnostic_path)?;
        encode_field(encoder, 3, &self.semantic_schedule)?;
        encode_field(encoder, 4, &self.storage_id)?;
        encode_field(encoder, 5, &self.failure_root_id)?;
        encode_field(encoder, 6, &self.initializer_id)?;
        encode_field(encoder, 7, &self.ensure_id)?;
        encode_array_field(encoder, 8, &self.dependencies)
    }
}
impl WireDecode for DecodedStrongInitializationUnitSemanticProjectionV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(8)?;
        Self::decode_fields(decoder)
    }
}
impl WireEncode for DecodedStrongInitializationUnitSemanticProjectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
        self.encode_fields(encoder)
    }
}

impl DecodedStrongInitializationUnitSemanticProjectionV1 {
    /// V2 dependency proofs remain in the checked plan; the projection keeps
    /// the established unit-id wire and cannot construct those proofs.
    pub fn validate_against<D: WireEncode>(
        self,
        expected: &crate::StrongInitializationUnitSemanticPlan<D>,
    ) -> Result<(), StrongSemanticProjectionError> {
        compare_semantics(&self, &expected.semantic_projection())
    }
}
