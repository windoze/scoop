use super::*;
use crate::ProtectedCallableInterfaceV1;
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{Encoder, WireEncode};

mod wire;
pub use wire::*;

/// Independent protected callable contracts, without public lookup authority.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalInheritanceSourceProtectedCallablesV1 {
    records: Vec<ProtectedCallableInterfaceV1>,
}

impl CanonicalInheritanceSourceProtectedCallablesV1 {
    pub fn try_new(
        mut records: Vec<ProtectedCallableInterfaceV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, SourceInventoryError> {
        charge_sort(records.len(), meter)?;
        records.sort_unstable_by_key(ProtectedCallableInterfaceV1::declaration);
        Self::from_ordered(records, meter)
    }

    fn from_ordered(
        records: Vec<ProtectedCallableInterfaceV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, SourceInventoryError> {
        validate_order(
            &records,
            ProtectedCallableInterfaceV1::declaration,
            "inheritance source protected callables",
            meter,
        )?;
        Ok(Self { records })
    }

    pub fn records(&self) -> &[ProtectedCallableInterfaceV1] {
        &self.records
    }

    pub fn get(
        &self,
        declaration: CallableTemplateOrigin,
    ) -> Option<&ProtectedCallableInterfaceV1> {
        self.records
            .binary_search_by_key(&declaration, ProtectedCallableInterfaceV1::declaration)
            .ok()
            .map(|index| &self.records[index])
    }
}

impl WireEncode for CanonicalInheritanceSourceProtectedCallablesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        super::wire::sequence(encoder, &self.records)
    }
}
