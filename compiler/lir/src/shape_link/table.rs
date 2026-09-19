use scoop_wire::{
    BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath,
    encode_canonical_temporary_with_meter,
};

use super::wire::EncodeResult;
use super::{DecodedExternalShapeLinkImportV1, ExternalShapeLinkImportV1, ShapeLinkError};

#[derive(Debug)]
pub struct CanonicalExternalShapeLinkImportsV1<'a> {
    records: Vec<ExternalShapeLinkImportV1<'a>>,
}

impl<'a> CanonicalExternalShapeLinkImportsV1<'a> {
    /// Only the complete closure may choose the actual-use subset. Every
    /// input record has already replayed its provider and support relation.
    pub(crate) fn from_checked(
        records: Vec<ExternalShapeLinkImportV1<'a>>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, ShapeLinkError> {
        let path = WirePath::root();
        let count = records.len() as u64;
        meter.check_table_entries(count, &path)?;
        meter.charge_nodes(count, &path)?;
        meter.charge_work(
            count.saturating_mul(u64::from(count.max(1).ilog2()) + 1),
            &path,
        )?;
        let mut keyed = Vec::new();
        meter.try_reserve_collection_slots(&mut keyed, records.len(), &path)?;
        for record in records {
            let key = key(&record.provider(), &record.subject(), meter)?;
            keyed.push((key, record));
        }
        keyed.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        for pair in keyed.windows(2) {
            if pair[0].0 == pair[1].0 {
                return Err(ShapeLinkError::Duplicate {
                    provider: pair[1].1.provider(),
                    subject: pair[1].1.subject(),
                });
            }
        }
        let mut records = Vec::new();
        meter.try_reserve_collection_slots(&mut records, keyed.len(), &path)?;
        records.extend(keyed.into_iter().map(|(_, record)| record));
        Ok(Self { records })
    }
    pub fn records(&self) -> &[ExternalShapeLinkImportV1<'a>] {
        &self.records
    }
}

#[derive(Debug)]
pub struct DecodedCanonicalExternalShapeLinkImportsV1 {
    records: Vec<DecodedExternalShapeLinkImportV1>,
}

impl DecodedCanonicalExternalShapeLinkImportsV1 {
    pub fn validate_against<'a>(
        self,
        expected: &CanonicalExternalShapeLinkImportsV1<'a>,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalExternalShapeLinkImportsV1<'a>, ShapeLinkError> {
        let path = WirePath::root();
        meter.charge_nodes(self.records.len() as u64, &path)?;
        meter.charge_work(self.records.len() as u64, &path)?;
        if self.records.len() != expected.records.len() {
            return Err(ShapeLinkError::Count);
        }
        let mut previous: Option<Vec<u8>> = None;
        for (actual, expected) in self.records.into_iter().zip(&expected.records) {
            let key = key(&actual.provider, &actual.subject, meter)?;
            if previous.as_ref().is_some_and(|previous| previous >= &key) {
                return Err(ShapeLinkError::Order);
            }
            previous = Some(key);
            actual.validate_against(expected, meter)?;
        }
        let mut records = Vec::new();
        meter.try_reserve_collection_slots(&mut records, expected.records.len(), &path)?;
        records.extend_from_slice(&expected.records);
        CanonicalExternalShapeLinkImportsV1::from_checked(records, meter)
    }
}

impl WireEncode for CanonicalExternalShapeLinkImportsV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        sequence(encoder, &self.records)
    }
}
impl WireEncode for DecodedCanonicalExternalShapeLinkImportsV1 {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        sequence(encoder, &self.records)
    }
}
impl WireDecode for DecodedCanonicalExternalShapeLinkImportsV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedExternalShapeLinkImportV1::decode(decoder))
            .map(|records| Self { records })
    }
}

fn sequence(encoder: &mut Encoder, records: &[impl WireEncode]) -> EncodeResult {
    encoder.array(records.len() as u64)?;
    for record in records {
        record.encode(encoder)?;
    }
    Ok(())
}
fn key(
    provider: &impl WireEncode,
    subject: &impl WireEncode,
    meter: &mut BudgetMeter,
) -> Result<Vec<u8>, WireError> {
    struct Key<'a, P, S>(&'a P, &'a S);
    impl<P: WireEncode, S: WireEncode> WireEncode for Key<'_, P, S> {
        fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
            encoder.array(2)?;
            self.0.encode(encoder)?;
            self.1.encode(encoder)
        }
    }
    encode_canonical_temporary_with_meter(&Key(provider, subject), meter, &WirePath::root())
}
