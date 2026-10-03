use crate::cross_cone_type_semantics::wire;
use crate::{
    CanonicalInheritanceSlotContractsV1, CanonicalInheritanceSlotSchemasV1,
    CanonicalProtectedDeclarationRefsV1, NominalInheritanceEdgesV1,
};
use scoop_identity::PersistentExactTypeId;
use scoop_wire::{Encoder, WireEncode};
use std::collections::BTreeSet;

mod decode;
mod errors;
mod table;
#[cfg(test)]
pub(in crate::cross_cone_type_semantics) mod tests;
mod validation;

pub use decode::*;
pub use errors::*;
pub use table::*;
pub use validation::*;

/// Complete inheritance edges, protected member references and dispatch slots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalInheritanceInterfaceV1 {
    edges: NominalInheritanceEdgesV1,
    slots: CanonicalInheritanceSlotContractsV1,
    protected_members: CanonicalProtectedDeclarationRefsV1,
    slot_schemas: CanonicalInheritanceSlotSchemasV1,
}
impl NominalInheritanceInterfaceV1 {
    pub fn try_new(
        edges: NominalInheritanceEdgesV1,
        slots: CanonicalInheritanceSlotContractsV1,
        protected_members: CanonicalProtectedDeclarationRefsV1,
        slot_schemas: CanonicalInheritanceSlotSchemasV1,
    ) -> Result<Self, InheritanceInterfaceBuildError> {
        let required: BTreeSet<_> = slot_schemas
            .records()
            .iter()
            .flat_map(|schema| {
                schema
                    .slots()
                    .iter()
                    .map(move |slot| (schema.role(), *slot))
            })
            .collect();
        if !required
            .iter()
            .copied()
            .eq(slots.records().iter().map(|slot| slot.key()))
        {
            return Err(InheritanceInterfaceBuildError::SlotClosure);
        }
        Ok(Self {
            edges,
            slots,
            protected_members,
            slot_schemas,
        })
    }
    pub const fn owner(&self) -> PersistentExactTypeId {
        self.edges.owner()
    }
    pub const fn edges(&self) -> &NominalInheritanceEdgesV1 {
        &self.edges
    }
    pub const fn slots(&self) -> &CanonicalInheritanceSlotContractsV1 {
        &self.slots
    }
    pub const fn protected_members(&self) -> &CanonicalProtectedDeclarationRefsV1 {
        &self.protected_members
    }
    pub const fn slot_schemas(&self) -> &CanonicalInheritanceSlotSchemasV1 {
        &self.slot_schemas
    }
}
impl WireEncode for NominalInheritanceInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(7)?;
        self.edges.encode_fields(encoder)?;
        // Fields 5 and 6 are retired; visibility and constructors use shared declarations.
        encoder.field(7)?;
        self.slots.encode(encoder)?;
        encoder.field(8)?;
        self.protected_members.encode(encoder)?;
        encoder.field(9)?;
        self.slot_schemas.encode(encoder)
    }
}
