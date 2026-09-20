//! Independent declaration contracts for dispatch roots and selected targets.

use super::*;
use crate::{
    CallableModalityV1, DeclarationAccessSourceV1, InheritanceCallableDeclarationV1,
    InheritanceCallableSignatureV1, InheritanceSourceCallableFactsV1,
};
use scoop_wire::{Encoder, WireEncode};

mod wire;
pub use wire::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InheritanceSourceCallableV1 {
    declaration: InheritanceCallableDeclarationV1,
    signature: InheritanceCallableSignatureV1,
    modality: CallableModalityV1,
    declaration_access: DeclarationAccessSourceV1,
}

impl InheritanceSourceCallableV1 {
    pub const fn new(
        declaration: InheritanceCallableDeclarationV1,
        signature: InheritanceCallableSignatureV1,
        modality: CallableModalityV1,
        declaration_access: DeclarationAccessSourceV1,
    ) -> Self {
        Self {
            declaration,
            signature,
            modality,
            declaration_access,
        }
    }
    pub const fn declaration(&self) -> InheritanceCallableDeclarationV1 {
        self.declaration
    }
    pub const fn signature(&self) -> &InheritanceCallableSignatureV1 {
        &self.signature
    }
    pub const fn modality(&self) -> CallableModalityV1 {
        self.modality
    }
    pub const fn declaration_access(&self) -> &DeclarationAccessSourceV1 {
        &self.declaration_access
    }
    pub const fn facts(&self) -> InheritanceSourceCallableFactsV1<'_> {
        InheritanceSourceCallableFactsV1 {
            signature: &self.signature,
            modality: self.modality,
            declaration_access: &self.declaration_access,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalInheritanceSourceCallablesV1 {
    records: Vec<InheritanceSourceCallableV1>,
}

impl CanonicalInheritanceSourceCallablesV1 {
    pub fn try_new(
        mut records: Vec<InheritanceSourceCallableV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, SourceInventoryError> {
        charge_sort(records.len(), meter)?;
        records.sort_unstable_by_key(InheritanceSourceCallableV1::declaration);
        Self::from_ordered(records, meter)
    }
    fn from_ordered(
        records: Vec<InheritanceSourceCallableV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, SourceInventoryError> {
        validate_order(
            &records,
            InheritanceSourceCallableV1::declaration,
            "inheritance source callables",
            meter,
        )?;
        Ok(Self { records })
    }
    pub fn records(&self) -> &[InheritanceSourceCallableV1] {
        &self.records
    }
    pub fn get(
        &self,
        declaration: InheritanceCallableDeclarationV1,
    ) -> Option<&InheritanceSourceCallableV1> {
        self.records
            .binary_search_by_key(&declaration, InheritanceSourceCallableV1::declaration)
            .ok()
            .map(|index| &self.records[index])
    }
}

impl WireEncode for CanonicalInheritanceSourceCallablesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        super::wire::sequence(encoder, &self.records)
    }
}
