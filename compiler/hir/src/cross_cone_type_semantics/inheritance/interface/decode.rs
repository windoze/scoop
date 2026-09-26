use super::*;
use crate::{
    DecodedCanonicalInheritanceSlotContractsV1, DecodedCanonicalInheritanceSlotSchemasV1,
    DecodedCanonicalProtectedDeclarationRefsV1, DecodedNominalInheritanceEdgesV1,
    InheritanceSlotResolver, NestedSourceInterfaceResolver,
};
use scoop_wire::{Decoder, WireDecode, WireError};

pub trait NominalInheritanceInterfaceResolver<E>:
    InheritanceSlotResolver<E> + NestedSourceInterfaceResolver<E>
{
}
impl<R, E> NominalInheritanceInterfaceResolver<E> for R where
    R: InheritanceSlotResolver<E> + NestedSourceInterfaceResolver<E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedNominalInheritanceInterfaceV1 {
    edges: DecodedNominalInheritanceEdgesV1,
    constructors: DecodedCanonicalInheritanceConstructorsV1,
    slots: DecodedCanonicalInheritanceSlotContractsV1,
    protected_members: DecodedCanonicalProtectedDeclarationRefsV1,
    slot_schemas: DecodedCanonicalInheritanceSlotSchemasV1,
}
impl DecodedNominalInheritanceInterfaceV1 {
    pub fn resolve<R: NominalInheritanceInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<NominalInheritanceInterfaceV1, InheritanceInterfaceResolutionError<E>> {
        use InheritanceInterfaceResolutionError as Error;

        let edges = self.edges.resolve(resolver).map_err(Error::Edges)?;
        let constructors = self.constructors.resolve(resolver)?;
        let slots = self.slots.resolve(resolver).map_err(Error::Slots)?;
        let members = self
            .protected_members
            .resolve(resolver)
            .map_err(Error::Members)?;
        let schemas = self
            .slot_schemas
            .resolve(resolver)
            .map_err(Error::Schemas)?;

        NominalInheritanceInterfaceV1::try_new(edges, constructors, slots, members, schemas)
            .map_err(Error::Build)
    }
}
impl WireDecode for DecodedNominalInheritanceInterfaceV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(8)?;
        Ok(Self {
            edges: DecodedNominalInheritanceEdgesV1::decode_fields(decoder)?,
            constructors: decoder.field(6, DecodedCanonicalInheritanceConstructorsV1::decode)?,
            slots: decoder.field(7, DecodedCanonicalInheritanceSlotContractsV1::decode)?,
            protected_members: decoder
                .field(8, DecodedCanonicalProtectedDeclarationRefsV1::decode)?,
            slot_schemas: decoder.field(9, DecodedCanonicalInheritanceSlotSchemasV1::decode)?,
        })
    }
}
impl WireEncode for DecodedNominalInheritanceInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
        self.edges.encode_fields(encoder)?;
        // Field 5 is retired.
        encoder.field(6)?;
        self.constructors.encode(encoder)?;
        encoder.field(7)?;
        self.slots.encode(encoder)?;
        encoder.field(8)?;
        self.protected_members.encode(encoder)?;
        encoder.field(9)?;
        self.slot_schemas.encode(encoder)
    }
}
