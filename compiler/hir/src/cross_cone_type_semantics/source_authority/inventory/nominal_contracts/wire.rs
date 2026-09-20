use super::*;
use crate::{
    DecodedCanonicalBinderListV1, DecodedCanonicalSignatureTypesV1, DecodedNestedSourceMemberRefV1,
    DecodedNominalSourceShapeV1, DecodedSourceNominalId, MeteredInterfaceResolutionError,
    NestedSourceInterfaceResolver,
};
use scoop_identity::DecodedPersistentId;
use scoop_wire::{Decoder, WireDecode};

mod records;

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedContract {
    owner: DecodedSourceNominalId,
    modality: NominalInheritanceModalityV1,
    type_parameters: DecodedCanonicalBinderListV1,
    supertypes: DecodedCanonicalSignatureTypesV1,
    constructors: Vec<DecodedPersistentId<PersistentConstructorId>>,
    members: Vec<DecodedNestedSourceMemberRefV1>,
    children: Vec<DecodedSourceNominalId>,
    source_shape: DecodedNominalSourceShapeV1,
}
impl DecodedContract {
    fn resolve<R: NestedSourceInterfaceResolver<E>, E: fmt::Display>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<NominalSourceContractV1, SourceInventoryError> {
        meter.charge_work(128, &WirePath::root())?;
        let owner = self.owner.resolve(resolver).map_err(reference)?;
        let type_parameters = resolved(self.type_parameters.resolve_metered(resolver, meter))?;
        let supertypes = resolved(self.supertypes.resolve_metered(resolver, meter))?;
        let mut constructors = references(self.constructors.len(), meter)?;
        for id in self.constructors {
            constructors.push(resolver.resolve(id).map_err(reference)?);
        }
        validate_order(
            &constructors,
            |id| *id,
            "nominal source constructors",
            meter,
        )?;
        let constructors = CanonicalPersistentIdsV1::try_new(constructors).map_err(reference)?;
        let mut members = references(self.members.len(), meter)?;
        for member in self.members {
            members.push(member.resolve(resolver).map_err(reference)?);
        }
        let members = CanonicalNestedMemberRefsV1::from_ordered(members).map_err(reference)?;
        let mut children = references(self.children.len(), meter)?;
        for child in self.children {
            children.push(child.resolve(resolver).map_err(reference)?);
        }
        let children = CanonicalNestedNominalRefsV1::from_ordered(children).map_err(reference)?;
        let source_shape = resolved(self.source_shape.resolve_metered(resolver, meter))?;
        NominalSourceContractV1::try_new(
            owner,
            self.modality,
            type_parameters,
            supertypes,
            constructors,
            members,
            children,
            source_shape,
        )
    }
}
fn resolved<T, E: fmt::Display>(
    result: Result<T, MeteredInterfaceResolutionError<E>>,
) -> Result<T, SourceInventoryError> {
    result.map_err(|error| match error {
        MeteredInterfaceResolutionError::Resource(error) => SourceInventoryError::Resource(error),
        MeteredInterfaceResolutionError::Value(error) => reference(error),
    })
}
fn references<T>(count: usize, meter: &mut BudgetMeter) -> Result<Vec<T>, SourceInventoryError> {
    // Typed references have bounded encodings, including canonical-order keys.
    meter.charge_work((count as u64).saturating_mul(128), &WirePath::root())?;
    meter.charge_owned_bytes((count as u64).saturating_mul(128), &WirePath::root())?;
    reserve(count, meter)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalNominalSourceContractsV1 {
    records: Vec<DecodedContract>,
}
impl DecodedCanonicalNominalSourceContractsV1 {
    pub fn resolve<R: NestedSourceInterfaceResolver<E>, E: fmt::Display>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalNominalSourceContractsV1, SourceInventoryError> {
        let mut records = reserve(self.records.len(), meter)?;
        for record in self.records {
            records.push(record.resolve(resolver, meter)?);
        }
        CanonicalNominalSourceContractsV1::from_ordered(records, meter)
    }
}
impl WireDecode for DecodedCanonicalNominalSourceContractsV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|d, _| DecodedContract::decode(d))
            .map(|records| Self { records })
    }
}
impl WireEncode for DecodedCanonicalNominalSourceContractsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        super::super::wire::sequence(encoder, &self.records)
    }
}

#[cfg(test)]
mod tests;
