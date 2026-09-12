use std::fmt;

use scoop_wire::{Encoder, WireEncode};

mod decode;

pub use decode::{DecodedCanonicalNativeLibraryName, DecodedSourceNativeSymbol};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalNativeLibraryName(String);

impl CanonicalNativeLibraryName {
    pub fn new(value: &str) -> Result<Self, CanonicalNativeNameError> {
        Self::from_owned(value.to_owned())
    }

    pub fn from_owned(value: String) -> Result<Self, CanonicalNativeNameError> {
        validate_native_name(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl WireEncode for CanonicalNativeLibraryName {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.text(&self.0)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceNativeSymbol(Vec<u8>);

impl SourceNativeSymbol {
    pub fn new(value: &str) -> Result<Self, SourceNativeSymbolError> {
        if value.is_empty() {
            return Err(SourceNativeSymbolError::Empty);
        }
        if value.len() > 4_095 {
            return Err(SourceNativeSymbolError::TooLong);
        }
        if value.as_bytes().contains(&0) {
            return Err(SourceNativeSymbolError::Nul);
        }
        Ok(Self(value.as_bytes().to_vec()))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl WireEncode for SourceNativeSymbol {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalNativeNameError {
    Empty,
    TooLong,
    DotPath,
    ForbiddenCharacter,
    EdgeWhitespace,
}

impl fmt::Display for CanonicalNativeNameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "native library name must not be empty",
            Self::TooLong => "native library name exceeds 255 UTF-8 bytes",
            Self::DotPath => "native library name must not be '.' or '..'",
            Self::ForbiddenCharacter => {
                "native library name contains a forbidden control or path character"
            }
            Self::EdgeWhitespace => {
                "native library name must not start or end with ASCII whitespace"
            }
        })
    }
}

impl std::error::Error for CanonicalNativeNameError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceNativeSymbolError {
    Empty,
    TooLong,
    InvalidUtf8,
    Nul,
}

impl fmt::Display for SourceNativeSymbolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "source native symbol must not be empty",
            Self::TooLong => "source native symbol exceeds 4095 UTF-8 bytes",
            Self::InvalidUtf8 => "source native symbol must be valid UTF-8",
            Self::Nul => "source native symbol must not contain NUL",
        })
    }
}

impl std::error::Error for SourceNativeSymbolError {}

pub(crate) fn validate_native_name(value: &str) -> Result<(), CanonicalNativeNameError> {
    if value.is_empty() {
        return Err(CanonicalNativeNameError::Empty);
    }
    if value.len() > 255 {
        return Err(CanonicalNativeNameError::TooLong);
    }
    if matches!(value, "." | "..") {
        return Err(CanonicalNativeNameError::DotPath);
    }
    if value
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_whitespace)
        || value.as_bytes().last().is_some_and(u8::is_ascii_whitespace)
    {
        return Err(CanonicalNativeNameError::EdgeWhitespace);
    }
    if value
        .as_bytes()
        .iter()
        .any(|byte| byte.is_ascii_control() || matches!(byte, b'/' | b'\\' | 0))
    {
        return Err(CanonicalNativeNameError::ForbiddenCharacter);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::{CanonicalNativeLibraryName, SourceNativeSymbol};

    #[test]
    fn native_library_name_preserves_validated_utf8() {
        let name = CanonicalNativeLibraryName::new("Résumé-库").unwrap();
        assert_eq!(name.as_str(), "Résumé-库");
        assert_eq!(
            encode(&name).unwrap(),
            "Résumé-库".as_bytes_with_cbor_text()
        );
        assert_eq!(
            CanonicalNativeLibraryName::from_owned("native-core".to_owned())
                .unwrap()
                .as_str(),
            "native-core"
        );
    }

    #[test]
    fn native_library_name_rejects_path_and_edge_forms() {
        for value in ["", ".", "..", "/usr/lib", "lib\\name", " lib", "lib "] {
            assert!(CanonicalNativeLibraryName::new(value).is_err(), "{value:?}");
        }
        assert!(CanonicalNativeLibraryName::new(&"a".repeat(256)).is_err());
    }

    #[test]
    fn source_symbol_is_bytes_and_rejects_nul() {
        let symbol = SourceNativeSymbol::new("_入口").unwrap();
        let encoded = encode(&symbol).unwrap();
        assert_eq!(encoded[0], 0x40 + symbol.as_bytes().len() as u8);
        assert_eq!(&encoded[1..], symbol.as_bytes());
        assert!(SourceNativeSymbol::new("a\0b").is_err());
    }

    trait CborText {
        fn as_bytes_with_cbor_text(&self) -> Vec<u8>;
    }

    impl CborText for str {
        fn as_bytes_with_cbor_text(&self) -> Vec<u8> {
            let bytes = self.as_bytes();
            assert!(bytes.len() < 24);
            [vec![0x60 + bytes.len() as u8], bytes.to_vec()].concat()
        }
    }
}
