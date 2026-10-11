use super::IntegerKind;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

impl IntegerKind {
    pub(crate) const fn wire_tag(self) -> u64 {
        self.index() as u64 + 1
    }

    pub(crate) fn from_wire_tag(tag: u64) -> Option<Self> {
        usize::try_from(tag.checked_sub(1)?)
            .ok()
            .and_then(|index| Self::ALL.get(index).copied())
    }
}

impl WireEncode for IntegerKind {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(self.wire_tag())
    }
}

impl WireDecode for IntegerKind {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let tag = decoder.unsigned()?;
        Self::from_wire_tag(tag).ok_or_else(|| {
            WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )
        })
    }
}
