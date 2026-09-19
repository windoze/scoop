use super::*;

#[derive(Debug)]
pub struct DecodedStrongStaticStorageSemanticProjectionV1 {
    pub(in crate::production::registration_production) storage:
        DecodedPersistentId<PersistentStaticStorageId>,
    storage_symbol: DecodedPersistentSymbolRequest,
    pub(in crate::production::registration_production) layout:
        DecodedPersistentId<PersistentLayoutId>,
    pub(in crate::production::registration_production) scan: DecodedPersistentId<PersistentScanId>,
    pub(in crate::production::registration_production) scan_program: DecodedRefScan,
    pub(in crate::production::registration_production) scan_kind: u32,
    pub(in crate::production::registration_production) byte_size: u64,
    pub(in crate::production::registration_production) allocation_extent: u64,
    pub(in crate::production::registration_production) required_alignment: u64,
    pub(in crate::production::registration_production) initial_state:
        DecodedStrongStaticStorageInitialStatePlanV1,
}
impl DecodedStrongStaticStorageSemanticProjectionV1 {
    pub(super) fn decode_fields(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
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
        })
    }
    pub(super) fn encode_fields(
        &self,
        encoder: &mut Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_field(encoder, 1, &self.storage)?;
        encode_field(encoder, 2, &self.storage_symbol)?;
        encode_field(encoder, 3, &self.layout)?;
        encode_field(encoder, 4, &self.scan)?;
        encode_field(encoder, 5, &self.scan_program)?;
        encode_unsigned_field(encoder, 6, u64::from(self.scan_kind))?;
        encode_unsigned_field(encoder, 7, self.byte_size)?;
        encode_unsigned_field(encoder, 8, self.allocation_extent)?;
        encode_unsigned_field(encoder, 9, self.required_alignment)?;
        encode_field(encoder, 10, &self.initial_state)
    }
}
impl WireDecode for DecodedStrongStaticStorageSemanticProjectionV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(10)?;
        Self::decode_fields(decoder)
    }
}
impl WireEncode for DecodedStrongStaticStorageSemanticProjectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(10)?;
        self.encode_fields(encoder)
    }
}

impl DecodedStrongStaticStorageSemanticProjectionV1 {
    /// Checks this projection against a complete provider semantic plan. It
    /// does not authorize the backing storage as an externally selected use.
    pub fn validate_against(
        self,
        expected: &crate::StrongStaticStorageSemanticPlanV1,
        meter: &mut scoop_wire::BudgetMeter,
    ) -> Result<(), StrongSemanticProjectionError> {
        let path = scoop_wire::WirePath::root();
        meter.charge_nodes(1, &path)?;
        meter.charge_work(10, &path)?;
        match (&self.scan_program, expected.scan_program()) {
            (DecodedRefScan::None, crate::RefScan::None) => {}
            (DecodedRefScan::References(actual), crate::RefScan::References(expected)) => {
                meter.charge_work(
                    (actual.len() as u64).saturating_add(expected.len() as u64),
                    &path,
                )?;
            }
            _ => return Err(StrongSemanticProjectionError::Mismatch),
        }
        if let DecodedStrongStaticStorageInitialStatePlanV1::EncodedStaticValue {
            initial_template,
            immortal_relocations,
        } = &self.initial_state
        {
            meter.charge_work(
                (initial_template.len() as u64)
                    .saturating_add((immortal_relocations.len() as u64).saturating_mul(3)),
                &path,
            )?;
        }
        meter.charge_work(
            (expected.initial_state().initial_template().len() as u64).saturating_add(
                (expected.initial_state().immortal_relocations().len() as u64).saturating_mul(3),
            ),
            &path,
        )?;
        compare_semantics(&self, &expected.semantic_projection(), meter)
    }
}
