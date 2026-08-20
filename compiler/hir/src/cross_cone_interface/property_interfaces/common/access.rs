use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

/// Public direct/slot classification of a logical property and its getter.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PropertyPublicAccessV1 {
    DirectOnly,
    PublicSlot,
}

impl WireEncode for PropertyPublicAccessV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::DirectOnly => 1,
            Self::PublicSlot => 2,
        })
    }
}

impl WireDecode for PropertyPublicAccessV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::DirectOnly),
            2 => Ok(Self::PublicSlot),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

/// Foreign visibility of the setter carried by a read-write property.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PropertySetterPublicAccessV1 {
    Restricted,
    Public,
}

impl WireEncode for PropertySetterPublicAccessV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Restricted => 1,
            Self::Public => 2,
        })
    }
}

impl WireDecode for PropertySetterPublicAccessV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Restricted),
            2 => Ok(Self::Public),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

/// Cross-Cone access category without provider-private storage identities.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PropertyRepresentationV1 {
    Const,
    RuntimeAccessor,
    AbstractSlot,
}

impl WireEncode for PropertyRepresentationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Const => 1,
            Self::RuntimeAccessor => 2,
            Self::AbstractSlot => 3,
        })
    }
}

impl WireDecode for PropertyRepresentationV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Const),
            2 => Ok(Self::RuntimeAccessor),
            3 => Ok(Self::AbstractSlot),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

fn unknown_tag(decoder: &Decoder<'_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}
