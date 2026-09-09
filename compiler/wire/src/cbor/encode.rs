use std::fmt;

/// A trusted producer for the Wire CBOR v1 subset.
///
/// Only definite arrays/maps, unsigned integers, byte strings, and UTF-8 text
/// are exposed. This makes forbidden CBOR types unrepresentable at call sites.
pub struct Encoder {
    inner: minicbor::Encoder<FallibleVec>,
}

impl Encoder {
    pub fn new() -> Self {
        Self::with_max_length(None)
    }

    pub(crate) fn with_max_length(max_length: Option<usize>) -> Self {
        Self {
            inner: minicbor::Encoder::new(FallibleVec {
                bytes: Vec::new(),
                max_length,
            }),
        }
    }

    pub fn unsigned(&mut self, value: u64) -> Result<(), EncodeError> {
        self.inner.u64(value).map_err(EncodeError::from)?;
        Ok(())
    }

    pub fn bytes(&mut self, value: &[u8]) -> Result<(), EncodeError> {
        self.inner.bytes(value).map_err(EncodeError::from)?;
        Ok(())
    }

    pub fn text(&mut self, value: &str) -> Result<(), EncodeError> {
        self.inner.str(value).map_err(EncodeError::from)?;
        Ok(())
    }

    pub fn array(&mut self, length: u64) -> Result<(), EncodeError> {
        self.inner.array(length).map_err(EncodeError::from)?;
        Ok(())
    }

    pub fn map(&mut self, length: u64) -> Result<(), EncodeError> {
        self.inner.map(length).map_err(EncodeError::from)?;
        Ok(())
    }

    pub fn field(&mut self, field: u32) -> Result<(), EncodeError> {
        self.unsigned(u64::from(field))
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.inner.into_writer().bytes
    }
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}

pub trait WireEncode {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), EncodeError>;
}

pub fn encode(value: &impl WireEncode) -> Result<Vec<u8>, EncodeError> {
    let mut encoder = Encoder::new();
    value.encode(&mut encoder)?;
    Ok(encoder.into_bytes())
}

pub(crate) fn encode_with_limit(
    value: &impl WireEncode,
    max_length: usize,
) -> Result<Vec<u8>, EncodeError> {
    let mut encoder = Encoder::with_max_length(Some(max_length));
    value.encode(&mut encoder)?;
    Ok(encoder.into_bytes())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EncodeError {
    Allocation,
    LengthLimit,
}

impl From<minicbor::encode::Error<OutputError>> for EncodeError {
    fn from(error: minicbor::encode::Error<OutputError>) -> Self {
        match error.into_write() {
            Some(OutputError::Allocation) => Self::Allocation,
            Some(OutputError::LengthLimit) | None => Self::LengthLimit,
        }
    }
}

impl fmt::Display for EncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Allocation => "Wire CBOR v1 output allocation failed",
            Self::LengthLimit => "Wire CBOR v1 output exceeded its length limit",
        })
    }
}

impl std::error::Error for EncodeError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutputError {
    Allocation,
    LengthLimit,
}

struct FallibleVec {
    bytes: Vec<u8>,
    max_length: Option<usize>,
}

impl minicbor::encode::Write for FallibleVec {
    type Error = OutputError;

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        let new_length = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or(OutputError::LengthLimit)?;
        if self.max_length.is_some_and(|limit| new_length > limit) {
            return Err(OutputError::LengthLimit);
        }
        self.bytes
            .try_reserve_exact(bytes.len())
            .map_err(|_| OutputError::Allocation)?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
}
