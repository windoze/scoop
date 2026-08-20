use std::fmt;

/// Canonical encoder for runtime metadata records.
///
/// Scalars are little-endian, products are written in declaration order, and
/// sums start with an explicit little-endian `u32` tag.
#[derive(Default)]
pub struct RuntimeEncoder {
    bytes: Vec<u8>,
}

impl RuntimeEncoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn u32(&mut self, value: u32) -> Result<(), RuntimeEncodeError> {
        self.write(&value.to_le_bytes())
    }

    pub fn u64(&mut self, value: u64) -> Result<(), RuntimeEncodeError> {
        self.write(&value.to_le_bytes())
    }

    pub fn fixed(&mut self, value: &[u8]) -> Result<(), RuntimeEncodeError> {
        self.write(value)
    }

    pub fn byte_span(&mut self, value: &[u8]) -> Result<(), RuntimeEncodeError> {
        let length = u64::try_from(value.len()).map_err(|_| RuntimeEncodeError::LengthOverflow)?;
        self.u64(length)?;
        self.write(value)
    }

    pub fn sequence_length(&mut self, length: usize) -> Result<(), RuntimeEncodeError> {
        let length = u64::try_from(length).map_err(|_| RuntimeEncodeError::LengthOverflow)?;
        self.u64(length)
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    fn write(&mut self, value: &[u8]) -> Result<(), RuntimeEncodeError> {
        self.bytes
            .try_reserve_exact(value.len())
            .map_err(|_| RuntimeEncodeError::Allocation)?;
        self.bytes.extend_from_slice(value);
        Ok(())
    }
}

pub trait RuntimeEncode {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError>;
}

pub fn encode_runtime(value: &impl RuntimeEncode) -> Result<Vec<u8>, RuntimeEncodeError> {
    let mut encoder = RuntimeEncoder::new();
    value.runtime_encode(&mut encoder)?;
    Ok(encoder.into_bytes())
}

pub struct RuntimeDecoder<'input> {
    input: &'input [u8],
    position: usize,
}

impl<'input> RuntimeDecoder<'input> {
    pub const fn new(input: &'input [u8]) -> Self {
        Self { input, position: 0 }
    }

    pub fn position(&self) -> u64 {
        self.position as u64
    }

    pub fn u32(&mut self) -> Result<u32, RuntimeDecodeError> {
        let bytes = self.fixed(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn u64(&mut self) -> Result<u64, RuntimeDecodeError> {
        let bytes = self.fixed(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    pub fn fixed(&mut self, length: usize) -> Result<&'input [u8], RuntimeDecodeError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or_else(|| self.error(RuntimeDecodeErrorKind::LengthOutOfRange))?;
        let value = self.input.get(self.position..end).ok_or_else(|| {
            self.error(RuntimeDecodeErrorKind::UnexpectedEnd {
                needed: length as u64,
                remaining: self.input.len().saturating_sub(self.position) as u64,
            })
        })?;
        self.position = end;
        Ok(value)
    }

    pub fn byte_span(&mut self) -> Result<&'input [u8], RuntimeDecodeError> {
        let length = self.u64()?;
        let length = usize::try_from(length)
            .map_err(|_| self.error(RuntimeDecodeErrorKind::LengthOutOfRange))?;
        self.fixed(length)
    }

    pub fn sequence_length(&mut self) -> Result<usize, RuntimeDecodeError> {
        let length = self.u64()?;
        usize::try_from(length).map_err(|_| self.error(RuntimeDecodeErrorKind::LengthOutOfRange))
    }

    pub fn finish(&self) -> Result<(), RuntimeDecodeError> {
        if self.position == self.input.len() {
            Ok(())
        } else {
            Err(self.error(RuntimeDecodeErrorKind::TrailingData {
                remaining: self.input.len().saturating_sub(self.position) as u64,
            }))
        }
    }

    pub fn error(&self, kind: RuntimeDecodeErrorKind) -> RuntimeDecodeError {
        RuntimeDecodeError {
            kind,
            byte_offset: self.position(),
        }
    }
}

pub trait RuntimeDecode: Sized {
    fn runtime_decode(decoder: &mut RuntimeDecoder<'_>) -> Result<Self, RuntimeDecodeError>;
}

pub fn decode_runtime<T: RuntimeDecode>(input: &[u8]) -> Result<T, RuntimeDecodeError> {
    let mut decoder = RuntimeDecoder::new(input);
    let value = T::runtime_decode(&mut decoder)?;
    decoder.finish()?;
    Ok(value)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeEncodeError {
    Allocation,
    LengthOverflow,
}

impl fmt::Display for RuntimeEncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Allocation => "runtime metadata encoding allocation failed",
            Self::LengthOverflow => "runtime metadata length does not fit u64",
        })
    }
}

impl std::error::Error for RuntimeEncodeError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeDecodeErrorKind {
    UnexpectedEnd { needed: u64, remaining: u64 },
    TrailingData { remaining: u64 },
    LengthOutOfRange,
    UnknownTag { tag: u32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeDecodeError {
    kind: RuntimeDecodeErrorKind,
    byte_offset: u64,
}

impl RuntimeDecodeError {
    pub const fn kind(&self) -> RuntimeDecodeErrorKind {
        self.kind
    }

    pub const fn byte_offset(&self) -> u64 {
        self.byte_offset
    }
}

impl fmt::Display for RuntimeDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "runtime metadata decode error at byte {}: ",
            self.byte_offset
        )?;
        match self.kind {
            RuntimeDecodeErrorKind::UnexpectedEnd { needed, remaining } => write!(
                formatter,
                "unexpected end of input: need {needed} bytes, have {remaining}"
            ),
            RuntimeDecodeErrorKind::TrailingData { remaining } => {
                write!(formatter, "{remaining} trailing bytes")
            }
            RuntimeDecodeErrorKind::LengthOutOfRange => {
                formatter.write_str("length does not fit the host index type")
            }
            RuntimeDecodeErrorKind::UnknownTag { tag } => write!(formatter, "unknown tag {tag}"),
        }
    }
}

impl std::error::Error for RuntimeDecodeError {}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeDecode, RuntimeDecodeError, RuntimeDecodeErrorKind, RuntimeDecoder, RuntimeEncode,
        RuntimeEncoder, decode_runtime, encode_runtime,
    };

    struct Sample<'a> {
        tag: u32,
        payload: &'a [u8],
        values: &'a [u32],
    }

    impl RuntimeEncode for Sample<'_> {
        fn runtime_encode(
            &self,
            encoder: &mut RuntimeEncoder,
        ) -> Result<(), super::RuntimeEncodeError> {
            encoder.u32(self.tag)?;
            encoder.byte_span(self.payload)?;
            encoder.sequence_length(self.values.len())?;
            for value in self.values {
                encoder.u32(*value)?;
            }
            Ok(())
        }
    }

    #[derive(Debug, Eq, PartialEq)]
    struct DecodedSample {
        tag: u32,
        payload: Vec<u8>,
        values: Vec<u32>,
    }

    impl RuntimeDecode for DecodedSample {
        fn runtime_decode(decoder: &mut RuntimeDecoder<'_>) -> Result<Self, RuntimeDecodeError> {
            let tag = decoder.u32()?;
            let payload = decoder.byte_span()?.to_vec();
            let length = decoder.sequence_length()?;
            let mut values = Vec::with_capacity(length);
            for _ in 0..length {
                values.push(decoder.u32()?);
            }
            Ok(Self {
                tag,
                payload,
                values,
            })
        }
    }

    #[test]
    fn runtime_metadata_encoding_is_little_endian_and_explicitly_framed() {
        let encoded = encode_runtime(&Sample {
            tag: 0x0102_0304,
            payload: b"ab",
            values: &[5, 6],
        })
        .unwrap();

        assert_eq!(
            encoded,
            [
                b"\x04\x03\x02\x01".as_slice(),
                b"\x02\0\0\0\0\0\0\0ab",
                b"\x02\0\0\0\0\0\0\0",
                b"\x05\0\0\0\x06\0\0\0",
            ]
            .concat()
        );
    }

    #[test]
    fn runtime_metadata_decoder_consumes_the_same_framing() {
        let encoded = encode_runtime(&Sample {
            tag: 0x0102_0304,
            payload: b"ab",
            values: &[5, 6],
        })
        .unwrap();

        assert_eq!(
            decode_runtime::<DecodedSample>(&encoded).unwrap(),
            DecodedSample {
                tag: 0x0102_0304,
                payload: b"ab".to_vec(),
                values: vec![5, 6],
            }
        );
    }

    #[test]
    fn runtime_metadata_decoder_rejects_truncation_and_trailing_data() {
        let truncated = decode_runtime::<DecodedSample>(b"\x01\0\0").unwrap_err();
        assert_eq!(
            truncated.kind(),
            RuntimeDecodeErrorKind::UnexpectedEnd {
                needed: 4,
                remaining: 3,
            }
        );
        assert_eq!(truncated.byte_offset(), 0);

        #[derive(Debug)]
        struct OneU32;
        impl RuntimeDecode for OneU32 {
            fn runtime_decode(
                decoder: &mut RuntimeDecoder<'_>,
            ) -> Result<Self, RuntimeDecodeError> {
                decoder.u32()?;
                Ok(Self)
            }
        }
        let trailing = decode_runtime::<OneU32>(b"\x01\0\0\0x").unwrap_err();
        assert_eq!(
            trailing.kind(),
            RuntimeDecodeErrorKind::TrailingData { remaining: 1 }
        );
        assert_eq!(trailing.byte_offset(), 4);
    }
}
