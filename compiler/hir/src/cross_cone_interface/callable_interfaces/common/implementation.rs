use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::IntrinsicFunctionKind;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableImplementationV1 {
    Scoop,
    Intrinsic(IntrinsicFunctionKind),
    SourceExternScoop,
    SourceExternC,
}

impl WireEncode for CallableImplementationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(if matches!(self, Self::Intrinsic(_)) {
            2
        } else {
            1
        })?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Scoop => 1,
            Self::Intrinsic(_) => 2,
            Self::SourceExternScoop => 3,
            Self::SourceExternC => 4,
        })?;
        if let Self::Intrinsic(kind) = self {
            encoder.field(1)?;
            kind.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for CallableImplementationV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        if fields == 0 {
            return Err(error(decoder, WireErrorKind::MissingField { field: 0 }));
        }
        let tag = decoder.field(0, Decoder::unsigned)?;
        let expected = match tag {
            1 | 3 | 4 => 1,
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
        match tag {
            1 => Ok(Self::Scoop),
            2 => decoder
                .field(1, IntrinsicFunctionKind::decode)
                .map(Self::Intrinsic),
            3 => Ok(Self::SourceExternScoop),
            4 => Ok(Self::SourceExternC),
            _ => unreachable!("the implementation tag was validated above"),
        }
    }
}

fn error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests;
