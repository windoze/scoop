//! Required inheritance inputs projected independently of candidate interfaces.

use super::*;
use crate::{
    CanonicalInheritanceSlotSchemasV1, CanonicalPersistentIdsV1,
    CanonicalProtectedDeclarationRefsV1, ProtectedDeclarationRefV1,
};
use scoop_identity::{PersistentConstructorId, PersistentExactTypeId};
use scoop_wire::{Encoder, WireEncode};

mod decode;
pub use decode::*;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceInheritanceInventoryV1 {
    owner: PersistentExactTypeId,
    constructors: CanonicalPersistentIdsV1<PersistentConstructorId>,
    protected_members: CanonicalProtectedDeclarationRefsV1,
    slot_schemas: CanonicalInheritanceSlotSchemasV1,
}

impl SourceInheritanceInventoryV1 {
    pub fn try_new(
        owner: PersistentExactTypeId,
        constructors: CanonicalPersistentIdsV1<PersistentConstructorId>,
        protected_members: CanonicalProtectedDeclarationRefsV1,
        slot_schemas: CanonicalInheritanceSlotSchemasV1,
    ) -> Result<Self, SourceInventoryError> {
        let result = Self {
            owner,
            constructors,
            protected_members,
            slot_schemas,
        };
        result.validate()?;
        Ok(result)
    }

    pub const fn owner(&self) -> PersistentExactTypeId {
        self.owner
    }
    pub const fn constructors(&self) -> &CanonicalPersistentIdsV1<PersistentConstructorId> {
        &self.constructors
    }
    pub const fn protected_members(&self) -> &CanonicalProtectedDeclarationRefsV1 {
        &self.protected_members
    }
    pub const fn slot_schemas(&self) -> &CanonicalInheritanceSlotSchemasV1 {
        &self.slot_schemas
    }

    fn validate(&self) -> Result<(), SourceInventoryError> {
        for member in self.protected_members.values() {
            if let ProtectedDeclarationRefV1::Constructor(constructor) = member {
                return Err(SourceInventoryError::ConstructorInMembers {
                    owner: self.owner,
                    constructor: *constructor,
                });
            }
        }

        Ok(())
    }
}

impl WireEncode for SourceInheritanceInventoryV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.constructors.encode(encoder)?;
        encoder.field(3)?;
        self.protected_members.encode(encoder)?;
        encoder.field(4)?;
        self.slot_schemas.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSourceInheritanceInventoriesV1 {
    records: Vec<SourceInheritanceInventoryV1>,
    owners: CanonicalPersistentIdsV1<PersistentExactTypeId>,
}

impl CanonicalSourceInheritanceInventoriesV1 {
    pub fn try_new(
        mut records: Vec<SourceInheritanceInventoryV1>,
    ) -> Result<Self, SourceInventoryError> {
        records.sort_unstable_by_key(SourceInheritanceInventoryV1::owner);
        Self::from_ordered(records)
    }

    fn from_ordered(
        records: Vec<SourceInheritanceInventoryV1>,
    ) -> Result<Self, SourceInventoryError> {
        validate_order(
            &records,
            SourceInheritanceInventoryV1::owner,
            "inheritance owners",
        )?;
        let mut owners = reserve(records.len())?;
        for record in records.iter() {
            record.validate()?;
            owners.push(record.owner());
        }
        let owners = CanonicalPersistentIdsV1::try_new(owners).map_err(reference)?;
        Ok(Self { records, owners })
    }

    pub fn records(&self) -> &[SourceInheritanceInventoryV1] {
        &self.records
    }
    pub const fn owners(&self) -> &CanonicalPersistentIdsV1<PersistentExactTypeId> {
        &self.owners
    }
    pub fn get(&self, owner: PersistentExactTypeId) -> Option<&SourceInheritanceInventoryV1> {
        self.records
            .binary_search_by_key(&owner, SourceInheritanceInventoryV1::owner)
            .ok()
            .map(|index| &self.records[index])
    }
}

impl WireEncode for CanonicalSourceInheritanceInventoriesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}
