//! Domain-separated SHA-256 digests and the shared 256-bit digest type.

use core::fmt;
use sha2::{Digest as _, Sha256};

/// A 256-bit digest. Ordering is the byte order of the digest, which is
/// the canonical ordering used whenever tables are keyed by digest.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Digest256([u8; 32]);

impl Digest256 {
    /// The all-zero digest; used for elided/self-referential slots that
    /// wire schemas deliberately leave unset.
    pub const ZERO: Digest256 = Digest256([0u8; 32]);

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Digest256(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Parses 64 lowercase hex characters. Test/vector helper; identities
    /// are always computed, never spelled by hand in production code.
    pub fn from_hex(hex: &str) -> Result<Self, HexError> {
        let bytes = hex.as_bytes();
        if bytes.len() != 64 {
            return Err(HexError::Length(bytes.len()));
        }
        let mut out = [0u8; 32];
        for (index, slot) in out.iter_mut().enumerate() {
            *slot = (nibble(bytes[2 * index])? << 4) | nibble(bytes[2 * index + 1])?;
        }
        Ok(Digest256(out))
    }

    pub fn to_hex(self) -> String {
        let mut out = String::with_capacity(64);
        for byte in self.0 {
            out.push(char::from_digit((byte >> 4) as u32, 16).expect("hex digit"));
            out.push(char::from_digit((byte & 0xF) as u32, 16).expect("hex digit"));
        }
        out
    }
}

fn nibble(byte: u8) -> Result<u8, HexError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(HexError::InvalidByte(byte)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HexError {
    Length(usize),
    InvalidByte(u8),
}

impl fmt::Display for HexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HexError::Length(len) => write!(f, "expected 64 hex characters, found {len}"),
            HexError::InvalidByte(byte) => write!(f, "invalid lowercase hex byte 0x{byte:02x}"),
        }
    }
}

impl std::error::Error for HexError {}

impl fmt::Debug for Digest256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Digest256({})", self.to_hex())
    }
}

impl fmt::Display for Digest256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// Domain-separated hasher: the domain tag is written verbatim, then every
/// field is written with a `u64` little-endian length prefix, so no two
/// distinct field sequences can produce the same byte stream (DESIGN
/// section 1.1: fixed field order and length prefixes, never separator
/// concatenation).
///
/// Fixed-width integers are fields of their little-endian bytes, and
/// canonical CBOR payloads are ordinary byte fields. Schemas that fix an
/// explicit byte-level encoder (for example the runtime metadata records
/// shared with the C runtime) use their own encoder instead of this
/// generic builder.
pub struct DomainHasher {
    hasher: Sha256,
}

impl DomainHasher {
    pub fn new(domain: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(domain);
        DomainHasher { hasher }
    }

    pub fn field(mut self, bytes: &[u8]) -> Self {
        self.hasher.update((bytes.len() as u64).to_le_bytes());
        self.hasher.update(bytes);
        self
    }

    pub fn u32_field(self, value: u32) -> Self {
        self.field(&value.to_le_bytes())
    }

    pub fn u64_field(self, value: u64) -> Self {
        self.field(&value.to_le_bytes())
    }

    pub fn finish(self) -> Digest256 {
        let mut out = [0u8; 32];
        out.copy_from_slice(&self.hasher.finalize());
        Digest256::from_bytes(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        let digest = DomainHasher::new(b"domain").field(b"value").finish();
        let hex = digest.to_hex();
        assert_eq!(Digest256::from_hex(&hex), Ok(digest));
        assert!(Digest256::from_hex("00").is_err());
        assert!(Digest256::from_hex(&hex.to_uppercase()).is_err());
    }

    #[test]
    fn fields_are_length_prefixed() {
        // ("ab", "c") and ("a", "bc") must not collide.
        let joined = DomainHasher::new(b"d").field(b"ab").field(b"c").finish();
        let split = DomainHasher::new(b"d").field(b"a").field(b"bc").finish();
        assert_ne!(joined, split);
        // Domain tags separate inputs.
        let other = DomainHasher::new(b"e").field(b"ab").field(b"c").finish();
        assert_ne!(joined, other);
    }
}
