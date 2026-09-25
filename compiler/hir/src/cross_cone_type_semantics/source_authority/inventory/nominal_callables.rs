use super::*;
use crate::NominalSupportCallableInterfaceV1;
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{Encoder, WireEncode};

mod wire;
pub use wire::*;

/// Complete member and variant sources, without lookup or execution authority.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalNominalSourceCallablesV1 {
    records: Vec<NominalSupportCallableInterfaceV1>,
}
impl CanonicalNominalSourceCallablesV1 {
    pub fn try_new(
        mut records: Vec<NominalSupportCallableInterfaceV1>,
    ) -> Result<Self, SourceInventoryError> {
        records.sort_unstable_by_key(NominalSupportCallableInterfaceV1::declaration);
        Self::from_ordered(records)
    }
    fn from_ordered(
        records: Vec<NominalSupportCallableInterfaceV1>,
    ) -> Result<Self, SourceInventoryError> {
        validate_order(
            &records,
            NominalSupportCallableInterfaceV1::declaration,
            "nominal source callables",
        )?;
        Ok(Self { records })
    }
    pub fn records(&self) -> &[NominalSupportCallableInterfaceV1] {
        &self.records
    }
    pub fn get(
        &self,
        declaration: CallableTemplateOrigin,
    ) -> Option<&NominalSupportCallableInterfaceV1> {
        self.records
            .binary_search_by_key(&declaration, NominalSupportCallableInterfaceV1::declaration)
            .ok()
            .map(|index| &self.records[index])
    }
}
impl WireEncode for CanonicalNominalSourceCallablesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        super::wire::sequence(encoder, &self.records)
    }
}
