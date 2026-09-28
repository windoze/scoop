//! Semantic fields shared with the complete registration wire records.
use super::*;

#[derive(Clone, Copy, Debug)]
pub struct StrongStaticStorageSemanticProjectionV1<'a>(
    &'a crate::StrongStaticStorageSemanticPlanV1,
);
impl crate::StrongStaticStorageSemanticPlanV1 {
    pub const fn semantic_projection(&self) -> StrongStaticStorageSemanticProjectionV1<'_> {
        StrongStaticStorageSemanticProjectionV1(self)
    }

    pub const fn canonical_projection(&self) -> StaticStorageCanonicalProjectionV1<'_> {
        StaticStorageCanonicalProjectionV1(self)
    }
}

/// Physical content omits the consumer's routing choice for a value layout.
#[derive(Clone, Copy, Debug)]
pub struct StaticStorageCanonicalProjectionV1<'a>(&'a crate::StrongStaticStorageSemanticPlanV1);

impl WireEncode for StaticStorageCanonicalProjectionV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(10)?;
        encode_static_semantic_fields(encoder, self.0)
    }
}
impl WireEncode for StrongStaticStorageSemanticProjectionV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(11)?;
        encode_static_semantic_fields(encoder, self.0)?;
        encode_field(encoder, 32, &self.0.layout_provider())
    }
}
pub(super) fn encode_static_semantic_fields(
    encoder: &mut Encoder,
    semantic: &crate::StrongStaticStorageSemanticPlanV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encode_field(encoder, 1, &semantic.storage())?;
    encode_field(encoder, 2, &semantic.symbol())?;
    encode_field(encoder, 3, &semantic.layout())?;
    encode_field(encoder, 4, &semantic.scan())?;
    encoder.field(5)?;
    encode_ref_scan(encoder, semantic.scan_program())?;
    encode_unsigned_field(encoder, 6, u64::from(semantic.scan_kind().tag()))?;
    encode_unsigned_field(encoder, 7, semantic.byte_size())?;
    encode_unsigned_field(encoder, 8, semantic.allocation_extent())?;
    encode_unsigned_field(encoder, 9, semantic.required_alignment())?;
    encoder.field(10)?;
    encode_static_initial_state(encoder, semantic.initial_state())
}

#[derive(Clone, Copy, Debug)]
pub struct StrongInitializationUnitSemanticProjectionV1<'a, D>(
    &'a crate::StrongInitializationUnitSemanticPlan<D>,
);
impl<D> crate::StrongInitializationUnitSemanticPlan<D> {
    pub const fn semantic_projection(&self) -> StrongInitializationUnitSemanticProjectionV1<'_, D> {
        StrongInitializationUnitSemanticProjectionV1(self)
    }
}
impl<D: WireEncode> WireEncode for StrongInitializationUnitSemanticProjectionV1<'_, D> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
        encode_unit_semantic_fields(encoder, self.0)
    }
}
pub(super) fn encode_unit_semantic_fields<D: WireEncode>(
    encoder: &mut Encoder,
    semantic: &crate::StrongInitializationUnitSemanticPlan<D>,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encode_field(encoder, 1, &semantic.unit())?;
    encoder.field(2)?;
    encoder.text(semantic.diagnostic_path())?;
    encoder.field(3)?;
    super::initialization::encode_initialization_semantic_schedule(encoder, semantic.schedule())?;
    encode_field(encoder, 4, &semantic.storage())?;
    encode_field(encoder, 5, &semantic.failure_root())?;
    encode_field(encoder, 6, &semantic.initializer())?;
    encode_field(encoder, 7, &semantic.ensure())?;
    encode_array_field(encoder, 8, semantic.dependencies())
}
