use std::fmt;

use scoop_wire::{Decoder, Encoder, HashError, WireDecode, WireEncode, WireError};

pub use crate::ids::ConeIdentity;
use crate::ids::derive_persistent_id;

const CONE_ID_DOMAIN_V1: &str = "scoop-cone-id-v1";

impl ConeIdentity {
    pub const CORE: Self = Self::from_protocol_bytes([
        0x5e, 0xa5, 0xf5, 0xe8, 0xff, 0x24, 0x81, 0x82, 0xc8, 0xf8, 0xc7, 0xe1, 0x04, 0x3c, 0xaa,
        0xe2, 0x0f, 0x16, 0x3b, 0xce, 0xfd, 0x34, 0xcc, 0xa4, 0xe9, 0x7d, 0x8c, 0x6a, 0x03, 0xbf,
        0x62, 0x0d,
    ]);

    pub const SINGLE_FILE: Self = Self::from_protocol_bytes([
        0x00, 0x76, 0x9a, 0xf7, 0xcd, 0x4a, 0x85, 0xd9, 0x88, 0x41, 0xcd, 0x63, 0xda, 0xbb, 0x8e,
        0x73, 0xc4, 0xd4, 0x1b, 0xce, 0x56, 0xf5, 0xe2, 0x25, 0xf7, 0x29, 0x5d, 0xd2, 0x7c, 0x3f,
        0x39, 0xb6,
    ]);

    const fn from_protocol_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConeCoordinate {
    group: String,
    name: String,
    version: String,
}

impl ConeCoordinate {
    pub fn new(group: &str, name: &str, version: &str) -> Result<Self, ConeCoordinateError> {
        validate_coordinate_text(ConeCoordinateComponent::Group, group)?;
        validate_coordinate_text(ConeCoordinateComponent::Name, name)?;
        let parsed = semver::Version::parse(version)
            .map_err(|_| ConeCoordinateError::InvalidSemanticVersion)?;
        if parsed.to_string() != version {
            return Err(ConeCoordinateError::NonCanonicalSemanticVersion);
        }
        Ok(Self {
            group: group.to_owned(),
            name: name.to_owned(),
            version: version.to_owned(),
        })
    }

    pub fn reserved_core() -> Self {
        Self {
            group: "scoop".to_owned(),
            name: "scoop.core".to_owned(),
            version: "0.1.0".to_owned(),
        }
    }

    pub fn reserved_single_file() -> Self {
        Self {
            group: "scoop".to_owned(),
            name: "single-file".to_owned(),
            version: "0.0.0".to_owned(),
        }
    }

    pub fn group(&self) -> &str {
        &self.group
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn identity(&self) -> Result<ConeIdentity, HashError> {
        derive_persistent_id(CONE_ID_DOMAIN_V1, self)
    }
}

impl WireEncode for ConeCoordinate {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        encoder.text(&self.group)?;
        encoder.field(2)?;
        encoder.text(&self.name)?;
        encoder.field(3)?;
        encoder.text(&self.version)
    }
}

impl fmt::Display for ConeCoordinate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}:{}", self.group, self.name, self.version)
    }
}

/// Syntactically decoded coordinate that has not passed semantic validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedConeCoordinate {
    group: String,
    name: String,
    version: String,
}

impl DecodedConeCoordinate {
    pub fn validate(self) -> Result<ConeCoordinate, ConeCoordinateError> {
        ConeCoordinate::new(&self.group, &self.name, &self.version)
    }
}

impl WireEncode for DecodedConeCoordinate {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        encoder.text(&self.group)?;
        encoder.field(2)?;
        encoder.text(&self.name)?;
        encoder.field(3)?;
        encoder.text(&self.version)
    }
}

impl WireDecode for DecodedConeCoordinate {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        let group = decoder.field(1, Decoder::owned_text)?;
        let name = decoder.field(2, Decoder::owned_text)?;
        let version = decoder.field(3, Decoder::owned_text)?;
        Ok(Self {
            group,
            name,
            version,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConeCoordinateComponent {
    Group,
    Name,
}

impl fmt::Display for ConeCoordinateComponent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Group => "group",
            Self::Name => "name",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConeCoordinateTextError {
    Empty,
    NonAscii,
    EmptySegment,
    InvalidSegmentStart,
    InvalidSegmentCharacter,
}

impl fmt::Display for ConeCoordinateTextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "must not be empty",
            Self::NonAscii => "must be ASCII",
            Self::EmptySegment => "contains an empty dot-separated segment",
            Self::InvalidSegmentStart => "segment must start with a lowercase ASCII letter",
            Self::InvalidSegmentCharacter => {
                "segment may contain only lowercase ASCII letters, digits, and hyphens"
            }
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConeCoordinateError {
    InvalidText {
        component: ConeCoordinateComponent,
        reason: ConeCoordinateTextError,
    },
    InvalidSemanticVersion,
    NonCanonicalSemanticVersion,
}

impl fmt::Display for ConeCoordinateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidText { component, reason } => {
                write!(formatter, "invalid Cone coordinate {component}: {reason}")
            }
            Self::InvalidSemanticVersion => {
                formatter.write_str("invalid Cone coordinate semantic version")
            }
            Self::NonCanonicalSemanticVersion => {
                formatter.write_str("Cone coordinate semantic version is not canonical")
            }
        }
    }
}

impl std::error::Error for ConeCoordinateError {}

fn validate_coordinate_text(
    component: ConeCoordinateComponent,
    value: &str,
) -> Result<(), ConeCoordinateError> {
    let invalid = |reason| ConeCoordinateError::InvalidText { component, reason };
    if value.is_empty() {
        return Err(invalid(ConeCoordinateTextError::Empty));
    }
    if !value.is_ascii() {
        return Err(invalid(ConeCoordinateTextError::NonAscii));
    }
    for segment in value.split('.') {
        if segment.is_empty() {
            return Err(invalid(ConeCoordinateTextError::EmptySegment));
        }
        let mut bytes = segment.bytes();
        if !bytes.next().is_some_and(|byte| byte.is_ascii_lowercase()) {
            return Err(invalid(ConeCoordinateTextError::InvalidSegmentStart));
        }
        if !bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-') {
            return Err(invalid(ConeCoordinateTextError::InvalidSegmentCharacter));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use scoop_wire::{DecodeLimits, decode_canonical, encode};

    use super::{ConeCoordinate, ConeCoordinateError, ConeIdentity, DecodedConeCoordinate};

    #[test]
    fn reserved_coordinates_have_fixed_wire_and_hash_vectors() {
        let core = ConeCoordinate::reserved_core();
        assert_eq!(
            encode(&core).unwrap(),
            b"\xa3\x01\x65scoop\x02\x6ascoop.core\x03\x650.1.0"
        );
        assert_eq!(core.identity().unwrap(), ConeIdentity::CORE);
        assert_eq!(
            core.identity().unwrap().to_string(),
            "5ea5f5e8ff248182c8f8c7e1043caae20f163bcefd34cca4e97d8c6a03bf620d"
        );

        let single = ConeCoordinate::reserved_single_file();
        assert_eq!(
            encode(&single).unwrap(),
            b"\xa3\x01\x65scoop\x02\x6bsingle-file\x03\x650.0.0"
        );
        assert_eq!(single.identity().unwrap(), ConeIdentity::SINGLE_FILE);
        assert_eq!(
            single.identity().unwrap().to_string(),
            "00769af7cd4a85d98841cd63dabb8e73c4d41bce56f5e225f7295dd27c3f39b6"
        );
    }

    #[test]
    fn coordinate_decode_requires_a_separate_validation_step() {
        let bytes = encode(&ConeCoordinate::reserved_core()).unwrap();
        let decoded =
            decode_canonical::<DecodedConeCoordinate>(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(decoded.validate().unwrap(), ConeCoordinate::reserved_core());
    }

    #[test]
    fn coordinate_grammar_is_closed() {
        for (group, name) in [
            ("", "valid"),
            ("Upper", "valid"),
            ("two..parts", "valid"),
            ("1start", "valid"),
            ("bad_underscore", "valid"),
            ("válid", "valid"),
        ] {
            assert!(ConeCoordinate::new(group, name, "1.0.0").is_err());
        }
        assert!(ConeCoordinate::new("org.example", "my-cone2", "1.0.0-alpha.1+build.7").is_ok());
    }

    #[test]
    fn semantic_version_must_equal_its_canonical_rendering() {
        assert_eq!(
            ConeCoordinate::new("org", "name", "01.0.0"),
            Err(ConeCoordinateError::InvalidSemanticVersion)
        );
        assert_eq!(
            ConeCoordinate::new("org", "name", "v1.0.0"),
            Err(ConeCoordinateError::InvalidSemanticVersion)
        );
        assert!(ConeCoordinate::new("org", "name", "1.0.0 ").is_err());
    }
}
