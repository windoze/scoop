//! Source receiver types survive logical argument adaptation and substitution.

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceCallReceiver<T> {
    NoReceiver,
    Receiver { static_type: T },
}

impl<T> SourceCallReceiver<T> {
    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> SourceCallReceiver<U> {
        match self {
            Self::NoReceiver => SourceCallReceiver::NoReceiver,
            Self::Receiver { static_type } => SourceCallReceiver::Receiver {
                static_type: map(static_type),
            },
        }
    }

    pub fn try_map<U, E>(
        self,
        map: impl FnOnce(T) -> Result<U, E>,
    ) -> Result<SourceCallReceiver<U>, E> {
        Ok(match self {
            Self::NoReceiver => SourceCallReceiver::NoReceiver,
            Self::Receiver { static_type } => SourceCallReceiver::Receiver {
                static_type: map(static_type)?,
            },
        })
    }

    pub const fn as_ref(&self) -> SourceCallReceiver<&T> {
        match self {
            Self::NoReceiver => SourceCallReceiver::NoReceiver,
            Self::Receiver { static_type } => SourceCallReceiver::Receiver { static_type },
        }
    }

    pub const fn has_receiver(&self) -> bool {
        matches!(self, Self::Receiver { .. })
    }
}

impl<T: WireEncode> WireEncode for SourceCallReceiver<T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NoReceiver => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::Receiver { static_type } => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                static_type.encode(encoder)
            }
        }
    }
}

impl<T: WireDecode> WireDecode for SourceCallReceiver<T> {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let expected = match tag {
            1 => 1,
            2 => 2,
            _ => return Err(error(decoder, WireErrorKind::UnknownTag { tag })),
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
            Ok(Self::NoReceiver)
        } else {
            Ok(Self::Receiver {
                static_type: decoder.field(1, T::decode)?,
            })
        }
    }
}

fn error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
