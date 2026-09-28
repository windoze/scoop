//! Shared Compile/Link subjects for general cross-Cone Strong definitions.
//!
//! Identity resolution and physical definition binding are constituents. A
//! selected layout closure must additionally prove export and use authority,
//! including object/initialization ownership for storage and unit subjects.

use scoop_identity::{
    CallableDefinitionOwner, ConeIdentity, ObjectDefinitionPlanKey, PersistentCallableBodyId,
    PersistentDispatchTableId, PersistentExactTypeId, PersistentInitializationUnitId,
    PersistentLayoutId, PersistentScanId, PersistentStaticStorageId, PersistentSymbolKey,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_wire::{HashError, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder};

mod definition;
mod wire;
pub use definition::*;
pub use wire::*;

/// These tags belong to the layout Link capability, not persistent identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExternalStrongShapeSubjectV1 {
    Callable(CallableDefinitionOwner),
    Layout(PersistentLayoutId),
    Scan(PersistentScanId),
    TypeDescriptor(PersistentExactTypeId),
    DispatchTable(PersistentDispatchTableId),
    TypeRegistration(PersistentExactTypeId),
    StaticStorage(PersistentStaticStorageId),
    StaticStorageRegistration(PersistentStaticStorageId),
    InitializationCell(PersistentInitializationUnitId),
    InitializationDescriptor(PersistentInitializationUnitId),
}

impl ExternalStrongShapeSubjectV1 {
    pub const fn tag(self) -> u32 {
        match self {
            Self::Callable(_) => 1,
            Self::Layout(_) => 2,
            Self::Scan(_) => 3,
            Self::TypeDescriptor(_) => 4,
            Self::DispatchTable(_) => 5,
            Self::TypeRegistration(_) => 6,
            Self::StaticStorage(_) => 7,
            Self::StaticStorageRegistration(_) => 8,
            Self::InitializationCell(_) => 9,
            Self::InitializationDescriptor(_) => 10,
        }
    }

    /// Both identities are derived together so a subject cannot be relabeled
    /// as another definition role while retaining a same-typed payload id.
    pub fn expected_definition(
        self,
        provider: ConeIdentity,
    ) -> Result<(ObjectDefinitionPlanKey, PersistentSymbolKey), StrongShapeDefinitionError> {
        let (entity, role, symbol) = self.definition_parts()?;
        Ok((
            match self {
                Self::Callable(CallableDefinitionOwner::Odr(member)) => {
                    ObjectDefinitionPlanKey::odr(member.member())
                }
                _ => ObjectDefinitionPlanKey::strong(provider, entity, role)
                    .map_err(StrongShapeDefinitionError::DefinitionIdentity)?,
            },
            symbol,
        ))
    }

    fn definition_parts(
        self,
    ) -> Result<
        (
            StrongDefinitionEntity,
            StrongDefinitionRole,
            PersistentSymbolKey,
        ),
        HashError,
    > {
        use PersistentSymbolKey as Symbol;
        use StrongDefinitionEntity as Entity;
        use StrongDefinitionRole as Role;
        Ok(match self {
            Self::Callable(owner) => {
                let body = PersistentCallableBodyId::from_key(&owner.body_key())?;
                (
                    Entity::callable_body(body),
                    Role::CallableBody,
                    Symbol::CallableBody(body),
                )
            }
            Self::Layout(id) => (Entity::layout(id), Role::Layout, Symbol::Layout(id)),
            Self::Scan(id) => (Entity::scan(id), Role::ScanProgram, Symbol::ScanProgram(id)),
            Self::TypeDescriptor(id) => (
                Entity::exact_type(id),
                Role::TypeDescriptor,
                Symbol::TypeDescriptor(id),
            ),
            Self::DispatchTable(id) => (
                Entity::dispatch_table(id),
                Role::DispatchTable,
                Symbol::DispatchTable(id),
            ),
            Self::TypeRegistration(id) => (
                Entity::exact_type(id),
                Role::TypeRegistration,
                Symbol::TypeRegistration(id),
            ),
            Self::StaticStorage(id) => (
                Entity::static_storage(id),
                Role::StaticStorage,
                Symbol::StaticStorage(id),
            ),
            Self::StaticStorageRegistration(id) => (
                Entity::static_storage(id),
                Role::RootRegistration,
                Symbol::RootRegistration(id),
            ),
            Self::InitializationCell(id) => (
                Entity::initialization_unit(id),
                Role::InitializationCell,
                Symbol::InitializationCell(id),
            ),
            Self::InitializationDescriptor(id) => (
                Entity::initialization_unit(id),
                Role::InitializationDescriptor,
                Symbol::InitializationDescriptor(id),
            ),
        })
    }
}

impl RuntimeEncode for ExternalStrongShapeSubjectV1 {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encoder.u32(self.tag())?;
        match self {
            Self::Callable(owner) => owner.runtime_encode(encoder),
            Self::Layout(id) => encoder.fixed(id.as_array()),
            Self::Scan(id) => encoder.fixed(id.as_array()),
            Self::TypeDescriptor(id) | Self::TypeRegistration(id) => encoder.fixed(id.as_array()),
            Self::DispatchTable(id) => encoder.fixed(id.as_array()),
            Self::StaticStorage(id) | Self::StaticStorageRegistration(id) => {
                encoder.fixed(id.as_array())
            }
            Self::InitializationCell(id) | Self::InitializationDescriptor(id) => {
                encoder.fixed(id.as_array())
            }
        }
    }
}

#[cfg(test)]
mod tests;
