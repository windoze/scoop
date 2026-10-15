use scoop_wire::{
    Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath, encode_canonical_temporary,
};

use super::wire::EncodeResult;
use super::{DecodedExternalShapeLinkImportV1, ExternalShapeLinkImportV1, ShapeLinkError};

mod replay;

#[derive(Debug)]
pub struct CanonicalExternalShapeLinkImportsV1 {
    records: Vec<ExternalShapeLinkImportV1>,
}

impl CanonicalExternalShapeLinkImportsV1 {
    /// Only the complete closure may choose the actual-use subset. Every
    /// input record has already replayed its provider and support relation.
    pub(crate) fn from_checked(
        records: Vec<ExternalShapeLinkImportV1>,
    ) -> Result<Self, ShapeLinkError> {
        let path = WirePath::root();

        let mut keyed = Vec::new();
        scoop_wire::allocation::try_reserve(&mut keyed, records.len(), &path)?;
        for record in records {
            let key = key(&record.provider(), &record.subject())?;
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
        scoop_wire::allocation::try_reserve(&mut records, keyed.len(), &path)?;
        records.extend(keyed.into_iter().map(|(_, record)| record));
        Ok(Self { records })
    }
    /// Code inputs retain the selected semantic contract; artifact imports only store references.
    pub fn code_projection(&self) -> impl WireEncode + '_ {
        CodeProjection(self)
    }

    pub fn records(&self) -> &[ExternalShapeLinkImportV1] {
        &self.records
    }
}

#[derive(Debug)]
pub struct DecodedCanonicalExternalShapeLinkImportsV1 {
    records: Vec<DecodedExternalShapeLinkImportV1>,
}

impl WireEncode for CanonicalExternalShapeLinkImportsV1 {
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
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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
fn key(provider: &impl WireEncode, subject: &impl WireEncode) -> Result<Vec<u8>, WireError> {
    struct Key<'a, P, S>(&'a P, &'a S);
    impl<P: WireEncode, S: WireEncode> WireEncode for Key<'_, P, S> {
        fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
            encoder.array(2)?;
            self.0.encode(encoder)?;
            self.1.encode(encoder)
        }
    }
    encode_canonical_temporary(&Key(provider, subject), &WirePath::root())
}

struct CodeProjection<'a>(&'a CanonicalExternalShapeLinkImportsV1);
impl WireEncode for CodeProjection<'_> {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        use super::wire::field;
        encoder.array(self.0.records.len() as u64)?;
        for record in &self.0.records {
            encoder.map(5)?;
            field(encoder, 1, &record.provider())?;
            field(encoder, 2, &record.subject())?;
            field(encoder, 3, &record.expected_symbol())?;
            field(encoder, 4, &record.required_definition())?;
            field(encoder, 5, record.contract())?;
        }
        Ok(())
    }
}
