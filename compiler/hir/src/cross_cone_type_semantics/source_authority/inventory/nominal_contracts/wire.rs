use super::*;
use crate::{
    DecodedCanonicalBinderListV1, DecodedCanonicalSignatureTypesV1, DecodedNestedSourceMemberRefV1,
    DecodedNominalSourceShapeV1, DecodedSourceNominalId, NestedSourceInterfaceResolver,
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
    ) -> Result<NominalSourceContractV1, SourceInventoryError> {
        let owner = self.owner.resolve(resolver).map_err(reference)?;
        let type_parameters = self.type_parameters.resolve(resolver).map_err(reference)?;
        let supertypes = self.supertypes.resolve(resolver).map_err(reference)?;
        let mut constructors = references(self.constructors.len())?;
        for id in self.constructors {
            constructors.push(resolver.resolve(id).map_err(reference)?);
        }
        validate_order(&constructors, |id| *id, "nominal source constructors")?;
        let constructors = CanonicalPersistentIdsV1::try_new(constructors).map_err(reference)?;
        let mut members = references(self.members.len())?;
        for member in self.members {
            members.push(member.resolve(resolver).map_err(reference)?);
        }
        let members = CanonicalNestedMemberRefsV1::from_ordered(members).map_err(reference)?;
        let mut children = references(self.children.len())?;
        for child in self.children {
            children.push(child.resolve(resolver).map_err(reference)?);
        }
        let children = CanonicalNestedNominalRefsV1::from_ordered(children).map_err(reference)?;
        let source_shape = self.source_shape.resolve(resolver).map_err(reference)?;
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
fn references<T>(count: usize) -> Result<Vec<T>, SourceInventoryError> {
    // Typed references have bounded encodings, including canonical-order keys.

    reserve(count)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalNominalSourceContractsV1 {
    records: Vec<DecodedContract>,
}
impl DecodedCanonicalNominalSourceContractsV1 {
    pub fn resolve<R: NestedSourceInterfaceResolver<E>, E: fmt::Display>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalNominalSourceContractsV1, SourceInventoryError> {
        let mut records = reserve(self.records.len())?;
        for record in self.records {
            records.push(record.resolve(resolver)?);
        }
        CanonicalNominalSourceContractsV1::from_ordered(records)
    }
}
impl WireDecode for DecodedCanonicalNominalSourceContractsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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
