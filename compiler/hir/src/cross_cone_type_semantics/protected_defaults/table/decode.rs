use scoop_wire::{BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath};

use super::*;
use crate::{
    DecodedProtectedDefaultTemplateV1, DefaultStatementReferenceResolver,
    ProtectedDefaultReferenceResolver,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalProtectedDefaultTemplatesV1 {
    records: Vec<DecodedProtectedDefaultTemplateV1>,
}
impl DecodedCanonicalProtectedDefaultTemplatesV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalProtectedDefaultTemplatesV1, ProtectedDefaultTemplateTableResolutionError<E>>
    where
        R: DefaultStatementReferenceResolver<E> + ProtectedDefaultReferenceResolver<E>,
    {
        use ProtectedDefaultTemplateTableResolutionError as Error;
        let path = WirePath::root();
        let count = self.records.len();
        let mut records = Vec::new();
        meter
            .try_reserve_collection_slots(&mut records, count, &path)
            .map_err(Error::Resource)?;
        // Exact key projection allocation and its fixed-size identity ordering.
        meter
            .charge_collection_slots(count as u64, &path)
            .map_err(Error::Resource)?;
        let comparisons =
            (count as u64).saturating_mul(u64::from(usize::BITS - count.leading_zeros()) + 1);
        meter
            .charge_work(comparisons.saturating_mul(64), &path)
            .map_err(Error::Resource)?;
        for (index, record) in self.records.into_iter().enumerate() {
            records.push(
                record
                    .resolve(resolver, meter)
                    .map_err(|error| Error::Template { index, error })?,
            );
        }
        CanonicalProtectedDefaultTemplatesV1::from_ordered(records).map_err(Error::Build)
    }
}
impl WireDecode for DecodedCanonicalProtectedDefaultTemplatesV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedProtectedDefaultTemplateV1::decode(decoder))
            .map(|records| Self { records })
    }
}
impl WireEncode for DecodedCanonicalProtectedDefaultTemplatesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}
