use std::fmt;
use std::path::{Path, PathBuf};

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostPathEncoding {
    UnixBytes,
    WindowsWtf16Le,
}

impl HostPathEncoding {
    const fn tag(self) -> u64 {
        match self {
            Self::UnixBytes => 1,
            Self::WindowsWtf16Le => 2,
        }
    }

    fn from_tag(tag: u64) -> Result<Self, HostPathError> {
        match tag {
            1 => Ok(Self::UnixBytes),
            2 => Ok(Self::WindowsWtf16Le),
            _ => Err(HostPathError::UnknownEncoding(tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostPathCarrier {
    encoding: HostPathEncoding,
    bytes: Vec<u8>,
}

impl HostPathCarrier {
    #[cfg(unix)]
    pub fn from_path(path: &Path) -> Result<Self, HostPathError> {
        use std::os::unix::ffi::OsStrExt;

        Self::new(
            HostPathEncoding::UnixBytes,
            path.as_os_str().as_bytes().to_vec(),
        )
    }

    #[cfg(windows)]
    pub fn from_path(path: &Path) -> Result<Self, HostPathError> {
        use std::os::windows::ffi::OsStrExt;

        let words = path.as_os_str().encode_wide().collect::<Vec<_>>();
        let byte_length = words
            .len()
            .checked_mul(2)
            .ok_or(HostPathError::LengthOverflow)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(byte_length)
            .map_err(|_| HostPathError::Allocation)?;
        for word in words {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        Self::new(HostPathEncoding::WindowsWtf16Le, bytes)
    }

    fn new(encoding: HostPathEncoding, bytes: Vec<u8>) -> Result<Self, HostPathError> {
        validate_path_bytes(encoding, &bytes)?;
        Ok(Self { encoding, bytes })
    }

    pub const fn encoding(&self) -> HostPathEncoding {
        self.encoding
    }

    pub fn raw_bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[cfg(unix)]
    pub fn to_path_buf(&self) -> Result<PathBuf, HostPathError> {
        use std::os::unix::ffi::OsStringExt;

        if self.encoding != HostPathEncoding::UnixBytes {
            return Err(HostPathError::WrongHostEncoding {
                expected: HostPathEncoding::UnixBytes,
                actual: self.encoding,
            });
        }
        Ok(PathBuf::from(std::ffi::OsString::from_vec(
            self.bytes.clone(),
        )))
    }

    #[cfg(windows)]
    pub fn to_path_buf(&self) -> Result<PathBuf, HostPathError> {
        use std::os::windows::ffi::OsStringExt;

        if self.encoding != HostPathEncoding::WindowsWtf16Le {
            return Err(HostPathError::WrongHostEncoding {
                expected: HostPathEncoding::WindowsWtf16Le,
                actual: self.encoding,
            });
        }
        let words = self
            .bytes
            .chunks_exact(2)
            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
            .collect::<Vec<_>>();
        Ok(PathBuf::from(std::ffi::OsString::from_wide(&words)))
    }
}

impl WireEncode for HostPathCarrier {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.unsigned(self.encoding.tag())?;
        encoder.field(2)?;
        encoder.bytes(&self.bytes)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DecodedHostPathCarrier {
    encoding: u64,
    bytes: Vec<u8>,
}

impl DecodedHostPathCarrier {
    pub(crate) fn validate(self) -> Result<HostPathCarrier, HostPathError> {
        let encoding = HostPathEncoding::from_tag(self.encoding)?;
        validate_host_encoding(encoding)?;
        HostPathCarrier::new(encoding, self.bytes)
    }
}

impl WireEncode for DecodedHostPathCarrier {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.unsigned(self.encoding)?;
        encoder.field(2)?;
        encoder.bytes(&self.bytes)
    }
}

impl WireDecode for DecodedHostPathCarrier {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let encoding = decoder.field(1, Decoder::unsigned)?;
        let bytes = decoder.field(2, Decoder::owned_bytes)?;
        Ok(Self { encoding, bytes })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostPathError {
    Empty,
    LengthOverflow,
    ContainsNul,
    InvalidWindowsByteLength,
    UnknownEncoding(u64),
    WrongHostEncoding {
        expected: HostPathEncoding,
        actual: HostPathEncoding,
    },
    Allocation,
}

impl fmt::Display for HostPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("host path must not be empty"),
            Self::LengthOverflow => formatter.write_str("host path byte length overflows usize"),
            Self::ContainsNul => formatter.write_str("host path must not contain NUL"),
            Self::InvalidWindowsByteLength => {
                formatter.write_str("Windows host path must contain complete little-endian u16s")
            }
            Self::UnknownEncoding(tag) => write!(formatter, "unknown host path encoding tag {tag}"),
            Self::WrongHostEncoding { expected, actual } => write!(
                formatter,
                "host path encoding {actual:?} cannot be used on this host; expected {expected:?}"
            ),
            Self::Allocation => formatter.write_str("failed to allocate host path carrier"),
        }
    }
}

impl std::error::Error for HostPathError {}

fn validate_path_bytes(encoding: HostPathEncoding, bytes: &[u8]) -> Result<(), HostPathError> {
    if bytes.is_empty() {
        return Err(HostPathError::Empty);
    }
    match encoding {
        HostPathEncoding::UnixBytes => {
            if bytes.contains(&0) {
                return Err(HostPathError::ContainsNul);
            }
        }
        HostPathEncoding::WindowsWtf16Le => {
            if !bytes.chunks_exact(2).remainder().is_empty() {
                return Err(HostPathError::InvalidWindowsByteLength);
            }
            if bytes
                .chunks_exact(2)
                .any(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]) == 0)
            {
                return Err(HostPathError::ContainsNul);
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn validate_host_encoding(encoding: HostPathEncoding) -> Result<(), HostPathError> {
    if encoding == HostPathEncoding::UnixBytes {
        Ok(())
    } else {
        Err(HostPathError::WrongHostEncoding {
            expected: HostPathEncoding::UnixBytes,
            actual: encoding,
        })
    }
}

#[cfg(windows)]
fn validate_host_encoding(encoding: HostPathEncoding) -> Result<(), HostPathError> {
    if encoding == HostPathEncoding::WindowsWtf16Le {
        Ok(())
    } else {
        Err(HostPathError::WrongHostEncoding {
            expected: HostPathEncoding::WindowsWtf16Le,
            actual: encoding,
        })
    }
}

#[cfg(test)]
mod tests {
    use scoop_wire::{decode_canonical, encode};

    use super::*;

    #[test]
    fn host_path_round_trips_without_utf8_conversion() {
        #[cfg(unix)]
        let path = {
            use std::os::unix::ffi::OsStringExt;
            PathBuf::from(std::ffi::OsString::from_vec(vec![b'a', 0xff, b'b']))
        };
        #[cfg(windows)]
        let path = PathBuf::from("opaque-path");

        let carrier = HostPathCarrier::from_path(&path).unwrap();
        let decoded = decode_canonical::<DecodedHostPathCarrier>(&encode(&carrier).unwrap())
            .unwrap()
            .validate()
            .unwrap();
        assert_eq!(decoded.to_path_buf().unwrap(), path);
    }

    #[test]
    fn path_validation_rejects_empty_nul_and_wrong_width() {
        assert_eq!(
            HostPathCarrier::new(HostPathEncoding::UnixBytes, Vec::new()).unwrap_err(),
            HostPathError::Empty
        );
        assert_eq!(
            HostPathCarrier::new(HostPathEncoding::UnixBytes, vec![0]).unwrap_err(),
            HostPathError::ContainsNul
        );
        assert_eq!(
            HostPathCarrier::new(HostPathEncoding::WindowsWtf16Le, vec![1]).unwrap_err(),
            HostPathError::InvalidWindowsByteLength
        );
        #[cfg(unix)]
        assert!(matches!(
            DecodedHostPathCarrier {
                encoding: HostPathEncoding::WindowsWtf16Le.tag(),
                bytes: vec![b'a', 0],
            }
            .validate(),
            Err(HostPathError::WrongHostEncoding { .. })
        ));

        #[cfg(windows)]
        assert!(matches!(
            DecodedHostPathCarrier {
                encoding: HostPathEncoding::UnixBytes.tag(),
                bytes: vec![b'a'],
            }
            .validate(),
            Err(HostPathError::WrongHostEncoding { .. })
        ));
    }
}
