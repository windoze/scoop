use super::*;
use crate::NominalSupportPropertyInterfaceV1;
use scoop_identity::PersistentPropertyId;
use scoop_wire::{Encoder, WireEncode};

mod wire;
pub use wire::*;

/// Independent runtime property contracts for protected and dispatch sources.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalInheritanceSourcePropertiesV1 {
    records: Vec<NominalSupportPropertyInterfaceV1>,
}

impl CanonicalInheritanceSourcePropertiesV1 {
    pub fn try_new(
        mut records: Vec<NominalSupportPropertyInterfaceV1>,
    ) -> Result<Self, SourceInventoryError> {
        records.sort_unstable_by_key(NominalSupportPropertyInterfaceV1::declaration);
        Self::from_ordered(records)
    }

    fn from_ordered(
        records: Vec<NominalSupportPropertyInterfaceV1>,
    ) -> Result<Self, SourceInventoryError> {
        validate_order(
            &records,
            NominalSupportPropertyInterfaceV1::declaration,
            "inheritance source properties",
        )?;

        for record in &records {
            if !matches!(
                record.payload(),
                crate::NominalSupportPropertyPayloadV1::Runtime { .. }
            ) {
                return Err(SourceInventoryError::NonRuntimeProperty(
                    record.declaration(),
                ));
            }
        }
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

impl WireEncode for CanonicalInheritanceSourcePropertiesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        super::wire::sequence(encoder, &self.records)
    }
}
