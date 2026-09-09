use std::fmt;

use sha2::{Digest, Sha256};

use crate::cbor::{WireEncode, encode};

/// A content digest. Semantic identities use distinct newtypes in
/// `scoop-identity` and cannot be converted from this type through safe APIs.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Digest256([u8; 32]);

impl Digest256 {
    pub const fn from_array(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for Digest256 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HashError {
    LengthOverflow,
    InvalidDomain,
    CborEncoding,
}

impl fmt::Display for HashError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::LengthOverflow => "byte span length does not fit u64",
            Self::InvalidDomain => "hash domain must be non-NUL ASCII",
            Self::CborEncoding => "canonical CBOR encoding failed",
        })
    }
}

impl std::error::Error for HashError {}

pub fn sha256(bytes: &[u8]) -> Digest256 {
    Digest256(Sha256::digest(bytes).into())
}

pub fn byte_span(bytes: &[u8]) -> Result<Vec<u8>, HashError> {
    let length = u64::try_from(bytes.len()).map_err(|_| HashError::LengthOverflow)?;
    let mut framed = Vec::new();
    let capacity = 8usize
        .checked_add(bytes.len())
        .ok_or(HashError::LengthOverflow)?;
    framed
        .try_reserve_exact(capacity)
        .map_err(|_| HashError::LengthOverflow)?;
    framed.extend_from_slice(&length.to_le_bytes());
    framed.extend_from_slice(bytes);
    Ok(framed)
}

pub fn domain_separated_cbor_hash(
    domain: &str,
    value: &impl WireEncode,
) -> Result<Digest256, HashError> {
    if !domain.is_ascii() || domain.as_bytes().contains(&0) {
        return Err(HashError::InvalidDomain);
    }
    let encoded = encode(value).map_err(|_| HashError::CborEncoding)?;
    let length = u64::try_from(domain.len()).map_err(|_| HashError::LengthOverflow)?;
    let mut hasher = Sha256::new();
    hasher.update(length.to_le_bytes());
    hasher.update(domain.as_bytes());
    hasher.update(encoded);
    Ok(Digest256(hasher.finalize().into()))
}

#[cfg(test)]
mod tests {
    use crate::cbor::{Encoder, WireEncode};

    use super::{byte_span, domain_separated_cbor_hash, sha256};

    struct One;

    impl WireEncode for One {
        fn encode(&self, encoder: &mut Encoder) -> Result<(), crate::cbor::EncodeError> {
            encoder.unsigned(1)
        }
    }

    #[test]
    fn sha256_matches_standard_vector() {
        assert_eq!(
            sha256(b"abc").to_string(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn byte_span_uses_little_endian_u64_length() {
        assert_eq!(byte_span(b"abc").unwrap(), b"\x03\0\0\0\0\0\0\0abc");
    }

    #[test]
    fn domain_separated_hash_has_a_fixed_vector() {
        assert_eq!(
            domain_separated_cbor_hash("scoop-wire-test-v1", &One)
                .unwrap()
                .to_string(),
            "9a7155a4014f2071f951555acd3e72e44dc4b3f1107033e52c56011fd886ebcd"
        );
    }
}
