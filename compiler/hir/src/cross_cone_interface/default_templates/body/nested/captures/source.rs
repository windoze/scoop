use scoop_identity::LocalValueSelector;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

/// The value read when a lexical closure or local-function capture is formed.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultCaptureSourceV1 {
    Local(LocalValueSelector),
    EnclosingCapture(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum IndexedCaptureSource {
    Local(u32),
    EnclosingCapture(u32),
}

impl WireEncode for IndexedCaptureSource {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        let (tag, index) = match self {
            Self::Local(index) => (1, *index),
            Self::EnclosingCapture(index) => (2, *index),
        };
        encoder.unsigned(tag)?;
        encoder.field(1)?;
        encoder.unsigned(u64::from(index))
    }
}

impl WireDecode for IndexedCaptureSource {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let index = decoder.field(1, Decoder::u32)?;
        match tag {
            1 => Ok(Self::Local(index)),
            2 => Ok(Self::EnclosingCapture(index)),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}
