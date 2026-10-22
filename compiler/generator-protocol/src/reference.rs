use scoop_identity::{DecodedPersistentId, PersistentId};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{fmt, marker::PhantomData, str::FromStr};

/// A reference of one particular identity kind, resolved by ordinary lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Reference<I: PersistentId> {
    bytes: [u8; 32],
    marker: PhantomData<I>,
}

impl<I: PersistentId> Reference<I> {
    pub const fn as_array(&self) -> &[u8; 32] {
        &self.bytes
    }
}

impl<I: PersistentId> From<I> for Reference<I> {
    fn from(id: I) -> Self {
        Self {
            bytes: *id.as_array(),
            marker: PhantomData,
        }
    }
}

impl<I: PersistentId> From<Reference<I>> for DecodedPersistentId<I> {
    fn from(reference: Reference<I>) -> Self {
        Self::from_bytes(reference.bytes)
    }
}

impl<I: PersistentId> FromStr for Reference<I> {
    type Err = ReferenceFormatError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.len() != 64 {
            return Err(ReferenceFormatError);
        }
        let mut bytes = [0; 32];
        for (output, pair) in bytes.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
            *output = (digit(pair[0])? << 4) | digit(pair[1])?;
        }
        Ok(Self {
            bytes,
            marker: PhantomData,
        })
    }
}

fn digit(value: u8) -> Result<u8, ReferenceFormatError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(ReferenceFormatError),
    }
}

impl<I: PersistentId> fmt::Display for Reference<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.bytes {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl<I: PersistentId> Serialize for Reference<I> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de, I: PersistentId> Deserialize<'de> for Reference<I> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReferenceFormatError;

impl fmt::Display for ReferenceFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("identity reference must contain exactly 64 hexadecimal digits")
    }
}

impl std::error::Error for ReferenceFormatError {}
