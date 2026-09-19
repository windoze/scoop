use super::*;
use crate::{
    DecodedCanonicalInheritanceSlotContractsV1, DecodedCanonicalInheritanceSlotSchemasV1,
    DecodedCanonicalProtectedDeclarationRefsV1, DecodedNominalAccessDomainsV1,
    DecodedNominalInheritanceEdgesV1, InheritanceSlotResolver, NestedSourceInterfaceResolver,
};
use scoop_wire::{BudgetMeter, Decoder, WireDecode, WireError, WirePath};

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
    domains: DecodedNominalAccessDomainsV1,
    constructors: DecodedCanonicalInheritanceConstructorsV1,
    slots: DecodedCanonicalInheritanceSlotContractsV1,
    protected_members: DecodedCanonicalProtectedDeclarationRefsV1,
    slot_schemas: DecodedCanonicalInheritanceSlotSchemasV1,
}
impl DecodedNominalInheritanceInterfaceV1 {
    pub fn resolve<R: NominalInheritanceInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<NominalInheritanceInterfaceV1, InheritanceInterfaceResolutionError<E>> {
        use InheritanceInterfaceResolutionError as Error;
        let path = WirePath::root();
        meter.charge_nodes(1, &path).map_err(Error::Resource)?;
        let edges = self
            .edges
            .resolve_metered(resolver, meter)
            .map_err(Error::Edges)?;
        let domains = self
            .domains
            .resolve_metered(resolver, meter)
            .map_err(Error::Domains)?;
        let constructors = self.constructors.resolve(resolver, meter)?;
        let slots = self.slots.resolve(resolver, meter).map_err(Error::Slots)?;
        let members = self
            .protected_members
            .resolve(resolver, meter)
            .map_err(Error::Members)?;
        let schemas = self
            .slot_schemas
            .resolve(resolver, meter)
            .map_err(Error::Schemas)?;
        let count = schemas.records().iter().fold(0_u64, |count, schema| {
            count.saturating_add(schema.slots().len() as u64)
        });
        meter
            .charge_collection_slots(count, &path)
            .map_err(Error::Resource)?;
        let comparisons = (u64::BITS - count.leading_zeros()) as u64;
        meter
            .charge_work(
                count
                    .saturating_mul(comparisons.saturating_add(1))
                    .saturating_add(members.values().len() as u64)
                    .saturating_add(slots.records().len() as u64),
                &path,
            )
            .map_err(Error::Resource)?;
        NominalInheritanceInterfaceV1::try_new(
            edges,
            domains,
            constructors,
            slots,
            members,
            schemas,
        )
        .map_err(Error::Build)
    }
}
impl WireDecode for DecodedNominalInheritanceInterfaceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(9)?;
        Ok(Self {
            edges: DecodedNominalInheritanceEdgesV1::decode_fields(decoder)?,
            domains: decoder.field(5, DecodedNominalAccessDomainsV1::decode)?,
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
        encoder.map(9)?;
        self.edges.encode_fields(encoder)?;
        encoder.field(5)?;
        self.domains.encode(encoder)?;
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
