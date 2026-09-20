use super::*;
use crate::{
    CanonicalBinderListV1, CanonicalNestedMemberRefsV1, CanonicalNestedNominalRefsV1,
    CanonicalPersistentIdsV1, CanonicalSignatureTypesV1, NominalInheritanceModalityV1,
    NominalSourceShapeV1, PublicNominalKindV1, SourceNominalId,
};
use scoop_identity::PersistentConstructorId;
use scoop_wire::{Encoder, WireEncode};

mod wire;
pub use wire::*;

/// Independent source structure, without lookup or materialization authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalSourceContractV1 {
    owner: SourceNominalId,
    modality: NominalInheritanceModalityV1,
    type_parameters: CanonicalBinderListV1,
    supertypes: CanonicalSignatureTypesV1,
    constructors: CanonicalPersistentIdsV1<PersistentConstructorId>,
    members: CanonicalNestedMemberRefsV1,
    children: CanonicalNestedNominalRefsV1,
    source_shape: NominalSourceShapeV1,
}
impl NominalSourceContractV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        owner: SourceNominalId,
        modality: NominalInheritanceModalityV1,
        type_parameters: CanonicalBinderListV1,
        supertypes: CanonicalSignatureTypesV1,
        constructors: CanonicalPersistentIdsV1<PersistentConstructorId>,
        members: CanonicalNestedMemberRefsV1,
        children: CanonicalNestedNominalRefsV1,
        source_shape: NominalSourceShapeV1,
    ) -> Result<Self, SourceInventoryError> {
        let kind = source_shape.kind();
        let valid = match kind {
            PublicNominalKindV1::Class => modality != NominalInheritanceModalityV1::Interface,
            PublicNominalKindV1::Interface => modality == NominalInheritanceModalityV1::Interface,
            PublicNominalKindV1::Struct
            | PublicNominalKindV1::Enum
            | PublicNominalKindV1::Object => modality == NominalInheritanceModalityV1::Final,
        };
        if !valid {
            return Err(reference(
                "nominal source modality disagrees with source kind",
            ));
        }
        if !constructors.is_empty()
            && !matches!(
                kind,
                PublicNominalKindV1::Class | PublicNominalKindV1::Struct
            )
        {
            return Err(reference("nominal source kind cannot own constructors"));
        }
        if matches!(owner, SourceNominalId::Concrete(_)) != type_parameters.is_empty()
            || (kind == PublicNominalKindV1::Object && !type_parameters.is_empty())
        {
            return Err(reference("nominal source owner disagrees with own binders"));
        }
        Ok(Self {
            owner,
            modality,
            type_parameters,
            supertypes,
            constructors,
            members,
            children,
            source_shape,
        })
    }
    pub const fn owner(&self) -> SourceNominalId {
        self.owner
    }
    pub const fn kind(&self) -> PublicNominalKindV1 {
        self.source_shape.kind()
    }
    pub const fn modality(&self) -> NominalInheritanceModalityV1 {
        self.modality
    }
    pub const fn type_parameters(&self) -> &CanonicalBinderListV1 {
        &self.type_parameters
    }
    pub const fn supertypes(&self) -> &CanonicalSignatureTypesV1 {
        &self.supertypes
    }
    pub const fn constructors(&self) -> &CanonicalPersistentIdsV1<PersistentConstructorId> {
        &self.constructors
    }
    pub const fn members(&self) -> &CanonicalNestedMemberRefsV1 {
        &self.members
    }
    pub const fn children(&self) -> &CanonicalNestedNominalRefsV1 {
        &self.children
    }
    pub const fn source_shape(&self) -> &NominalSourceShapeV1 {
        &self.source_shape
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalNominalSourceContractsV1 {
    records: Vec<NominalSourceContractV1>,
}
impl CanonicalNominalSourceContractsV1 {
    pub fn try_new(
        mut records: Vec<NominalSourceContractV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, SourceInventoryError> {
        charge_sort(records.len(), meter)?;
        records.sort_unstable_by_key(NominalSourceContractV1::owner);
        Self::from_ordered(records, meter)
    }
    fn from_ordered(
        records: Vec<NominalSourceContractV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, SourceInventoryError> {
        validate_order(
            &records,
            NominalSourceContractV1::owner,
            "nominal source contracts",
            meter,
        )?;
        Ok(Self { records })
    }
    pub fn records(&self) -> &[NominalSourceContractV1] {
        &self.records
    }
    pub fn get(&self, owner: SourceNominalId) -> Option<&NominalSourceContractV1> {
        self.records
            .binary_search_by_key(&owner, NominalSourceContractV1::owner)
            .ok()
            .map(|index| &self.records[index])
    }
}
impl WireEncode for CanonicalNominalSourceContractsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        super::wire::sequence(encoder, &self.records)
    }
}
