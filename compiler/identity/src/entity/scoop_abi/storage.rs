use super::*;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ScoopAbiValueShape {
    Scalar,
    Aggregate,
    Interface,
}

impl WireEncode for ScoopAbiValueShape {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Scalar => 1,
            Self::Aggregate => 2,
            Self::Interface => 3,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalScoopStorage {
    exact_type: PersistentExactTypeId,
    byte_size: u64,
    alignment: NonZeroU64,
    shape: ScoopAbiValueShape,
}

impl CanonicalScoopStorage {
    pub const fn new(
        exact_type: PersistentExactTypeId,
        byte_size: u64,
        alignment: NonZeroU64,
        shape: ScoopAbiValueShape,
    ) -> Self {
        Self {
            exact_type,
            byte_size,
            alignment,
            shape,
        }
    }

    pub const fn exact_type(self) -> PersistentExactTypeId {
        self.exact_type
    }

    pub const fn byte_size(self) -> u64 {
        self.byte_size
    }

    pub const fn alignment(self) -> NonZeroU64 {
        self.alignment
    }

    pub const fn shape(self) -> ScoopAbiValueShape {
        self.shape
    }
}

impl WireEncode for CanonicalScoopStorage {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.exact_type.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(self.byte_size)?;
        encoder.field(3)?;
        encoder.unsigned(self.alignment.get())?;
        encoder.field(4)?;
        self.shape.encode(encoder)
    }
}
