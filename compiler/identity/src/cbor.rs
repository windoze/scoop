//! Deterministic canonical CBOR (RFC 8949) reader and writer.
//!
//! The `.slib` manifest and every wire schema use one canonical profile
//! (`docs/milestone23/DESIGN.md` section 4.1): unsigned/negative integers
//! in shortest form, definite lengths only, byte and text strings, arrays
//! and integer-keyed maps whose keys strictly ascend. Floats, simple
//! values, tags and indefinite items never appear on the wire and are
//! rejected while decoding.
//!
//! Records follow one shape: a map whose keys are the declared field
//! numbers in ascending order, where a sum's variant tag occupies key `0`
//! and payload fields use the remaining declared keys.

use core::fmt;

pub type Result<T> = core::result::Result<T, CborError>;

/// A canonical-CBOR violation, truncation, or a schema mismatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CborError {
    UnexpectedEof,
    TrailingBytes(usize),
    MajorMismatch {
        expected: u8,
        actual: u8,
    },
    UnsupportedMajorType(u8),
    UnsupportedAdditionalInfo {
        major: u8,
        info: u8,
    },
    IndefiniteLength(u8),
    NonCanonicalInteger {
        value: u64,
        info: u8,
    },
    InvalidUtf8,
    /// A declared length claims more items/bytes than the input can hold.
    LengthExceedsInput {
        claimed: u64,
        remaining: usize,
    },
    MapKeyOutOfOrder {
        previous: u64,
        current: u64,
    },
    /// The schema declares exactly `expected` entries but the map header
    /// declared `actual`.
    WrongEntryCount {
        expected: u64,
        actual: u64,
    },
    UnexpectedKey(u64),
    MissingKey(u64),
    NestingLimitExceeded(u32),
}

impl fmt::Display for CborError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CborError::UnexpectedEof => write!(f, "unexpected end of CBOR input"),
            CborError::TrailingBytes(count) => {
                write!(f, "{count} trailing byte(s) after CBOR value")
            }
            CborError::MajorMismatch { expected, actual } => write!(
                f,
                "wrong CBOR major type: expected {expected}, found {actual}"
            ),
            CborError::UnsupportedMajorType(major) => {
                write!(f, "unsupported CBOR major type {major} (float/simple/tag)")
            }
            CborError::UnsupportedAdditionalInfo { major, info } => write!(
                f,
                "reserved CBOR additional info {info} on major type {major}"
            ),
            CborError::IndefiniteLength(major) => {
                write!(f, "indefinite length on CBOR major type {major}")
            }
            CborError::NonCanonicalInteger { value, info } => write!(
                f,
                "non-canonical CBOR integer {value} encoded with additional info {info}"
            ),
            CborError::InvalidUtf8 => write!(f, "CBOR text string is not valid UTF-8"),
            CborError::LengthExceedsInput { claimed, remaining } => write!(
                f,
                "declared length {claimed} exceeds the {remaining} remaining input byte(s)"
            ),
            CborError::MapKeyOutOfOrder { previous, current } => write!(
                f,
                "non-canonical map key order: {current} follows {previous}"
            ),
            CborError::WrongEntryCount { expected, actual } => write!(
                f,
                "record declares {actual} entries, schema requires {expected}"
            ),
            CborError::UnexpectedKey(key) => write!(f, "unexpected record field key {key}"),
            CborError::MissingKey(key) => write!(f, "missing required record field key {key}"),
            CborError::NestingLimitExceeded(limit) => {
                write!(f, "CBOR nesting exceeds the limit of {limit}")
            }
        }
    }
}

impl std::error::Error for CborError {}

/// Canonical CBOR encoder. Every emitted item uses the shortest definite
/// encoding; the caller is responsible for writing map keys in strictly
/// ascending order (the typed encoders in this workspace always do).
#[derive(Default, Clone)]
pub struct CborWriter {
    bytes: Vec<u8>,
}

impl CborWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    fn head(&mut self, major: u8, value: u64) {
        debug_assert!(major <= 7);
        let base = major << 5;
        match value {
            0..=23 => self.bytes.push(base | value as u8),
            24..=0xFF => {
                self.bytes.push(base | 24);
                self.bytes.push(value as u8);
            }
            0x100..=0xFFFF => {
                self.bytes.push(base | 25);
                self.bytes.extend_from_slice(&(value as u16).to_be_bytes());
            }
            0x1_0000..=0xFFFF_FFFF => {
                self.bytes.push(base | 26);
                self.bytes.extend_from_slice(&(value as u32).to_be_bytes());
            }
            _ => {
                self.bytes.push(base | 27);
                self.bytes.extend_from_slice(&value.to_be_bytes());
            }
        }
    }

    pub fn unsigned(&mut self, value: u64) -> &mut Self {
        self.head(0, value);
        self
    }

    /// Encodes a negative integer (`value < 0`) as major type 1.
    pub fn negative(&mut self, value: i64) -> &mut Self {
        debug_assert!(value < 0);
        let argument = (-1_i128 - value as i128) as u64;
        self.head(1, argument);
        self
    }

    pub fn integer(&mut self, value: i64) -> &mut Self {
        if value >= 0 {
            self.unsigned(value as u64);
        } else {
            self.negative(value);
        }
        self
    }

    pub fn bytes(&mut self, data: &[u8]) -> &mut Self {
        self.head(2, data.len() as u64);
        self.bytes.extend_from_slice(data);
        self
    }

    pub fn text(&mut self, value: &str) -> &mut Self {
        self.head(3, value.len() as u64);
        self.bytes.extend_from_slice(value.as_bytes());
        self
    }

    /// Begins an array of `len` items.
    pub fn array(&mut self, len: u64) -> &mut Self {
        self.head(4, len);
        self
    }

    /// Begins a map of `len` key/value pairs. Keys must be written in
    /// strictly ascending unsigned order.
    pub fn map(&mut self, len: u64) -> &mut Self {
        self.head(5, len);
        self
    }

    /// Writes a map key; sugar for [`Self::unsigned`].
    pub fn field(&mut self, key: u64) -> &mut Self {
        self.unsigned(key)
    }
}

/// Canonical CBOR decoder over a borrowed byte slice.
pub struct CborReader<'a> {
    data: &'a [u8],
    position: usize,
    depth: u32,
    max_depth: u32,
}

impl<'a> CborReader<'a> {
    /// Creates a reader with an explicit nesting budget. The budget is
    /// enforced by [`Self::array`]/[`Self::map`] guards and only bounds
    /// structural nesting; recursive wire schemas thread the same budget
    /// through their own decoding.
    pub fn new(data: &'a [u8], max_depth: u32) -> Self {
        Self {
            data,
            position: 0,
            depth: 0,
            max_depth,
        }
    }

    pub fn remaining(&self) -> usize {
        self.data.len() - self.position
    }

    pub fn is_exhausted(&self) -> bool {
        self.position == self.data.len()
    }

    /// Succeeds only when every input byte was consumed.
    pub fn finish(&mut self) -> Result<()> {
        if self.is_exhausted() {
            Ok(())
        } else {
            Err(CborError::TrailingBytes(self.remaining()))
        }
    }

    fn read_byte(&mut self) -> Result<u8> {
        let byte = *self
            .data
            .get(self.position)
            .ok_or(CborError::UnexpectedEof)?;
        self.position += 1;
        Ok(byte)
    }

    fn read_head(&mut self) -> Result<(u8, u64)> {
        let byte = self.read_byte()?;
        let major = byte >> 5;
        let info = byte & 0x1F;
        if major >= 6 {
            return Err(CborError::UnsupportedMajorType(major));
        }
        let argument = match info {
            0..=23 => info as u64,
            24 => {
                let value = self.read_byte()? as u64;
                if value < 24 {
                    return Err(CborError::NonCanonicalInteger { value, info });
                }
                value
            }
            25 => {
                let raw = [self.read_byte()?, self.read_byte()?];
                let value = u16::from_be_bytes(raw) as u64;
                if value < 0x100 {
                    return Err(CborError::NonCanonicalInteger { value, info });
                }
                value
            }
            26 => {
                let mut raw = [0u8; 4];
                for slot in &mut raw {
                    *slot = self.read_byte()?;
                }
                let value = u32::from_be_bytes(raw) as u64;
                if value < 0x1_0000 {
                    return Err(CborError::NonCanonicalInteger { value, info });
                }
                value
            }
            27 => {
                let mut raw = [0u8; 8];
                for slot in &mut raw {
                    *slot = self.read_byte()?;
                }
                let value = u64::from_be_bytes(raw);
                if value < 0x1_0000_0000 {
                    return Err(CborError::NonCanonicalInteger { value, info });
                }
                value
            }
            28..=30 => {
                return Err(CborError::UnsupportedAdditionalInfo { major, info });
            }
            31 => return Err(CborError::IndefiniteLength(major)),
            // `info` is masked to five bits, so 32..=u8::MAX cannot occur.
            _ => unreachable!("CBOR additional info is masked to 5 bits"),
        };
        Ok((major, argument))
    }

    pub fn unsigned(&mut self) -> Result<u64> {
        let (major, value) = self.read_head()?;
        if major != 0 {
            return Err(CborError::MajorMismatch {
                expected: 0,
                actual: major,
            });
        }
        Ok(value)
    }

    pub fn integer(&mut self) -> Result<i64> {
        let (major, value) = self.read_head()?;
        match major {
            0 => {
                i64::try_from(value).map_err(|_| CborError::NonCanonicalInteger { value, info: 27 })
            }
            1 => {
                let signed = -(value as i128) - 1;
                i64::try_from(signed)
                    .map_err(|_| CborError::NonCanonicalInteger { value, info: 27 })
            }
            actual => Err(CborError::MajorMismatch {
                expected: 0,
                actual,
            }),
        }
    }

    pub fn bytes(&mut self) -> Result<&'a [u8]> {
        let (major, length) = self.read_head()?;
        if major != 2 {
            return Err(CborError::MajorMismatch {
                expected: 2,
                actual: major,
            });
        }
        self.take_slice(length)
    }

    pub fn text(&mut self) -> Result<&'a str> {
        let (major, length) = self.read_head()?;
        if major != 3 {
            return Err(CborError::MajorMismatch {
                expected: 3,
                actual: major,
            });
        }
        let slice = self.take_slice(length)?;
        core::str::from_utf8(slice).map_err(|_| CborError::InvalidUtf8)
    }

    fn take_slice(&mut self, length: u64) -> Result<&'a [u8]> {
        let remaining = self.remaining();
        if length as u128 > remaining as u128 {
            return Err(CborError::LengthExceedsInput {
                claimed: length,
                remaining,
            });
        }
        let start = self.position;
        self.position += length as usize;
        Ok(&self.data[start..self.position])
    }

    fn enter(&mut self, major: u8, length: u64) -> Result<u64> {
        if self.depth >= self.max_depth {
            return Err(CborError::NestingLimitExceeded(self.max_depth));
        }
        // Nested items occupy at least one byte each and map entries at
        // least two (key + value), so claims beyond the remaining input are
        // malformed regardless of content.
        let minimum_per_entry: u128 = if major == 5 { 2 } else { 1 };
        let remaining = self.remaining() as u128;
        if length as u128 * minimum_per_entry > remaining {
            return Err(CborError::LengthExceedsInput {
                claimed: length,
                remaining: remaining as usize,
            });
        }
        debug_assert!(matches!(major, 4 | 5));
        self.depth += 1;
        Ok(length)
    }

    /// Begins an array and returns a guard carrying the item count. The
    /// guard dereferences back to this reader; dropping it closes the
    /// nesting level.
    pub fn array(&mut self) -> Result<SeqGuard<'_, 'a>> {
        let (major, length) = self.read_head()?;
        if major != 4 {
            return Err(CborError::MajorMismatch {
                expected: 4,
                actual: major,
            });
        }
        let count = self.enter(major, length)?;
        Ok(SeqGuard {
            reader: self,
            count,
        })
    }

    /// Begins a map and returns a guard carrying the entry count. Keys are
    /// read through [`MapGuard::next_key`], which enforces strictly
    /// ascending order.
    pub fn map(&mut self) -> Result<MapGuard<'_, 'a>> {
        let (major, length) = self.read_head()?;
        if major != 5 {
            return Err(CborError::MajorMismatch {
                expected: 5,
                actual: major,
            });
        }
        let count = self.enter(major, length)?;
        Ok(MapGuard {
            reader: self,
            remaining: count,
            previous_key: None,
        })
    }
}

impl fmt::Debug for SeqGuard<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SeqGuard")
            .field("count", &self.count)
            .finish()
    }
}

/// Open array scope; dereferences to the underlying [`CborReader`].
pub struct SeqGuard<'r, 'a> {
    reader: &'r mut CborReader<'a>,
    count: u64,
}

impl SeqGuard<'_, '_> {
    pub fn count(&self) -> u64 {
        self.count
    }
}

impl<'r, 'a> core::ops::Deref for SeqGuard<'r, 'a> {
    type Target = CborReader<'a>;

    fn deref(&self) -> &Self::Target {
        self.reader
    }
}

impl<'r, 'a> core::ops::DerefMut for SeqGuard<'r, 'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.reader
    }
}

impl Drop for SeqGuard<'_, '_> {
    fn drop(&mut self) {
        self.reader.depth -= 1;
    }
}

/// Open map scope with canonical key tracking.
pub struct MapGuard<'r, 'a> {
    reader: &'r mut CborReader<'a>,
    remaining: u64,
    previous_key: Option<u64>,
}

impl MapGuard<'_, '_> {
    /// Reads the next field key, or `None` once every declared entry has
    /// been consumed. Keys must strictly ascend.
    pub fn next_key(&mut self) -> Result<Option<u64>> {
        if self.remaining == 0 {
            return Ok(None);
        }
        self.remaining -= 1;
        let key = self.reader.unsigned()?;
        if let Some(previous) = self.previous_key {
            if key <= previous {
                return Err(CborError::MapKeyOutOfOrder {
                    previous,
                    current: key,
                });
            }
        }
        self.previous_key = Some(key);
        Ok(Some(key))
    }

    /// Reads the next key if it is exactly `expected`; otherwise the value
    /// is absent and the key is not consumed.
    pub fn optional_key(&mut self, expected: u64) -> Result<bool> {
        if self.remaining == 0 {
            return Ok(false);
        }
        // Peek by reading and un-reading: keys are at most 9 bytes, and
        // `read_head` always advances from a known position.
        let rewind = self.reader.position;
        let key = self.reader.unsigned();
        match key {
            Ok(key) if key == expected => {
                self.remaining -= 1;
                if let Some(previous) = self.previous_key {
                    if key <= previous {
                        return Err(CborError::MapKeyOutOfOrder {
                            previous,
                            current: key,
                        });
                    }
                }
                self.previous_key = Some(key);
                Ok(true)
            }
            Ok(_) => {
                self.reader.position = rewind;
                Ok(false)
            }
            Err(error) => {
                self.reader.position = rewind;
                Err(error)
            }
        }
    }

    /// Number of entries not yet consumed.
    pub fn remaining_entries(&self) -> u64 {
        self.remaining
    }
}

impl Drop for MapGuard<'_, '_> {
    fn drop(&mut self) {
        self.reader.depth -= 1;
    }
}

impl fmt::Debug for MapGuard<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapGuard")
            .field("remaining", &self.remaining)
            .field("previous_key", &self.previous_key)
            .finish()
    }
}

impl<'r, 'a> core::ops::Deref for MapGuard<'r, 'a> {
    type Target = CborReader<'a>;

    fn deref(&self) -> &Self::Target {
        self.reader
    }
}

impl<'r, 'a> core::ops::DerefMut for MapGuard<'r, 'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.reader
    }
}
