//! Release-call conditions in the declaration's existing signature binder scope.

use scoop_identity::SignatureTypeKey;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::ConditionalReleaseCallability;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReleaseValueBinderV1 {
    pub depth: u32,
    pub index: u32,
}

impl ReleaseValueBinderV1 {
    pub fn signature(self) -> SignatureTypeKey {
        SignatureTypeKey::Binder {
            depth: self.depth,
            index: self.index,
        }
    }
}

pub type CallableReleaseCallabilityV1 = ConditionalReleaseCallability<ReleaseValueBinderV1>;

impl WireEncode for CallableReleaseCallabilityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Unavailable => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::NoTransition { requirements } => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                encoder.array(requirements.len() as u64)?;
                for requirement in requirements {
                    encoder.array(2)?;
                    encoder.unsigned(u64::from(requirement.depth))?;
                    encoder.unsigned(u64::from(requirement.index))?;
                }
                Ok(())
            }
        }
    }
}

impl WireDecode for CallableReleaseCallabilityV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let length = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let expected = match tag {
            1 => 1,
            2 => 2,
            tag => return Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        };
        if length != expected {
            return Err(error(
                decoder,
                WireErrorKind::InvalidLength {
                    expected,
                    actual: length,
                },
            ));
        }
        if tag == 1 {
            return Ok(Self::Unavailable);
        }
        let requirements = decoder.field(1, |decoder| {
            decoder.decode_array(|decoder, _| {
                let actual = decoder.array()?;
                if actual != 2 {
                    return Err(error(
                        decoder,
                        WireErrorKind::InvalidLength {
                            expected: 2,
                            actual,
                        },
                    ));
                }
                let mut component = || {
                    let value = decoder.unsigned()?;
                    u32::try_from(value)
                        .map_err(|_| error(decoder, WireErrorKind::IntegerOutOfRange))
                };
                Ok(ReleaseValueBinderV1 {
                    depth: component()?,
                    index: component()?,
                })
            })
        })?;
        if requirements.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(error(decoder, WireErrorKind::NonCanonicalCbor));
        }
        Ok(Self::NoTransition { requirements })
    }
}

fn error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
