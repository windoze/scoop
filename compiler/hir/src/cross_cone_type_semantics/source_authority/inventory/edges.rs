use scoop_identity::{PersistentExactTypeId, PersistentIdResolver};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::*;
use crate::{
    DecodedNominalInheritanceEdgesV1, InheritanceEdgeResolutionError, NominalInheritanceEdgesV1,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalNominalInheritanceEdgesV1 {
    records: Vec<NominalInheritanceEdgesV1>,
}

impl CanonicalNominalInheritanceEdgesV1 {
    pub fn try_new(
        mut records: Vec<NominalInheritanceEdgesV1>,
    ) -> Result<Self, SourceInventoryError> {
        records.sort_unstable_by_key(NominalInheritanceEdgesV1::owner);
        Self::from_ordered(records)
    }

    fn from_ordered(records: Vec<NominalInheritanceEdgesV1>) -> Result<Self, SourceInventoryError> {
        validate_order(
            &records,
            NominalInheritanceEdgesV1::owner,
            "inheritance edges",
        )?;

        Ok(Self { records })
    }

    pub fn records(&self) -> &[NominalInheritanceEdgesV1] {
        &self.records
    }
}

impl WireEncode for CanonicalNominalInheritanceEdgesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalNominalInheritanceEdgesV1 {
    records: Vec<DecodedNominalInheritanceEdgesV1>,
}

impl DecodedCanonicalNominalInheritanceEdgesV1 {
    pub fn resolve<R>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalNominalInheritanceEdgesV1, SourceInventoryError>
    where
        R: PersistentIdResolver<PersistentExactTypeId>,
        R::Error: fmt::Display,
    {
        let mut records = reserve(self.records.len())?;
        for record in self.records {
            records.push(record.resolve(resolver).map_err(|error| match error {
                InheritanceEdgeResolutionError::Resource(error) => {
                    SourceInventoryError::Resource(error)
                }
                error => reference(error),
            })?);
        }
        CanonicalNominalInheritanceEdgesV1::from_ordered(records)
    }
}

impl WireDecode for DecodedCanonicalNominalInheritanceEdgesV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|d, _| DecodedNominalInheritanceEdgesV1::decode(d))
            .map(|records| Self { records })
    }
}

impl WireEncode for DecodedCanonicalNominalInheritanceEdgesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}
