use scoop_identity::{
    ConeIdentity, DecodedPersistentId, PersistentExactTypeId, PersistentIdResolver,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::*;
use crate::TypeSectionDependencyFactV1;

impl WireEncode for TypeSectionDependencyFactV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.provider.encode(encoder)?;
        encoder.field(2)?;
        self.exact.encode(encoder)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalTypeSectionDependencyFactsV1 {
    records: Vec<TypeSectionDependencyFactV1>,
}

impl CanonicalTypeSectionDependencyFactsV1 {
    pub fn try_new(
        mut records: Vec<TypeSectionDependencyFactV1>,
    ) -> Result<Self, SourceInventoryError> {
        records.sort_unstable_by_key(|record| record.exact);
        Self::from_ordered(records)
    }

    fn from_ordered(
        records: Vec<TypeSectionDependencyFactV1>,
    ) -> Result<Self, SourceInventoryError> {
        validate_order(&records, |record| record.exact, "dependency facts")?;
        Ok(Self { records })
    }

    pub fn records(&self) -> &[TypeSectionDependencyFactV1] {
        &self.records
    }
}

impl WireEncode for CanonicalTypeSectionDependencyFactsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedTypeSectionDependencyFactV1 {
    provider: DecodedPersistentId<ConeIdentity>,
    exact: DecodedPersistentId<PersistentExactTypeId>,
}

impl WireDecode for DecodedTypeSectionDependencyFactV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            provider: decoder.field(1, DecodedPersistentId::decode)?,
            exact: decoder.field(2, DecodedPersistentId::decode)?,
        })
    }
}

impl WireEncode for DecodedTypeSectionDependencyFactV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.provider.encode(encoder)?;
        encoder.field(2)?;
        self.exact.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalTypeSectionDependencyFactsV1 {
    records: Vec<DecodedTypeSectionDependencyFactV1>,
}

impl DecodedCanonicalTypeSectionDependencyFactsV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalTypeSectionDependencyFactsV1, SourceInventoryError>
    where
        R: PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentIdResolver<PersistentExactTypeId, Error = E>,
        E: fmt::Display,
    {
        let mut records = reserve(self.records.len())?;
        for record in self.records {
            records.push(TypeSectionDependencyFactV1 {
                provider: resolver.resolve(record.provider).map_err(reference)?,
                exact: resolver.resolve(record.exact).map_err(reference)?,
            });
        }
        CanonicalTypeSectionDependencyFactsV1::from_ordered(records)
    }
}

impl WireDecode for DecodedCanonicalTypeSectionDependencyFactsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|d, _| DecodedTypeSectionDependencyFactV1::decode(d))
            .map(|records| Self { records })
    }
}

impl WireEncode for DecodedCanonicalTypeSectionDependencyFactsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}
