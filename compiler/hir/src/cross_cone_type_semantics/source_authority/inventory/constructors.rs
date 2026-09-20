use super::*;
use crate::NominalSupportConstructorInterfaceV1;
use scoop_identity::PersistentConstructorId;
use scoop_wire::{Encoder, WireEncode};

mod wire;
pub use wire::*;

/// Independent constructor source contracts, including protected declarations.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalInheritanceSourceConstructorsV1 {
    records: Vec<NominalSupportConstructorInterfaceV1>,
}

impl CanonicalInheritanceSourceConstructorsV1 {
    pub fn try_new(
        mut records: Vec<NominalSupportConstructorInterfaceV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, SourceInventoryError> {
        charge_sort(records.len(), meter)?;
        records.sort_unstable_by_key(NominalSupportConstructorInterfaceV1::declaration);
        Self::from_ordered(records, meter)
    }

    fn from_ordered(
        records: Vec<NominalSupportConstructorInterfaceV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, SourceInventoryError> {
        validate_order(
            &records,
            NominalSupportConstructorInterfaceV1::declaration,
            "inheritance source constructors",
            meter,
        )?;
        Ok(Self { records })
    }

    pub fn records(&self) -> &[NominalSupportConstructorInterfaceV1] {
        &self.records
    }

    pub fn get(
        &self,
        declaration: PersistentConstructorId,
    ) -> Option<&NominalSupportConstructorInterfaceV1> {
        self.records
            .binary_search_by_key(
                &declaration,
                NominalSupportConstructorInterfaceV1::declaration,
            )
            .ok()
            .map(|index| &self.records[index])
    }
}

impl WireEncode for CanonicalInheritanceSourceConstructorsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        super::wire::sequence(encoder, &self.records)
    }
}
