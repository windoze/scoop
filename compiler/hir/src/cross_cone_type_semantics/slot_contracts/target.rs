use scoop_identity::PersistentTypeId;
use scoop_wire::{Encoder, WireEncode};

use super::{
    InheritanceCallableDeclarationV1, InheritanceCallableSignatureV1,
    InheritanceSlotContractBuildError, wire,
};
use crate::{CallableModalityV1, DeclarationAccessSourceV1};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InheritanceSlotTargetV1 {
    pub(super) declaration: InheritanceCallableDeclarationV1,
    pub(super) owner: PersistentTypeId,
    pub(super) signature: InheritanceCallableSignatureV1,
    pub(super) modality: CallableModalityV1,
    pub(super) declaration_access: DeclarationAccessSourceV1,
}
impl InheritanceSlotTargetV1 {
    pub fn try_new(
        declaration: InheritanceCallableDeclarationV1,
        owner: PersistentTypeId,
        signature: InheritanceCallableSignatureV1,
        modality: CallableModalityV1,
        declaration_access: DeclarationAccessSourceV1,
    ) -> Result<Self, InheritanceSlotContractBuildError> {
        if modality == CallableModalityV1::Abstract {
            return Err(InheritanceSlotContractBuildError::AbstractTarget);
        }
        Ok(Self {
            declaration,
            owner,
            signature,
            modality,
            declaration_access,
        })
    }
    pub const fn declaration(&self) -> InheritanceCallableDeclarationV1 {
        self.declaration
    }
    pub const fn owner(&self) -> PersistentTypeId {
        self.owner
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
}
impl WireEncode for InheritanceSlotTargetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.owner.encode(encoder)?;
        encoder.field(3)?;
        self.signature.encode(encoder)?;
        encoder.field(4)?;
        self.modality.encode(encoder)?;
        encoder.field(5)?;
        self.declaration_access.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InheritanceSlotImplementationV1 {
    Abstract,
    Concrete(InheritanceSlotTargetV1),
    InterfaceDefault(InheritanceSlotTargetV1),
}
impl InheritanceSlotImplementationV1 {
    pub const fn target(&self) -> Option<&InheritanceSlotTargetV1> {
        match self {
            Self::Abstract => None,
            Self::Concrete(target) | Self::InterfaceDefault(target) => Some(target),
        }
    }
}
impl WireEncode for InheritanceSlotImplementationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Abstract => wire::tag(encoder, 1, 1),
            Self::Concrete(target) | Self::InterfaceDefault(target) => {
                wire::tag(
                    encoder,
                    2,
                    if matches!(self, Self::Concrete(_)) {
                        2
                    } else {
                        3
                    },
                )?;
                encoder.field(1)?;
                target.encode(encoder)
            }
        }
    }
}
