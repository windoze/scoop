use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::*;
use crate::{DecodedSourceNominalId, SourceNominalId, SourceNominalIdResolver};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalSourceNominalIdsV1 {
    values: Vec<SourceNominalId>,
}

impl CanonicalSourceNominalIdsV1 {
    pub fn try_new(mut values: Vec<SourceNominalId>) -> Result<Self, SourceInventoryError> {
        values.sort_unstable();
        Self::from_ordered(values)
    }

    fn from_ordered(values: Vec<SourceNominalId>) -> Result<Self, SourceInventoryError> {
        validate_order(&values, |value| *value, "source roots")?;
        Ok(Self { values })
    }

    pub fn values(&self) -> &[SourceNominalId] {
        &self.values
    }
}

impl WireEncode for CanonicalSourceNominalIdsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.values)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalSourceNominalIdsV1 {
    values: Vec<DecodedSourceNominalId>,
}

impl DecodedCanonicalSourceNominalIdsV1 {
    pub fn resolve<R: SourceNominalIdResolver<E>, E: fmt::Display>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalSourceNominalIdsV1, SourceInventoryError> {
        let mut values = reserve(self.values.len())?;
        for value in self.values {
            values.push(value.resolve(resolver).map_err(reference)?);
        }
        CanonicalSourceNominalIdsV1::from_ordered(values)
    }
}

impl WireDecode for DecodedCanonicalSourceNominalIdsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|d, _| DecodedSourceNominalId::decode(d))
            .map(|values| Self { values })
    }
}

impl WireEncode for DecodedCanonicalSourceNominalIdsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.values)
    }
}
