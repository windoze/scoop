use super::*;
use crate::NominalSupportPropertyInterfaceV1;
use scoop_identity::PersistentPropertyId;
use scoop_wire::{Encoder, WireEncode};

mod wire;
pub use wire::*;

/// Complete nominal member sources, retaining restricted runtime and const
/// declarations without granting public lookup or materialization authority.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalNominalSourcePropertiesV1 {
    records: Vec<NominalSupportPropertyInterfaceV1>,
}
impl CanonicalNominalSourcePropertiesV1 {
    pub fn try_new(
        mut records: Vec<NominalSupportPropertyInterfaceV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, SourceInventoryError> {
        charge_sort(records.len(), meter)?;
        records.sort_unstable_by_key(NominalSupportPropertyInterfaceV1::declaration);
        Self::from_ordered(records, meter)
    }
    fn from_ordered(
        records: Vec<NominalSupportPropertyInterfaceV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, SourceInventoryError> {
        validate_order(
            &records,
            NominalSupportPropertyInterfaceV1::declaration,
            "nominal source properties",
            meter,
        )?;
        Ok(Self { records })
    }
    pub fn records(&self) -> &[NominalSupportPropertyInterfaceV1] {
        &self.records
    }
    pub fn get(
        &self,
        declaration: PersistentPropertyId,
    ) -> Option<&NominalSupportPropertyInterfaceV1> {
        self.records
            .binary_search_by_key(&declaration, NominalSupportPropertyInterfaceV1::declaration)
            .ok()
            .map(|index| &self.records[index])
    }
}
impl WireEncode for CanonicalNominalSourcePropertiesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        super::wire::sequence(encoder, &self.records)
    }
}
