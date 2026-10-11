use scoop_wire::{Decoder, Encoder, WireError, WireErrorKind};

/// A lexical loop distance, resolved to a fresh LoopId when a body is materialized.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultLoopTargetV1(u32);

impl DefaultLoopTargetV1 {
    pub const INNERMOST: Self = Self(0);

    pub const fn new(depth: u32) -> Self {
        Self(depth)
    }

    pub const fn depth(self) -> u32 {
        self.0
    }

    pub(super) fn encode_jump(
        self,
        encoder: &mut Encoder,
        tag: u64,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(if self.0 == 0 { 1 } else { 2 })?;
        encoder.field(0)?;
        encoder.unsigned(tag)?;
        if self.0 != 0 {
            encoder.field(1)?;
            encoder.unsigned(u64::from(self.0))?;
        }
        Ok(())
    }

    pub(super) fn decode_jump(decoder: &mut Decoder<'_>, fields: u64) -> Result<Self, WireError> {
        if fields == 1 {
            return Ok(Self::INNERMOST);
        }
        if fields != 2 {
            return Err(error(
                decoder,
                WireErrorKind::InvalidLength {
                    expected: 2,
                    actual: fields,
                },
            ));
        }
        decoder.field(1, |decoder| {
            let depth = decoder.u32()?;
            if depth == 0 {
                return Err(error(decoder, WireErrorKind::NonCanonicalCbor));
            }
            Ok(Self(depth))
        })
    }
}

fn error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
