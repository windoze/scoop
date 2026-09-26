use super::InheritanceSourceSlotSelectionV1;
use crate::DecodedInheritanceCallableDeclarationV1;
use crate::cross_cone_type_semantics::wire as codec;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

impl WireEncode for InheritanceSourceSlotSelectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, declaration) = match self {
            Self::Abstract => return codec::tag(encoder, 1, 1),
            Self::Concrete(declaration) => (2, declaration),
            Self::InterfaceDefault(declaration) => (3, declaration),
        };
        codec::tag(encoder, 2, tag)?;
        encoder.field(1)?;
        declaration.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedInheritanceSourceSlotSelectionV1 {
    Abstract,
    Concrete(DecodedInheritanceCallableDeclarationV1),
    InterfaceDefault(DecodedInheritanceCallableDeclarationV1),
}

impl WireEncode for DecodedInheritanceSourceSlotSelectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, declaration) = match self {
            Self::Abstract => return codec::tag(encoder, 1, 1),
            Self::Concrete(declaration) => (2, declaration),
            Self::InterfaceDefault(declaration) => (3, declaration),
        };
        codec::tag(encoder, 2, tag)?;
        encoder.field(1)?;
        declaration.encode(encoder)
    }
}

impl WireDecode for DecodedInheritanceSourceSlotSelectionV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                codec::expect_fields(decoder, fields, 1)?;
                Ok(Self::Abstract)
            }
            tag @ (2 | 3) => {
                codec::expect_fields(decoder, fields, 2)?;
                let declaration =
                    decoder.field(1, DecodedInheritanceCallableDeclarationV1::decode)?;
                Ok(if tag == 2 {
                    Self::Concrete(declaration)
                } else {
                    Self::InterfaceDefault(declaration)
                })
            }
            tag => Err(codec::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
