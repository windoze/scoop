use crate::{PathSegment, WireError, WireErrorKind, WirePath, WireType};

use super::encode::encode_with_limit;
use super::{EncodeError, WireEncode};

/// Type-directed strict decoder for the Wire CBOR v1 subset.
pub struct Decoder<'input> {
    input: &'input [u8],
    inner: minicbor::Decoder<'input>,

    path: WirePath,
}

impl<'input> Decoder<'input> {
    pub fn new(input: &'input [u8]) -> Result<Self, WireError> {
        if input.is_empty() {
            return Err(WireError::new(
                WireErrorKind::UnexpectedEnd,
                WirePath::root(),
                Some(0),
            ));
        }
        Ok(Self {
            input,
            inner: minicbor::Decoder::new(input),
            path: WirePath::root(),
        })
    }

    pub fn position(&self) -> u64 {
        self.inner.position() as u64
    }

    pub fn path(&self) -> &WirePath {
        &self.path
    }

    pub fn unsigned(&mut self) -> Result<u64, WireError> {
        let start = self.inner.position();
        let value = self
            .inner
            .u64()
            .map_err(|_| self.type_error(WireType::Unsigned))?;
        self.require_canonical_head(start, 0, value)?;
        Ok(value)
    }

    pub fn u32(&mut self) -> Result<u32, WireError> {
        let value = self.unsigned()?;
        u32::try_from(value).map_err(|_| self.error(WireErrorKind::IntegerOutOfRange))
    }

    pub fn bytes(&mut self) -> Result<&'input [u8], WireError> {
        self.decode_bytes()
    }

    fn decode_bytes(&mut self) -> Result<&'input [u8], WireError> {
        let start = self.inner.position();
        if self.input.get(start) == Some(&0x5f) {
            return Err(self.error_at(
                WireErrorKind::IndefiniteLength {
                    expected: WireType::Bytes,
                },
                start,
            ));
        }
        let value = self
            .inner
            .bytes()
            .map_err(|_| self.type_error(WireType::Bytes))?;
        let length =
            u64::try_from(value.len()).map_err(|_| self.error(WireErrorKind::IntegerOutOfRange))?;
        self.require_canonical_head(start, 2, length)?;
        Ok(value)
    }

    pub fn text(&mut self) -> Result<&'input str, WireError> {
        let start = self.inner.position();
        if self.input.get(start) == Some(&0x7f) {
            return Err(self.error_at(
                WireErrorKind::IndefiniteLength {
                    expected: WireType::Text,
                },
                start,
            ));
        }
        let value = self
            .inner
            .str()
            .map_err(|_| self.type_error(WireType::Text))?;
        let length =
            u64::try_from(value.len()).map_err(|_| self.error(WireErrorKind::IntegerOutOfRange))?;

        self.require_canonical_head(start, 3, length)?;
        Ok(value)
    }

    pub fn owned_bytes(&mut self) -> Result<Vec<u8>, WireError> {
        let value = self.bytes()?;
        self.copy_bytes(value)
    }

    fn copy_bytes(&mut self, value: &[u8]) -> Result<Vec<u8>, WireError> {
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(value.len())
            .map_err(|_| self.error(WireErrorKind::Allocation))?;
        owned.extend_from_slice(value);
        Ok(owned)
    }

    pub fn owned_text(&mut self) -> Result<String, WireError> {
        let value = self.text()?;

        let mut owned = String::new();
        owned
            .try_reserve_exact(value.len())
            .map_err(|_| self.error(WireErrorKind::Allocation))?;
        owned.push_str(value);
        Ok(owned)
    }

    pub fn array(&mut self) -> Result<u64, WireError> {
        let start = self.inner.position();
        let length = self
            .inner
            .array()
            .map_err(|_| self.type_error(WireType::Array))?
            .ok_or_else(|| {
                self.error(WireErrorKind::IndefiniteLength {
                    expected: WireType::Array,
                })
            })?;

        self.require_canonical_head(start, 4, length)?;
        if length > (self.input.len() - self.inner.position()) as u64 {
            return Err(self.error(WireErrorKind::UnexpectedEnd));
        }
        Ok(length)
    }

    pub fn map(&mut self) -> Result<u64, WireError> {
        let start = self.inner.position();
        let length = self
            .inner
            .map()
            .map_err(|_| self.type_error(WireType::Map))?
            .ok_or_else(|| {
                self.error(WireErrorKind::IndefiniteLength {
                    expected: WireType::Map,
                })
            })?;

        self.require_canonical_head(start, 5, length)?;
        if length > ((self.input.len() - self.inner.position()) / 2) as u64 {
            return Err(self.error(WireErrorKind::UnexpectedEnd));
        }
        Ok(length)
    }

    pub fn expect_map(&mut self, expected: u64) -> Result<(), WireError> {
        let actual = self.map()?;
        if actual == expected {
            Ok(())
        } else {
            Err(self.error(WireErrorKind::InvalidLength { expected, actual }))
        }
    }

    pub fn field<T>(
        &mut self,
        expected: u32,
        decode: impl FnOnce(&mut Self) -> Result<T, WireError>,
    ) -> Result<T, WireError> {
        let actual = self.unsigned()?;
        if actual != u64::from(expected) {
            return Err(self.error(WireErrorKind::UnexpectedField { expected, actual }));
        }
        self.with_nested(PathSegment::Field(expected), decode)
    }

    pub fn index<T>(
        &mut self,
        index: u64,
        decode: impl FnOnce(&mut Self) -> Result<T, WireError>,
    ) -> Result<T, WireError> {
        self.with_nested(PathSegment::Index(index), decode)
    }

    pub fn key<T>(
        &mut self,
        kind: &'static str,
        bytes: [u8; 32],
        decode: impl FnOnce(&mut Self) -> Result<T, WireError>,
    ) -> Result<T, WireError> {
        self.with_nested(PathSegment::Key { kind, bytes }, decode)
    }

    pub fn decode_array<T>(
        &mut self,
        mut decode: impl FnMut(&mut Self, u64) -> Result<T, WireError>,
    ) -> Result<Vec<T>, WireError> {
        let length = self.array()?;
        let mut values = Vec::new();
        crate::allocation::try_reserve_count(&mut values, length, &self.path)?;
        for index in 0..length {
            values.push(self.index(index, |decoder| decode(decoder, index))?);
        }
        Ok(values)
    }

    pub fn finish(&self) -> Result<(), WireError> {
        if self.inner.position() == self.input.len() {
            Ok(())
        } else {
            Err(self.error(WireErrorKind::TrailingData))
        }
    }

    fn with_nested<T>(
        &mut self,
        segment: PathSegment,
        decode: impl FnOnce(&mut Self) -> Result<T, WireError>,
    ) -> Result<T, WireError> {
        self.path.push(segment);
        let result = decode(self);
        self.path.pop();
        result
    }

    fn require_canonical_head(
        &self,
        start: usize,
        major: u8,
        argument: u64,
    ) -> Result<(), WireError> {
        let mut expected = [0u8; 9];
        let expected_length = encode_head(&mut expected, major, argument);
        let end = start
            .checked_add(expected_length)
            .ok_or_else(|| self.error(WireErrorKind::IntegerOutOfRange))?;
        if self.input.get(start..end) == Some(&expected[..expected_length]) {
            Ok(())
        } else {
            Err(self.error_at(WireErrorKind::NonCanonicalCbor, start))
        }
    }

    fn type_error(&self, expected: WireType) -> WireError {
        if self.inner.position() >= self.input.len() {
            self.error(WireErrorKind::UnexpectedEnd)
        } else {
            self.error(WireErrorKind::WrongType { expected })
        }
    }

    fn error(&self, kind: WireErrorKind) -> WireError {
        self.error_at(kind, self.inner.position())
    }

    fn error_at(&self, kind: WireErrorKind, position: usize) -> WireError {
        WireError::new(kind, self.path.clone(), Some(position as u64))
    }
}

pub trait WireDecode: WireEncode + Sized {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError>;
}

/// Strict decoding for documents that retain borrowed carrier byte strings.
/// Every implementation still uses [`Decoder`]'s canonical primitives, so it
/// does not need to copy the complete document for a re-encode check.
pub trait BorrowedWireDecode<'input>: Sized {
    fn decode(decoder: &mut Decoder<'input>) -> Result<Self, WireError>;
}

pub fn decode_canonical<T: WireDecode>(input: &[u8]) -> Result<T, WireError> {
    let mut decoder = Decoder::new(input)?;
    let value = T::decode(&mut decoder)?;
    decoder.finish()?;
    let canonical = encode_with_limit(&value, input.len()).map_err(|error| {
        let kind = match error {
            EncodeError::Allocation => WireErrorKind::Allocation,
            EncodeError::LengthLimit | EncodeError::OutputSinkMismatch => {
                WireErrorKind::NonCanonicalCbor
            }
        };
        WireError::new(kind, WirePath::root(), None)
    })?;
    if canonical != input {
        return Err(WireError::new(
            WireErrorKind::NonCanonicalCbor,
            WirePath::root(),
            None,
        ));
    }
    Ok(value)
}

pub fn decode_canonical_borrowed<'input, T: BorrowedWireDecode<'input>>(
    input: &'input [u8],
) -> Result<T, WireError> {
    let mut decoder = Decoder::new(input)?;
    let value = T::decode(&mut decoder)?;
    decoder.finish()?;
    Ok(value)
}

fn encode_head(output: &mut [u8; 9], major: u8, argument: u64) -> usize {
    let major = major << 5;
    match argument {
        0..=23 => {
            output[0] = major | argument as u8;
            1
        }
        24..=0xff => {
            output[0] = major | 24;
            output[1] = argument as u8;
            2
        }
        0x100..=0xffff => {
            output[0] = major | 25;
            output[1..3].copy_from_slice(&(argument as u16).to_be_bytes());
            3
        }
        0x1_0000..=0xffff_ffff => {
            output[0] = major | 26;
            output[1..5].copy_from_slice(&(argument as u32).to_be_bytes());
            5
        }
        _ => {
            output[0] = major | 27;
            output[1..9].copy_from_slice(&argument.to_be_bytes());
            9
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Encoder, WireErrorKind, encode};

    use super::{
        BorrowedWireDecode, Decoder, EncodeError, WireDecode, WireEncode, decode_canonical,
        decode_canonical_borrowed,
    };

    #[derive(Debug, Eq, PartialEq)]
    struct Pair {
        first: u64,
        second: String,
    }

    #[derive(Debug, Eq, PartialEq)]
    struct UnsignedValue(u64);

    #[derive(Debug, Eq, PartialEq)]
    struct Carrier(Vec<u8>);

    #[derive(Debug, Eq, PartialEq)]
    struct BorrowedCarrier<'input>(&'input [u8]);

    impl WireEncode for UnsignedValue {
        fn encode(&self, encoder: &mut Encoder) -> Result<(), EncodeError> {
            encoder.unsigned(self.0)
        }
    }

    impl WireDecode for UnsignedValue {
        fn decode(decoder: &mut Decoder<'_>) -> Result<Self, crate::WireError> {
            Ok(Self(decoder.unsigned()?))
        }
    }

    impl WireEncode for Carrier {
        fn encode(&self, encoder: &mut Encoder) -> Result<(), EncodeError> {
            encoder.bytes(&self.0)
        }
    }

    impl WireDecode for Carrier {
        fn decode(decoder: &mut Decoder<'_>) -> Result<Self, crate::WireError> {
            decoder.owned_bytes().map(Self)
        }
    }

    impl<'input> BorrowedWireDecode<'input> for BorrowedCarrier<'input> {
        fn decode(decoder: &mut Decoder<'input>) -> Result<Self, crate::WireError> {
            decoder.bytes().map(Self)
        }
    }

    impl WireEncode for Pair {
        fn encode(&self, encoder: &mut Encoder) -> Result<(), EncodeError> {
            encoder.map(2)?;
            encoder.field(1)?;
            encoder.unsigned(self.first)?;
            encoder.field(24)?;
            encoder.text(&self.second)
        }
    }

    impl WireDecode for Pair {
        fn decode(decoder: &mut Decoder<'_>) -> Result<Self, crate::WireError> {
            decoder.expect_map(2)?;
            let first = decoder.field(1, Decoder::unsigned)?;
            let second = decoder.field(24, Decoder::owned_text)?;
            Ok(Self { first, second })
        }
    }

    #[test]
    fn canonical_product_has_fixed_bytes() {
        let pair = Pair {
            first: 23,
            second: "é".to_string(),
        };
        assert_eq!(encode(&pair).unwrap(), b"\xa2\x01\x17\x18\x18\x62\xc3\xa9");
        assert_eq!(
            decode_canonical::<Pair>(&encode(&pair).unwrap()).unwrap(),
            pair
        );
    }

    #[test]
    fn unsigned_width_boundaries_use_shortest_encoding() {
        let vectors: &[(u64, &[u8])] = &[
            (23, b"\x17"),
            (24, b"\x18\x18"),
            (255, b"\x18\xff"),
            (256, b"\x19\x01\x00"),
            (65_535, b"\x19\xff\xff"),
            (65_536, b"\x1a\x00\x01\x00\x00"),
            (u64::from(u32::MAX), b"\x1a\xff\xff\xff\xff"),
            (
                u64::from(u32::MAX) + 1,
                b"\x1b\x00\x00\x00\x01\x00\x00\x00\x00",
            ),
            (u64::MAX, b"\x1b\xff\xff\xff\xff\xff\xff\xff\xff"),
        ];

        for (value, expected) in vectors {
            let encoded = encode(&UnsignedValue(*value)).unwrap();
            assert_eq!(&encoded, expected);
            assert_eq!(
                decode_canonical::<UnsignedValue>(&encoded).unwrap(),
                UnsignedValue(*value)
            );
        }
    }

    #[test]
    fn non_minimal_integer_is_rejected() {
        let bytes = b"\xa2\x01\x18\x17\x18\x18\x61x";
        let error = decode_canonical::<Pair>(bytes).unwrap_err();
        assert_eq!(error.kind(), &WireErrorKind::NonCanonicalCbor);
        assert_eq!(error.path().to_string(), "$.1");
    }

    #[test]
    fn wrong_field_order_is_rejected_at_stable_path() {
        let bytes = b"\xa2\x18\x18\x61x\x01\x17";
        let error = decode_canonical::<Pair>(bytes).unwrap_err();
        assert_eq!(
            error.kind(),
            &WireErrorKind::UnexpectedField {
                expected: 1,
                actual: 24,
            }
        );
        assert_eq!(error.path().to_string(), "$");
    }

    #[test]
    fn indefinite_map_is_rejected() {
        let bytes = b"\xbf\x01\x17\x18\x18\x61x\xff";
        let error = decode_canonical::<Pair>(bytes).unwrap_err();
        assert!(matches!(
            error.kind(),
            WireErrorKind::IndefiniteLength { .. }
        ));
    }

    #[test]
    fn protocol_forbidden_scalar_is_rejected() {
        for bytes in [
            b"\xa2\x01\x20\x18\x18\x61x".as_slice(),
            b"\xa2\x01\xf4\x18\x18\x61x".as_slice(),
            b"\xa2\x01\xf6\x18\x18\x61x".as_slice(),
            b"\xa2\x01\xc0\x00\x18\x18\x61x".as_slice(),
        ] {
            let error = decode_canonical::<Pair>(bytes).unwrap_err();
            assert!(matches!(error.kind(), WireErrorKind::WrongType { .. }));
        }
    }

    #[test]
    fn indefinite_text_is_reported_explicitly() {
        let bytes = b"\xa2\x01\x17\x18\x18\x7f\x61x\xff";
        let error = decode_canonical::<Pair>(bytes).unwrap_err();
        assert!(matches!(
            error.kind(),
            WireErrorKind::IndefiniteLength {
                expected: crate::WireType::Text
            }
        ));
        assert_eq!(error.path().to_string(), "$.24");
    }

    #[test]
    fn trailing_data_is_rejected() {
        let bytes = b"\xa2\x01\x17\x18\x18\x61x\x00";
        let error = decode_canonical::<Pair>(bytes).unwrap_err();
        assert_eq!(error.kind(), &WireErrorKind::TrailingData);
    }

    #[test]
    fn declared_collection_lengths_must_fit_the_input() {
        for bytes in [
            &[0x9b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff][..],
            &[0x83, 0x00, 0x00],
        ] {
            let mut decoder = Decoder::new(bytes).unwrap();
            assert_eq!(
                decoder
                    .decode_array(|decoder, _| decoder.unsigned())
                    .unwrap_err()
                    .kind(),
                &WireErrorKind::UnexpectedEnd
            );
        }
        for bytes in [
            &[0xbb, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff][..],
            &[0xa2, 0x00, 0x00, 0x01],
        ] {
            let mut decoder = Decoder::new(bytes).unwrap();
            assert_eq!(
                decoder.map().unwrap_err().kind(),
                &WireErrorKind::UnexpectedEnd
            );
        }
    }

    #[test]
    fn borrowed_document_keeps_bytes_zero_copy() {
        let encoded = encode(&Carrier(vec![1, 2, 3, 4])).unwrap();

        assert_eq!(
            decode_canonical_borrowed::<BorrowedCarrier<'_>>(&encoded),
            Ok(BorrowedCarrier(&encoded[1..]))
        );

        let non_minimal = [0x58, 0x01, 0x01];
        assert_eq!(
            decode_canonical_borrowed::<BorrowedCarrier<'_>>(&non_minimal,)
                .unwrap_err()
                .kind(),
            &WireErrorKind::NonCanonicalCbor
        );
    }
}
