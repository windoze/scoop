use std::fmt;

use la_arena::Arena;
use scoop_identity::{CborIdentityRecord, DispatchRole, DispatchSlotKey, PersistentDispatchSlotId};

use super::{InterfaceDef, InterfaceId, InterfaceMethodSlot, VirtualMethodId};

pub type DispatchSlotIdentityRecord = CborIdentityRecord<PersistentDispatchSlotId, DispatchSlotKey>;

/// Total persistent dispatch-slot relation for the LocalConcrete graph.
///
/// Virtual families and concrete interface applications keep their own local
/// ids, while every location points back to the exact declaration-level slot
/// identity established by Export HIR.
#[derive(Clone, Debug)]
pub struct DispatchSlotIdentities {
    virtual_slots: Vec<DispatchSlotIdentityRecord>,
    interface_slots: Vec<Vec<DispatchSlotIdentityRecord>>,
}

impl DispatchSlotIdentities {
    pub fn checked(
        virtual_slots: Vec<DispatchSlotIdentityRecord>,
        interface_slots: Vec<Vec<DispatchSlotIdentityRecord>>,
        interfaces: &Arena<InterfaceDef>,
    ) -> Result<Self, DispatchSlotIdentityRelationError> {
        for (family, record) in virtual_slots.iter().enumerate() {
            if record.key().role() == DispatchRole::InterfaceMethod {
                return Err(DispatchSlotIdentityRelationError::VirtualRole {
                    family: family as u32,
                });
            }
        }
        if interface_slots.len() != interfaces.len() {
            return Err(DispatchSlotIdentityRelationError::InterfaceCount {
                expected: interfaces.len(),
                actual: interface_slots.len(),
            });
        }
        for ((interface, definition), slots) in interfaces.iter().zip(&interface_slots) {
            if slots.len() != definition.methods.len() {
                return Err(DispatchSlotIdentityRelationError::InterfaceMethodCount {
                    interface: interface.into_raw().into_u32(),
                    expected: definition.methods.len(),
                    actual: slots.len(),
                });
            }
            for (slot, record) in slots.iter().enumerate() {
                if record.key().role() == DispatchRole::VirtualMethod {
                    return Err(DispatchSlotIdentityRelationError::InterfaceRole {
                        interface: interface.into_raw().into_u32(),
                        slot: slot as u32,
                    });
                }
            }
        }
        Ok(Self {
            virtual_slots,
            interface_slots,
        })
    }

    pub fn virtual_slot(&self, family: VirtualMethodId) -> &DispatchSlotIdentityRecord {
        &self.virtual_slots[family.into_raw() as usize]
    }

    pub fn interface_slot(
        &self,
        interface: InterfaceId,
        slot: InterfaceMethodSlot,
    ) -> &DispatchSlotIdentityRecord {
        &self.interface_slots[interface.into_raw().into_u32() as usize][slot.into_raw() as usize]
    }

    pub fn virtual_slots(
        &self,
    ) -> impl ExactSizeIterator<Item = (VirtualMethodId, &DispatchSlotIdentityRecord)> {
        self.virtual_slots
            .iter()
            .enumerate()
            .map(|(index, record)| (VirtualMethodId::from_raw(index as u32), record))
    }

    pub fn interface_slots(
        &self,
        interface: InterfaceId,
    ) -> impl ExactSizeIterator<Item = (InterfaceMethodSlot, &DispatchSlotIdentityRecord)> {
        self.interface_slots[interface.into_raw().into_u32() as usize]
            .iter()
            .enumerate()
            .map(|(index, record)| (InterfaceMethodSlot::from_raw(index as u32), record))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DispatchSlotIdentityRelationError {
    VirtualRole {
        family: u32,
    },
    InterfaceCount {
        expected: usize,
        actual: usize,
    },
    InterfaceMethodCount {
        interface: u32,
        expected: usize,
        actual: usize,
    },
    InterfaceRole {
        interface: u32,
        slot: u32,
    },
}

impl fmt::Display for DispatchSlotIdentityRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VirtualRole { family } => write!(
                formatter,
                "local virtual family {family} carries an interface-method slot identity"
            ),
            Self::InterfaceCount { expected, actual } => write!(
                formatter,
                "local interface identity relation has {actual} entries, expected {expected}"
            ),
            Self::InterfaceMethodCount {
                interface,
                expected,
                actual,
            } => write!(
                formatter,
                "local interface {interface} has {actual} slot identities, expected {expected}"
            ),
            Self::InterfaceRole { interface, slot } => write!(
                formatter,
                "local interface {interface} slot {slot} carries a virtual-method identity"
            ),
        }
    }
}

impl std::error::Error for DispatchSlotIdentityRelationError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, PackagePath,
        PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite,
    };

    use super::*;

    #[test]
    fn relation_rejects_an_interface_method_identity_in_a_virtual_family() {
        let declaration = SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                ConeIdentity::SINGLE_FILE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("run").unwrap(),
            0,
            None,
            Vec::new(),
        );
        let function = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
        let record =
            CborIdentityRecord::from_key(DispatchSlotKey::interface_method(function)).unwrap();

        assert_eq!(
            DispatchSlotIdentities::checked(vec![record], Vec::new(), &Arena::new()).unwrap_err(),
            DispatchSlotIdentityRelationError::VirtualRole { family: 0 }
        );
    }
}
