use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

/// Caller protocol of one source C extern declaration. This is independent of
/// the physical C symbol's ABI and must not enter its contract merge key.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CAbiCallMode {
    NativeSafe,
    GcLeaf,
}

impl WireEncode for CAbiCallMode {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::NativeSafe => 1,
            Self::GcLeaf => 2,
        })
    }
}

impl WireDecode for CAbiCallMode {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::NativeSafe),
            2 => Ok(Self::GcLeaf),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}
