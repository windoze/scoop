use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalIdentifier(String);

impl CanonicalIdentifier {
    pub fn new(value: &str) -> Result<Self, CanonicalIdentifierError> {
        validate_identifier(value)?;
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl WireEncode for CanonicalIdentifier {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.text(&self.0)
    }
}

impl fmt::Display for CanonicalIdentifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalIdentifier(String);

impl DecodedCanonicalIdentifier {
    pub fn byte_len(&self) -> usize {
        self.0.len()
    }

    pub fn validate(self) -> Result<CanonicalIdentifier, CanonicalIdentifierError> {
        CanonicalIdentifier::new(&self.0)
    }
}

impl WireEncode for DecodedCanonicalIdentifier {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.text(&self.0)
    }
}

impl WireDecode for DecodedCanonicalIdentifier {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.owned_text().map(Self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalIdentifierError {
    Empty,
    NonAscii,
    InvalidStart,
    InvalidContinuation,
}

impl fmt::Display for CanonicalIdentifierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "canonical identifier must not be empty",
            Self::NonAscii => "canonical identifier must be ASCII",
            Self::InvalidStart => {
                "canonical identifier must start with an ASCII letter or underscore"
            }
            Self::InvalidContinuation => {
                "canonical identifier may contain only ASCII letters, digits, and underscores"
            }
        })
    }
}

impl std::error::Error for CanonicalIdentifierError {}

fn validate_identifier(value: &str) -> Result<(), CanonicalIdentifierError> {
    if value.is_empty() {
        return Err(CanonicalIdentifierError::Empty);
    }
    if !value.is_ascii() {
        return Err(CanonicalIdentifierError::NonAscii);
    }
    let mut bytes = value.bytes();
    if !bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
    {
        return Err(CanonicalIdentifierError::InvalidStart);
    }
    if !bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_') {
        return Err(CanonicalIdentifierError::InvalidContinuation);
    }
    Ok(())
}

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PackagePath(Vec<CanonicalIdentifier>);

impl PackagePath {
    pub fn root() -> Self {
        Self(Vec::new())
    }

    pub fn from_segments(segments: Vec<CanonicalIdentifier>) -> Self {
        Self(segments)
    }

    pub fn segments(&self) -> &[CanonicalIdentifier] {
        &self.0
    }

    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }
}

impl WireEncode for PackagePath {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for segment in &self.0 {
            segment.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedPackagePath(Vec<String>);

impl DecodedPackagePath {
    pub fn validate(self) -> Result<PackagePath, CanonicalIdentifierError> {
        let segments = self
            .0
            .into_iter()
            .map(|segment| CanonicalIdentifier::new(&segment))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(PackagePath::from_segments(segments))
    }
}

impl WireEncode for DecodedPackagePath {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for segment in &self.0 {
            encoder.text(segment)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedPackagePath {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| decoder.owned_text())
            .map(Self)
    }
}

#[cfg(test)]
mod tests {
    use scoop_wire::{decode_canonical, encode};

    use super::{CanonicalIdentifier, CanonicalIdentifierError, DecodedPackagePath, PackagePath};

    #[test]
    fn identifier_matches_the_language_lexer_grammar() {
        for value in ["name", "Name2", "_", "_private"] {
            assert!(CanonicalIdentifier::new(value).is_ok());
        }
        for value in ["", "2fast", "hyphen-name", "café"] {
            assert!(CanonicalIdentifier::new(value).is_err());
        }
        assert_eq!(
            CanonicalIdentifier::new("2fast"),
            Err(CanonicalIdentifierError::InvalidStart)
        );
    }

    #[test]
    fn root_and_segmented_package_paths_have_fixed_wire() {
        assert_eq!(encode(&PackagePath::root()).unwrap(), b"\x80");
        let path = PackagePath::from_segments(vec![
            CanonicalIdentifier::new("org").unwrap(),
            CanonicalIdentifier::new("example").unwrap(),
        ]);
        let bytes = encode(&path).unwrap();
        assert_eq!(bytes, b"\x82\x63org\x67example");
        assert_eq!(
            decode_canonical::<DecodedPackagePath>(&bytes)
                .unwrap()
                .validate()
                .unwrap(),
            path
        );
    }

    #[test]
    fn decoded_package_path_rejects_invalid_segments() {
        let decoded = decode_canonical::<DecodedPackagePath>(b"\x81\x69not-valid").unwrap();
        assert_eq!(
            decoded.validate(),
            Err(CanonicalIdentifierError::InvalidContinuation)
        );
    }
}
