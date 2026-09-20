use super::*;
use crate::{ExportDefinitionSourceV1, ProtectedParameterCallingKindV1, SourceParameterShapeV1};
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{Encoder, WireEncode};
use std::collections::BTreeSet;

mod wire;
pub use wire::*;

/// Independent declaration-side argument facts, without default-body authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InheritanceSourceParameterV1 {
    shape: SourceParameterShapeV1,
    calling: ProtectedParameterCallingKindV1,
    origin: ExportDefinitionSourceV1,
}
impl InheritanceSourceParameterV1 {
    pub const fn new(
        shape: SourceParameterShapeV1,
        calling: ProtectedParameterCallingKindV1,
        origin: ExportDefinitionSourceV1,
    ) -> Self {
        Self {
            shape,
            calling,
            origin,
        }
    }
    pub const fn shape(&self) -> &SourceParameterShapeV1 {
        &self.shape
    }
    pub const fn calling_kind(&self) -> ProtectedParameterCallingKindV1 {
        self.calling
    }
    pub const fn definition_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.origin
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InheritanceSourceParameterProtocolV1 {
    owner: CallableTemplateOrigin,
    parameters: Vec<InheritanceSourceParameterV1>,
}
impl InheritanceSourceParameterProtocolV1 {
    pub fn try_new(
        owner: CallableTemplateOrigin,
        parameters: Vec<InheritanceSourceParameterV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, SourceInventoryError> {
        if !matches!(
            owner,
            CallableTemplateOrigin::Function(_)
                | CallableTemplateOrigin::GenericFunction(_)
                | CallableTemplateOrigin::Constructor(_)
        ) {
            return Err(reference(
                "inheritance parameter owner must be a source function or constructor",
            ));
        }
        let path = WirePath::root();
        charge_sort(parameters.len(), meter)?;
        meter.charge_collection_slots(parameters.len() as u64, &path)?;
        let mut names = BTreeSet::new();
        let mut vararg = false;
        for parameter in &parameters {
            let name = parameter.shape.name();
            meter.check_semantic_leaf(name.as_str().len() as u64, &path)?;
            meter.charge_work(
                (name.as_str().len() as u64)
                    .saturating_mul(u64::from(parameters.len().max(1).ilog2()) + 1),
                &path,
            )?;
            if !names.insert(name) {
                return Err(reference("duplicate inheritance source parameter name"));
            }
            if matches!(
                parameter.calling,
                ProtectedParameterCallingKindV1::VarargEmpty
                    | ProtectedParameterCallingKindV1::VarargDefault
            ) && std::mem::replace(&mut vararg, true)
            {
                return Err(reference("multiple inheritance source vararg parameters"));
            }
        }
        Ok(Self { owner, parameters })
    }
    pub const fn owner(&self) -> CallableTemplateOrigin {
        self.owner
    }
    pub fn parameters(&self) -> &[InheritanceSourceParameterV1] {
        &self.parameters
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalInheritanceSourceParameterProtocolsV1 {
    records: Vec<InheritanceSourceParameterProtocolV1>,
}
impl CanonicalInheritanceSourceParameterProtocolsV1 {
    pub fn try_new(
        mut records: Vec<InheritanceSourceParameterProtocolV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, SourceInventoryError> {
        charge_sort(records.len(), meter)?;
        records.sort_unstable_by_key(InheritanceSourceParameterProtocolV1::owner);
        Self::from_ordered(records, meter)
    }
    fn from_ordered(
        records: Vec<InheritanceSourceParameterProtocolV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, SourceInventoryError> {
        validate_order(
            &records,
            InheritanceSourceParameterProtocolV1::owner,
            "inheritance source parameter protocols",
            meter,
        )?;
        Ok(Self { records })
    }
    pub fn records(&self) -> &[InheritanceSourceParameterProtocolV1] {
        &self.records
    }
    pub fn get(
        &self,
        owner: CallableTemplateOrigin,
    ) -> Option<&InheritanceSourceParameterProtocolV1> {
        self.records
            .binary_search_by_key(&owner, InheritanceSourceParameterProtocolV1::owner)
            .ok()
            .map(|index| &self.records[index])
    }
}
impl WireEncode for CanonicalInheritanceSourceParameterProtocolsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        super::wire::sequence(encoder, &self.records)
    }
}
