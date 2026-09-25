use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath};

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
    ) -> Result<CanonicalProtectedDefaultTemplatesV1, ProtectedDefaultTemplateTableResolutionError<E>>
    where
        R: DefaultStatementReferenceResolver<E> + ProtectedDefaultReferenceResolver<E>,
    {
        use ProtectedDefaultTemplateTableResolutionError as Error;
        let path = WirePath::root();
        let count = self.records.len();
        let mut records = Vec::new();
        scoop_wire::allocation::try_reserve(&mut records, count, &path).map_err(Error::Resource)?;
        // Exact key projection allocation and its fixed-size identity ordering.

        for (index, record) in self.records.into_iter().enumerate() {
            records.push(
                record
                    .resolve(resolver)
                    .map_err(|error| Error::Template { index, error })?,
            );
        }
        CanonicalProtectedDefaultTemplatesV1::from_ordered(records).map_err(Error::Build)
    }
}
impl WireDecode for DecodedCanonicalProtectedDefaultTemplatesV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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
