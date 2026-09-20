use super::*;
use crate::{
    DecodedDefaultSourceTemplateV1, DefaultSourceReferenceResolver,
    DefaultStatementReferenceResolver,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalDefaultSourceTemplatesV1 {
    records: Vec<DecodedDefaultSourceTemplateV1>,
}
impl DecodedCanonicalDefaultSourceTemplatesV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalDefaultSourceTemplatesV1, DefaultSourceTemplateTableResolutionError<E>>
    where
        R: DefaultStatementReferenceResolver<E> + DefaultSourceReferenceResolver<E>,
    {
        use DefaultSourceTemplateTableResolutionError as Error;
        u32::try_from(self.records.len())
            .map_err(|_| Error::Table(DefaultSourceTemplateTableBuildError::TooMany))?;
        let path = WirePath::root();
        meter
            .check_semantic_depth(1, &path)
            .map_err(Error::Resource)?;
        meter.charge_nodes(1, &path).map_err(Error::Resource)?;
        meter
            .charge_work((self.records.len() as u64).saturating_add(1), &path)
            .map_err(Error::Resource)?;
        meter
            .charge_collection_slots(self.records.len() as u64, &path)
            .map_err(Error::Resource)?;
        meter
            .charge_owned_bytes(
                (self.records.len() as u64)
                    .saturating_mul(std::mem::size_of::<DefaultSourceTemplateV1>() as u64),
                &path,
            )
            .map_err(Error::Resource)?;
        let mut records = Vec::new();
        meter
            .try_reserve_collection_slots(&mut records, self.records.len(), &path)
            .map_err(Error::Resource)?;
        for (index, record) in self.records.into_iter().enumerate() {
            records.push(
                record
                    .resolve(resolver, meter)
                    .map_err(|error| Error::Template { index, error })?,
            );
        }
        CanonicalDefaultSourceTemplatesV1::from_ordered(records, meter).map_err(Error::Table)
    }
}
impl WireDecode for DecodedCanonicalDefaultSourceTemplatesV1 {
    fn decode(d: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        d.decode_array(|d, _| DecodedDefaultSourceTemplateV1::decode(d))
            .map(|records| Self { records })
    }
}
impl WireEncode for DecodedCanonicalDefaultSourceTemplatesV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(e)?;
        }
        Ok(())
    }
}
