//! A Unicode scalar, encoded as its unsigned code point.

use super::*;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalCharV1(char);

impl CanonicalCharV1 {
    pub const fn value(self) -> char {
        self.0
    }
}
impl From<char> for CanonicalCharV1 {
    fn from(value: char) -> Self {
        Self(value)
    }
}
impl From<CanonicalCharV1> for char {
    fn from(value: CanonicalCharV1) -> Self {
        value.0
    }
}
impl WireEncode for CanonicalCharV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(self.0 as u64)
    }
}
impl WireDecode for CanonicalCharV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let code = decoder.unsigned()?;
        u32::try_from(code)
            .ok()
            .and_then(char::from_u32)
            .map(Self)
            .ok_or_else(|| wire_error(decoder, WireErrorKind::IntegerOutOfRange))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoop_wire::{decode_canonical, encode};

    struct UncheckedScalar(u64);

    impl WireEncode for UncheckedScalar {
        fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
            encoder.unsigned(self.0)
        }
    }

    #[test]
    fn scalar_boundaries_round_trip_and_invalid_code_points_are_rejected() {
        for value in [
            '\0',
            '\u{7f}',
            '\u{7ff}',
            '\u{d7ff}',
            '\u{e000}',
            '\u{10ffff}',
        ] {
            let scalar = CanonicalCharV1::from(value);
            let bytes = encode(&scalar).unwrap();
            assert_eq!(decode_canonical::<CanonicalCharV1>(&bytes).unwrap(), scalar);
        }
        for code in [0xd800_u64, 0xdfff, 0x110000, u64::MAX] {
            let bytes = encode(&UncheckedScalar(code)).unwrap();
            assert!(decode_canonical::<CanonicalCharV1>(&bytes).is_err());
        }
    }
}
