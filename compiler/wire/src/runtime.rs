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

#[cfg(test)]
mod tests {
    use super::{RuntimeEncode, RuntimeEncoder, encode_runtime};

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
}
