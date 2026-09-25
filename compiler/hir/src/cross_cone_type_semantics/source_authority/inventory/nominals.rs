use scoop_identity::{
    ConeIdentity, PersistentIdResolver, PersistentKeyResolver, PersistentSourceContextId,
    SourceContextKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::*;
use crate::{
    DeclarationAccessSourceV1, DecodedDeclarationAccessSourceV1, DecodedSourceNominalId,
    SourceNominalId, SourceNominalIdResolver,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeSourceNominalV1 {
    owner: SourceNominalId,
    access: DeclarationAccessSourceV1,
}

impl TypeSourceNominalV1 {
    pub const fn new(owner: SourceNominalId, access: DeclarationAccessSourceV1) -> Self {
        Self { owner, access }
    }

    pub const fn owner(&self) -> SourceNominalId {
        self.owner
    }

    pub const fn access(&self) -> &DeclarationAccessSourceV1 {
        &self.access
    }
}

impl WireEncode for TypeSourceNominalV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.access.encode(encoder)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalTypeSourceNominalsV1 {
    records: Vec<TypeSourceNominalV1>,
}

impl CanonicalTypeSourceNominalsV1 {
    pub fn try_new(mut records: Vec<TypeSourceNominalV1>) -> Result<Self, SourceInventoryError> {
        records.sort_unstable_by_key(TypeSourceNominalV1::owner);
        Self::from_ordered(records)
    }

    fn from_ordered(records: Vec<TypeSourceNominalV1>) -> Result<Self, SourceInventoryError> {
        validate_order(&records, TypeSourceNominalV1::owner, "nominal snapshots")?;
        Ok(Self { records })
    }

    pub fn records(&self) -> &[TypeSourceNominalV1] {
        &self.records
    }

    pub fn get(&self, owner: SourceNominalId) -> Option<&TypeSourceNominalV1> {
        self.records
            .binary_search_by_key(&owner, TypeSourceNominalV1::owner)
            .ok()
            .map(|index| &self.records[index])
    }
}

impl WireEncode for CanonicalTypeSourceNominalsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedTypeSourceNominalV1 {
    owner: DecodedSourceNominalId,
    access: DecodedDeclarationAccessSourceV1,
}

impl WireDecode for DecodedTypeSourceNominalV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            owner: decoder.field(1, DecodedSourceNominalId::decode)?,
            access: decoder.field(2, DecodedDeclarationAccessSourceV1::decode)?,
        })
    }
}

impl WireEncode for DecodedTypeSourceNominalV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.access.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalTypeSourceNominalsV1 {
    records: Vec<DecodedTypeSourceNominalV1>,
}

impl DecodedCanonicalTypeSourceNominalsV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalTypeSourceNominalsV1, SourceInventoryError>
    where
        R: SourceNominalIdResolver<E>
            + PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>,
        E: fmt::Display,
    {
        let mut records = reserve(self.records.len())?;
        for record in self.records.into_iter() {
            let owner = record.owner.resolve(resolver).map_err(reference)?;
            let access = record.access.resolve(resolver).map_err(reference)?;
            records.push(TypeSourceNominalV1::new(owner, access));
        }
        CanonicalTypeSourceNominalsV1::from_ordered(records)
    }
}

impl WireDecode for DecodedCanonicalTypeSourceNominalsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|d, _| DecodedTypeSourceNominalV1::decode(d))
            .map(|records| Self { records })
    }
}

impl WireEncode for DecodedCanonicalTypeSourceNominalsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}
