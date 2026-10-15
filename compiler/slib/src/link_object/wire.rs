//! Fixed-width wire fields shared by Link metadata readers.

use std::marker::PhantomData;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

/// A typed 32-byte field whose width is checked during decoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DecodedFixedBytesV1<I> {
    bytes: [u8; 32],
    marker: PhantomData<fn() -> I>,
}

impl<I> DecodedFixedBytesV1<I> {
    pub(crate) const fn bytes(&self) -> &[u8; 32] {
        &self.bytes
    }

    pub(crate) fn matches(&self, expected: &[u8; 32]) -> bool {
        self.bytes == *expected
    }
}

impl<I> WireEncode for DecodedFixedBytesV1<I> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.bytes)
    }
}

impl<I> WireDecode for DecodedFixedBytesV1<I> {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let bytes = decoder.bytes()?;
        let bytes = <&[u8; 32]>::try_from(bytes).copied().map_err(|_| {
            WireError::new(
                WireErrorKind::InvalidLength {
                    expected: 32,
                    actual: bytes.len() as u64,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            )
        })?;
        Ok(Self {
            bytes,
            marker: PhantomData,
        })
    }
}
