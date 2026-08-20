use scoop_identity::PersistentExactTypeId;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

/// The exact nominal owner identifies its unique release body across stages.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MirClassReleasePolicyV1<Owner = PersistentExactTypeId> {
    #[default]
    None,
    SynchronousGcFree {
        owner: Owner,
    },
}

impl<Owner: WireEncode> WireEncode for MirClassReleasePolicyV1<Owner> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::None => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::SynchronousGcFree { owner } => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                owner.encode(encoder)
            }
        }
    }
}

impl<Owner: WireDecode> WireDecode for MirClassReleasePolicyV1<Owner> {
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
        match tag {
            1 => Ok(Self::None),
            _ => Ok(Self::SynchronousGcFree {
                owner: decoder.field(1, Owner::decode)?,
            }),
        }
    }
}

fn error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
