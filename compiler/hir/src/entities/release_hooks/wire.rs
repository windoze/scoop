use super::ReleasePolicy;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

impl<H: WireEncode> WireEncode for ReleasePolicy<H> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::None => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::SynchronousGcFree { hook } => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                hook.encode(encoder)
            }
        }
    }
}

impl<H: WireDecode> WireDecode for ReleasePolicy<H> {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let expected = match tag {
            1 => 1,
            2 => 2,
            tag => return Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        };
        if fields != expected {
            return Err(error(
                decoder,
                WireErrorKind::InvalidLength {
                    expected,
                    actual: fields,
                },
            ));
        }
        if tag == 1 {
            Ok(Self::None)
        } else {
            Ok(Self::SynchronousGcFree {
                hook: decoder.field(1, H::decode)?,
            })
        }
    }
}

fn error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
