//! Shared untrusted fixed-width wire carriers for Link projections.

use std::marker::PhantomData;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

/// A typed 32-byte wire carrier which deliberately provides no promotion to
/// its marker type. Semantic readers may only re-encode it for comparison
/// with a projection rebuilt from trusted proofs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DecodedFixedBytesV1<I> {
    bytes: [u8; 32],
    marker: PhantomData<fn() -> I>,
}

impl<I> DecodedFixedBytesV1<I> {
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
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
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
