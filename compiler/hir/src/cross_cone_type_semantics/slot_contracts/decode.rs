use scoop_identity::{DecodedPersistentId, PersistentDispatchSlotId, PersistentTypeId};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{DecodedInheritanceCallableDeclarationV1, DecodedInheritanceCallableSignatureV1, wire};
use crate::{
    CallableModalityV1, DecodedDeclarationAccessSourceV1, DecodedPersistentAccessDomainV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedInheritanceSlotTargetV1 {
    pub(super) declaration: DecodedInheritanceCallableDeclarationV1,
    pub(super) owner: DecodedPersistentId<PersistentTypeId>,
    pub(super) signature: DecodedInheritanceCallableSignatureV1,
    pub(super) modality: CallableModalityV1,
    pub(super) declaration_access: DecodedDeclarationAccessSourceV1,
}
impl WireEncode for DecodedInheritanceSlotTargetV1 {
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
impl WireDecode for DecodedInheritanceSlotTargetV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        Ok(Self {
            declaration: decoder.field(1, DecodedInheritanceCallableDeclarationV1::decode)?,
            owner: decoder.field(2, DecodedPersistentId::decode)?,
            signature: decoder.field(3, DecodedInheritanceCallableSignatureV1::decode)?,
            modality: decoder.field(4, CallableModalityV1::decode)?,
            declaration_access: decoder.field(5, DecodedDeclarationAccessSourceV1::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedInheritanceSlotImplementationV1 {
    Abstract,
    Concrete(DecodedInheritanceSlotTargetV1),
    InterfaceDefault(DecodedInheritanceSlotTargetV1),
}
impl WireEncode for DecodedInheritanceSlotImplementationV1 {
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
impl WireDecode for DecodedInheritanceSlotImplementationV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                wire::expect_fields(decoder, fields, 1)?;
                Ok(Self::Abstract)
            }
            tag @ (2 | 3) => {
                wire::expect_fields(decoder, fields, 2)?;
                let target = decoder.field(1, DecodedInheritanceSlotTargetV1::decode)?;
                Ok(if tag == 2 {
                    Self::Concrete(target)
                } else {
                    Self::InterfaceDefault(target)
                })
            }
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedInheritanceSlotContractV1 {
    pub(super) slot: DecodedPersistentId<PersistentDispatchSlotId>,
    pub(super) declaration_owner: DecodedPersistentId<PersistentTypeId>,
    pub(super) declaration: DecodedInheritanceCallableDeclarationV1,
    pub(super) signature: DecodedInheritanceCallableSignatureV1,
    pub(super) domain: DecodedPersistentAccessDomainV1,
    pub(super) implementation: DecodedInheritanceSlotImplementationV1,
    pub(super) declaration_access: DecodedDeclarationAccessSourceV1,
}
impl WireEncode for DecodedInheritanceSlotContractV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(7)?;
        encoder.field(1)?;
        self.slot.encode(encoder)?;
        encoder.field(2)?;
        self.declaration_owner.encode(encoder)?;
        encoder.field(3)?;
        self.declaration.encode(encoder)?;
        encoder.field(4)?;
        self.signature.encode(encoder)?;
        encoder.field(5)?;
        self.domain.encode(encoder)?;
        encoder.field(6)?;
        self.implementation.encode(encoder)?;
        encoder.field(7)?;
        self.declaration_access.encode(encoder)
    }
}
impl WireDecode for DecodedInheritanceSlotContractV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(7)?;
        Ok(Self {
            slot: decoder.field(1, DecodedPersistentId::decode)?,
            declaration_owner: decoder.field(2, DecodedPersistentId::decode)?,
            declaration: decoder.field(3, DecodedInheritanceCallableDeclarationV1::decode)?,
            signature: decoder.field(4, DecodedInheritanceCallableSignatureV1::decode)?,
            domain: decoder.field(5, DecodedPersistentAccessDomainV1::decode)?,
            implementation: decoder.field(6, DecodedInheritanceSlotImplementationV1::decode)?,
            declaration_access: decoder.field(7, DecodedDeclarationAccessSourceV1::decode)?,
        })
    }
}
