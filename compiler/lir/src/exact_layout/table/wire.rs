use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode};

use super::*;

#[derive(Debug)]
pub struct DecodedCanonicalExactLayoutExportsV1 {
    records: Vec<DecodedExactLayoutExportV1>,
}

impl DecodedCanonicalExactLayoutExportsV1 {
    pub fn validate_against(
        self,
        expected: &CanonicalExactLayoutExportsV1,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalExactLayoutExportsV1, ExactLayoutTableError> {
        meter.charge_work(self.records.len() as u64, &WirePath::root())?;
        if self.records.len() != expected.records().len() {
            return Err(ExactLayoutTableError::Count);
        }
        for (index, (record, expected)) in
            self.records.into_iter().zip(expected.records()).enumerate()
        {
            record
                .validate_against(expected, meter)
                .map_err(|source| ExactLayoutTableError::Record { index, source })?;
        }
        Ok(expected.clone())
    }
}

fn sequence<T: WireEncode>(
    encoder: &mut Encoder,
    records: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(records.len() as u64)?;
    for record in records {
        record.encode(encoder)?;
    }
    Ok(())
}

impl WireEncode for CanonicalExactLayoutExportsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        sequence(encoder, self.records())
    }
}
impl WireEncode for DecodedCanonicalExactLayoutExportsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        sequence(encoder, &self.records)
    }
}
impl WireDecode for DecodedCanonicalExactLayoutExportsV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedExactLayoutExportV1::decode(decoder))
            .map(|records| Self { records })
    }
}
