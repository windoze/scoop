use std::fmt;

use scoop_identity::PersistentDispatchSlotId;
use scoop_wire::{Encoder, WireEncode};

use super::wire;
use crate::{CallableModalityV1, DeclarationAccessSourceV1, InheritanceSlotSchemaRoleV1};

mod declaration;
mod decode;
mod resolution;
mod signature;
mod table;
mod target;
#[cfg(test)]
pub(in crate::cross_cone_type_semantics) mod tests;
mod validation;

pub use declaration::*;
pub use decode::*;
pub use resolution::*;
pub use signature::*;
pub use table::*;
pub use target::*;
pub use validation::*;

/// A complete root-slot contract with a separate selected implementation.
/// Construction checks local shape only; source and graph joins are separate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InheritanceSlotContractV1 {
    role: InheritanceSlotSchemaRoleV1,
    slot: PersistentDispatchSlotId,
    declaration: InheritanceCallableDeclarationV1,
    signature: InheritanceCallableSignatureV1,
    implementation: InheritanceSlotImplementationV1,
    declaration_access: DeclarationAccessSourceV1,
}
impl InheritanceSlotContractV1 {
    pub fn try_new(
        role: InheritanceSlotSchemaRoleV1,
        slot: PersistentDispatchSlotId,
        declaration: InheritanceCallableDeclarationV1,
        signature: InheritanceCallableSignatureV1,
        implementation: InheritanceSlotImplementationV1,
        declaration_access: DeclarationAccessSourceV1,
    ) -> Result<Self, InheritanceSlotContractBuildError> {
        let target = implementation.target();
        if !target.signature().matches_slot(&signature, role) {
            return Err(InheritanceSlotContractBuildError::SignatureMismatch);
        }
        if matches!(implementation, InheritanceSlotImplementationV1::Abstract(_))
            != (target.modality() == CallableModalityV1::Abstract)
        {
            return Err(InheritanceSlotContractBuildError::AbstractModality);
        }
        if matches!(
            implementation,
            InheritanceSlotImplementationV1::InterfaceDefault(_)
        ) != (target.modality() == CallableModalityV1::InterfaceDefault)
        {
            return Err(InheritanceSlotContractBuildError::DefaultModality);
        }
        let same_role = match (declaration, target.declaration()) {
            (InheritanceCallableDeclarationV1::DerivedEquality(_), _) => false,
            (
                InheritanceCallableDeclarationV1::Function(_),
                InheritanceCallableDeclarationV1::DerivedEquality(_),
            ) => {
                matches!(role, InheritanceSlotSchemaRoleV1::Interface { .. })
                    && matches!(implementation, InheritanceSlotImplementationV1::Concrete(_))
                    && target.modality() == CallableModalityV1::Final
            }
            (root, selected) => std::mem::discriminant(&root) == std::mem::discriminant(&selected),
        };
        if !same_role {
            return Err(InheritanceSlotContractBuildError::DeclarationRole);
        }
        Ok(Self {
            role,
            slot,
            declaration,
            signature,
            implementation,
            declaration_access,
        })
    }
    pub const fn role(&self) -> InheritanceSlotSchemaRoleV1 {
        self.role
    }
    pub const fn key(&self) -> (InheritanceSlotSchemaRoleV1, PersistentDispatchSlotId) {
        (self.role, self.slot)
    }
    pub const fn slot(&self) -> PersistentDispatchSlotId {
        self.slot
    }
    pub const fn declaration(&self) -> InheritanceCallableDeclarationV1 {
        self.declaration
    }
    pub const fn signature(&self) -> &InheritanceCallableSignatureV1 {
        &self.signature
    }
    pub const fn implementation(&self) -> &InheritanceSlotImplementationV1 {
        &self.implementation
    }
    pub const fn declaration_access(&self) -> &DeclarationAccessSourceV1 {
        &self.declaration_access
    }
}
impl WireEncode for InheritanceSlotContractV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(0)?;
        self.role.encode(encoder)?;
        encoder.field(1)?;
        self.slot.encode(encoder)?;
        encoder.field(3)?;
        self.declaration.encode(encoder)?;
        encoder.field(4)?;
        self.signature.encode(encoder)?;
        encoder.field(6)?;
        self.implementation.encode(encoder)?;
        encoder.field(7)?;
        self.declaration_access.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InheritanceSlotContractBuildError {
    AbstractModality,
    SignatureMismatch,
    DefaultModality,
    DeclarationRole,
    SlotOrder { index: usize },
}
impl fmt::Display for InheritanceSlotContractBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AbstractModality => {
                f.write_str("slot implementation variant disagrees with target abstract modality")
            }
            Self::SignatureMismatch => {
                f.write_str("slot target signature or effects disagree with the root contract")
            }
            Self::DefaultModality => {
                f.write_str("slot implementation variant disagrees with interface default modality")
            }
            Self::DeclarationRole => {
                f.write_str("slot target declaration role differs from the root declaration")
            }
            Self::SlotOrder { index } => write!(
                f,
                "duplicate or noncanonical inheritance slot at index {index}"
            ),
        }
    }
}
impl std::error::Error for InheritanceSlotContractBuildError {}
