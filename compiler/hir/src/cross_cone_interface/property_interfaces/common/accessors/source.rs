use scoop_identity::PersistentPropertyAccessorId;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

/// Source form of one accessor. Body identity is already its typed accessor id.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PropertyAccessorImplementationV1 {
    Storage,
    Constant,
    Body,
    AbstractSlot,
}

impl PropertyAccessorImplementationV1 {
    pub const fn from_source(source: crate::PropertyAccessorImplementation) -> Self {
        match source {
            crate::PropertyAccessorImplementation::Storage => Self::Storage,
            crate::PropertyAccessorImplementation::Constant => Self::Constant,
            crate::PropertyAccessorImplementation::Body(_) => Self::Body,
            crate::PropertyAccessorImplementation::AbstractSlot(_) => Self::AbstractSlot,
        }
    }

    pub const fn requires_body(self) -> bool {
        matches!(self, Self::Body | Self::AbstractSlot)
    }
}

impl WireEncode for PropertyAccessorImplementationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Storage => 1,
            Self::Constant => 2,
            Self::Body => 3,
            Self::AbstractSlot => 4,
        })
    }
}

impl WireDecode for PropertyAccessorImplementationV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Storage),
            2 => Ok(Self::Constant),
            3 => Ok(Self::Body),
            4 => Ok(Self::AbstractSlot),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PropertyAccessorSourceV1 {
    accessor: PersistentPropertyAccessorId,
    implementation: PropertyAccessorImplementationV1,
}

impl PropertyAccessorSourceV1 {
    pub const fn new(
        accessor: PersistentPropertyAccessorId,
        implementation: PropertyAccessorImplementationV1,
    ) -> Self {
        Self {
            accessor,
            implementation,
        }
    }

    pub const fn accessor(self) -> PersistentPropertyAccessorId {
        self.accessor
    }

    pub const fn implementation(self) -> PropertyAccessorImplementationV1 {
        self.implementation
    }
}

impl WireEncode for PropertyAccessorSourceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.accessor.encode(encoder)?;
        encoder.field(2)?;
        self.implementation.encode(encoder)
    }
}
