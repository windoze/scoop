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
    ) -> Result<CanonicalDefaultSourceTemplatesV1, DefaultSourceTemplateTableResolutionError<E>>
    where
        R: DefaultStatementReferenceResolver<E> + DefaultSourceReferenceResolver<E>,
    {
        use DefaultSourceTemplateTableResolutionError as Error;
        u32::try_from(self.records.len())
            .map_err(|_| Error::Table(DefaultSourceTemplateTableBuildError::TooMany))?;
        let path = WirePath::root();

        let mut records = Vec::new();
        scoop_wire::allocation::try_reserve(&mut records, self.records.len(), &path)
            .map_err(Error::Resource)?;
        for (index, record) in self.records.into_iter().enumerate() {
            records.push(
                record
                    .resolve(resolver)
                    .map_err(|error| Error::Template { index, error })?,
            );
        }
        CanonicalDefaultSourceTemplatesV1::from_ordered(records).map_err(Error::Table)
    }
}
impl WireDecode for DecodedCanonicalDefaultSourceTemplatesV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
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
