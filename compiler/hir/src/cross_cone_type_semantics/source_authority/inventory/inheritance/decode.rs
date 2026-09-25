use super::*;
use crate::{
    CallableDeclarationIdResolver, DecodedCanonicalInheritanceSlotSchemasV1,
    DecodedCanonicalPersistentIdsV1, DecodedCanonicalProtectedDeclarationRefsV1,
    InheritanceSlotSchemaResolutionError, ProtectedDeclarationResolutionError,
    SignatureTypeReferenceResolver,
};
use scoop_identity::{
    DecodedPersistentId, PersistentDispatchSlotId, PersistentIdResolver, PersistentPropertyId,
};
use scoop_wire::{Decoder, WireDecode, WireError};

pub trait SourceInheritanceInventoryResolver<E>:
    CallableDeclarationIdResolver<E>
    + SignatureTypeReferenceResolver<E>
    + PersistentIdResolver<PersistentPropertyId, Error = E>
    + PersistentIdResolver<PersistentExactTypeId, Error = E>
    + PersistentIdResolver<PersistentDispatchSlotId, Error = E>
{
}
impl<R, E> SourceInheritanceInventoryResolver<E> for R where
    R: CallableDeclarationIdResolver<E>
        + SignatureTypeReferenceResolver<E>
        + PersistentIdResolver<PersistentPropertyId, Error = E>
        + PersistentIdResolver<PersistentExactTypeId, Error = E>
        + PersistentIdResolver<PersistentDispatchSlotId, Error = E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedSourceInheritanceInventoryV1 {
    owner: DecodedPersistentId<PersistentExactTypeId>,
    constructors: DecodedCanonicalPersistentIdsV1<PersistentConstructorId>,
    protected_members: DecodedCanonicalProtectedDeclarationRefsV1,
    slot_schemas: DecodedCanonicalInheritanceSlotSchemasV1,
}

impl DecodedSourceInheritanceInventoryV1 {
    pub fn resolve<R: SourceInheritanceInventoryResolver<E>, E: fmt::Display>(
        self,
        resolver: &mut R,
    ) -> Result<SourceInheritanceInventoryV1, SourceInventoryError> {
        let owner = resolver.resolve(self.owner).map_err(reference)?;
        let constructors = self.constructors.resolve(resolver).map_err(reference)?;
        let members = self
            .protected_members
            .resolve(resolver)
            .map_err(|error| match error {
                ProtectedDeclarationResolutionError::Resource(error) => {
                    SourceInventoryError::Resource(error)
                }
                error => reference(error),
            })?;
        let schemas = self
            .slot_schemas
            .resolve(resolver)
            .map_err(|error| match error {
                InheritanceSlotSchemaResolutionError::Resource(error) => {
                    SourceInventoryError::Resource(error)
                }
                error => reference(error),
            })?;
        SourceInheritanceInventoryV1::try_new(owner, constructors, members, schemas)
    }
}

impl WireDecode for DecodedSourceInheritanceInventoryV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            owner: decoder.field(1, DecodedPersistentId::decode)?,
            constructors: decoder.field(2, DecodedCanonicalPersistentIdsV1::decode)?,
            protected_members: decoder
                .field(3, DecodedCanonicalProtectedDeclarationRefsV1::decode)?,
            slot_schemas: decoder.field(4, DecodedCanonicalInheritanceSlotSchemasV1::decode)?,
        })
    }
}

impl WireEncode for DecodedSourceInheritanceInventoryV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.constructors.encode(encoder)?;
        encoder.field(3)?;
        self.protected_members.encode(encoder)?;
        encoder.field(4)?;
        self.slot_schemas.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalSourceInheritanceInventoriesV1 {
    records: Vec<DecodedSourceInheritanceInventoryV1>,
}

impl DecodedCanonicalSourceInheritanceInventoriesV1 {
    pub fn resolve<R: SourceInheritanceInventoryResolver<E>, E: fmt::Display>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalSourceInheritanceInventoriesV1, SourceInventoryError> {
        let mut records = reserve(self.records.len())?;
        for record in self.records {
            records.push(record.resolve(resolver)?);
        }
        CanonicalSourceInheritanceInventoriesV1::from_ordered(records)
    }
}

impl WireDecode for DecodedCanonicalSourceInheritanceInventoriesV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedSourceInheritanceInventoryV1::decode(decoder))
            .map(|records| Self { records })
    }
}

impl WireEncode for DecodedCanonicalSourceInheritanceInventoriesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}
