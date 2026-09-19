use crate::cross_cone_type_semantics::wire;
use crate::{
    CanonicalInheritanceSlotContractsV1, CanonicalInheritanceSlotSchemasV1,
    CanonicalProtectedDeclarationRefsV1, NominalAccessDomainsV1, NominalInheritanceEdgesV1,
    ProtectedDeclarationRefV1,
};
use scoop_identity::PersistentExactTypeId;
use scoop_wire::{Encoder, WireEncode};
use std::collections::BTreeSet;

mod constructors;
mod decode;
mod errors;
mod table;
#[cfg(test)]
mod tests;
mod validation;

pub use constructors::*;
pub use decode::*;
pub use errors::*;
pub use table::*;
pub use validation::*;

/// Complete representation-independent inheritance data. Only the enclosing
/// source/graph/default closure can turn this transport record into authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalInheritanceInterfaceV1 {
    edges: NominalInheritanceEdgesV1,
    domains: NominalAccessDomainsV1,
    constructors: CanonicalInheritanceConstructorsV1,
    slots: CanonicalInheritanceSlotContractsV1,
    protected_members: CanonicalProtectedDeclarationRefsV1,
    slot_schemas: CanonicalInheritanceSlotSchemasV1,
}
impl NominalInheritanceInterfaceV1 {
    pub fn try_new(
        edges: NominalInheritanceEdgesV1,
        domains: NominalAccessDomainsV1,
        constructors: CanonicalInheritanceConstructorsV1,
        slots: CanonicalInheritanceSlotContractsV1,
        protected_members: CanonicalProtectedDeclarationRefsV1,
        slot_schemas: CanonicalInheritanceSlotSchemasV1,
    ) -> Result<Self, InheritanceInterfaceBuildError> {
        if !domains.slot().domain().is_empty() {
            return Err(InheritanceInterfaceBuildError::NominalSlotDomain);
        }
        if protected_members
            .values()
            .iter()
            .any(|value| matches!(value, ProtectedDeclarationRefV1::Constructor(_)))
        {
            return Err(InheritanceInterfaceBuildError::ConstructorInMembers);
        }
        let required: BTreeSet<_> = slot_schemas
            .records()
            .iter()
            .flat_map(|schema| schema.slots().iter().copied())
            .collect();
        if !required
            .iter()
            .copied()
            .eq(slots.records().iter().map(|slot| slot.slot()))
        {
            return Err(InheritanceInterfaceBuildError::SlotClosure);
        }
        Ok(Self {
            edges,
            domains,
            constructors,
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
    pub const fn domains(&self) -> &NominalAccessDomainsV1 {
        &self.domains
    }
    pub const fn constructors(&self) -> &CanonicalInheritanceConstructorsV1 {
        &self.constructors
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
        encoder.map(9)?;
        self.edges.encode_fields(encoder)?;
        encoder.field(5)?;
        self.domains.encode(encoder)?;
        encoder.field(6)?;
        self.constructors.encode(encoder)?;
        encoder.field(7)?;
        self.slots.encode(encoder)?;
        encoder.field(8)?;
        self.protected_members.encode(encoder)?;
        encoder.field(9)?;
        self.slot_schemas.encode(encoder)
    }
}
