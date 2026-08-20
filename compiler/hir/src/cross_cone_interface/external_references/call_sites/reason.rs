//! An actual source call's lookup route or direct declaration reference.

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirDependencyCallReasonV1 {
    SourceBinding(Vec<u32>),
    SourceDeclaration,
}

impl WireEncode for HirDependencyCallReasonV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (fields, tag) = match self {
            Self::SourceBinding(_) => (2, 1),
            Self::SourceDeclaration => (1, 3),
        };
        encoder.map(fields)?;
        encoder.field(0)?;
        encoder.unsigned(tag)?;
        match self {
            Self::SourceBinding(indices) => {
                encoder.field(1)?;
                super::encode_indices(indices, encoder)
            }
            Self::SourceDeclaration => Ok(()),
        }
    }
}

impl WireDecode for HirDependencyCallReasonV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        if fields == 0 {
            return Err(wire_error(
                decoder,
                WireErrorKind::MissingField { field: 0 },
            ));
        }
        let tag = decoder.field(0, Decoder::unsigned)?;
        let expected = match tag {
            1 => 2,
            3 => 1,
            tag => return Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        };
        if fields != expected {
            return Err(wire_error(
                decoder,
                WireErrorKind::InvalidLength {
                    expected,
                    actual: fields,
                },
            ));
        }
        match tag {
            1 => decoder
                .field(1, |d| d.decode_array(|d, _| d.u32()))
                .map(Self::SourceBinding),
            3 => Ok(Self::SourceDeclaration),
            _ => unreachable!("call reason tag was checked above"),
        }
    }
}

fn wire_error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retired_runtime_reason_tag_is_rejected() {
        let error =
            scoop_wire::decode_canonical::<HirDependencyCallReasonV1>(&[0xa1, 0, 2]).unwrap_err();
        assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 2 });
    }
}
