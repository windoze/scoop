use std::fmt;
use std::path::{Component, Path};

use scoop_wire::{Decoder, Encoder, HashError, WireDecode, WireEncode, WireError};

use crate::{ConeCoordinate, ConeIdentity, DecodedPersistentId, PersistentIdMismatch};

const SINGLE_FILE_LOGICAL_PATH: &str = "main.scoop";

/// SHA-256 of the raw UTF-8 bytes of one source file.
///
/// This remains a distinct type from semantic identities and other digests so
/// callers cannot accidentally substitute a content hash for an entity id.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceContentDigest([u8; 32]);

impl SourceContentDigest {
    pub fn from_utf8(source: &str) -> Self {
        Self(*scoop_wire::sha256(source.as_bytes()).as_array())
    }

    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}

impl WireEncode for SourceContentDigest {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl fmt::Display for SourceContentDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NormalizedSourcePath(String);

impl NormalizedSourcePath {
    pub fn new(value: &str) -> Result<Self, NormalizedSourcePathError> {
        validate_source_path(value)?;
        Ok(Self(value.to_owned()))
    }

    pub fn single_file() -> Self {
        Self(SINGLE_FILE_LOGICAL_PATH.to_owned())
    }

    /// Constructs canonical `/`-separated text from host filesystem
    /// components without performing lossy string replacement.
    pub fn from_relative_path(path: &Path) -> Result<Self, NormalizedSourcePathError> {
        let mut normalized = String::new();
        for component in path.components() {
            let segment = match component {
                Component::Normal(segment) => {
                    segment.to_str().ok_or(NormalizedSourcePathError::NonUtf8)?
                }
                Component::CurDir => return Err(NormalizedSourcePathError::CurrentSegment),
                Component::ParentDir => return Err(NormalizedSourcePathError::ParentSegment),
                Component::RootDir => return Err(NormalizedSourcePathError::Absolute),
                Component::Prefix(_) => return Err(NormalizedSourcePathError::HostPrefix),
            };
            let extra = segment
                .len()
                .checked_add(usize::from(!normalized.is_empty()))
                .ok_or(NormalizedSourcePathError::Allocation)?;
            normalized
                .try_reserve(extra)
                .map_err(|_| NormalizedSourcePathError::Allocation)?;
            if !normalized.is_empty() {
                normalized.push('/');
            }
            normalized.push_str(segment);
        }
        Self::new(&normalized)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl WireEncode for NormalizedSourcePath {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.text(&self.0)
    }
}

impl fmt::Display for NormalizedSourcePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedNormalizedSourcePath(String);

impl DecodedNormalizedSourcePath {
    pub fn validate(self) -> Result<NormalizedSourcePath, NormalizedSourcePathError> {
        NormalizedSourcePath::new(&self.0)
    }
}

impl WireEncode for DecodedNormalizedSourcePath {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.text(&self.0)
    }
}

impl WireDecode for DecodedNormalizedSourcePath {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.owned_text().map(Self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NormalizedSourcePathError {
    Empty,
    Absolute,
    HostPrefix,
    Backslash,
    Nul,
    EmptySegment,
    CurrentSegment,
    ParentSegment,
    NonUtf8,
    Allocation,
}

impl fmt::Display for NormalizedSourcePathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "normalized source path must not be empty",
            Self::Absolute => "normalized source path must be relative",
            Self::HostPrefix => "normalized source path must not contain a host path prefix",
            Self::Backslash => "normalized source path must use forward slashes",
            Self::Nul => "normalized source path must not contain NUL",
            Self::EmptySegment => "normalized source path must not contain an empty segment",
            Self::CurrentSegment => "normalized source path must not contain a '.' segment",
            Self::ParentSegment => "normalized source path must not contain a '..' segment",
            Self::NonUtf8 => "normalized source path filesystem component must be UTF-8",
            Self::Allocation => "failed to allocate normalized source path",
        })
    }
}

impl std::error::Error for NormalizedSourcePathError {}

fn validate_source_path(value: &str) -> Result<(), NormalizedSourcePathError> {
    if value.is_empty() {
        return Err(NormalizedSourcePathError::Empty);
    }
    if value.starts_with('/') {
        return Err(NormalizedSourcePathError::Absolute);
    }
    if value.contains('\\') {
        return Err(NormalizedSourcePathError::Backslash);
    }
    if value.contains('\0') {
        return Err(NormalizedSourcePathError::Nul);
    }
    let first = value.split('/').next().unwrap_or_default().as_bytes();
    if first.len() >= 2 && first[0].is_ascii_alphabetic() && first[1] == b':' {
        return Err(NormalizedSourcePathError::HostPrefix);
    }
    for segment in value.split('/') {
        match segment {
            "" => return Err(NormalizedSourcePathError::EmptySegment),
            "." => return Err(NormalizedSourcePathError::CurrentSegment),
            ".." => return Err(NormalizedSourcePathError::ParentSegment),
            _ => {}
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceIdentity {
    cone: ConeIdentity,
    logical_path: NormalizedSourcePath,
}

impl SourceIdentity {
    pub fn new(
        cone: ConeIdentity,
        logical_path: NormalizedSourcePath,
    ) -> Result<Self, SourceIdentityError> {
        if cone == ConeIdentity::SINGLE_FILE && logical_path.as_str() != SINGLE_FILE_LOGICAL_PATH {
            return Err(SourceIdentityError::InvalidSingleFilePath);
        }
        Ok(Self { cone, logical_path })
    }

    pub fn single_file() -> Self {
        Self {
            cone: ConeIdentity::SINGLE_FILE,
            logical_path: NormalizedSourcePath::single_file(),
        }
    }

    pub fn cone(&self) -> ConeIdentity {
        self.cone
    }

    pub fn logical_path(&self) -> &NormalizedSourcePath {
        &self.logical_path
    }

    pub fn canonical_semantic_name(
        &self,
        coordinate: &ConeCoordinate,
    ) -> Result<String, SemanticSourceNameError> {
        let coordinate_identity = coordinate
            .identity()
            .map_err(SemanticSourceNameError::Hash)?;
        if coordinate_identity != self.cone {
            return Err(SemanticSourceNameError::ConeMismatch);
        }
        let coordinate_text = coordinate.to_string();
        let capacity = coordinate_text
            .len()
            .checked_add(1)
            .and_then(|length| length.checked_add(self.logical_path.as_str().len()))
            .ok_or(SemanticSourceNameError::Allocation)?;
        let mut name = String::new();
        name.try_reserve_exact(capacity)
            .map_err(|_| SemanticSourceNameError::Allocation)?;
        name.push_str(&coordinate_text);
        name.push('/');
        name.push_str(self.logical_path.as_str());
        Ok(name)
    }
}

impl WireEncode for SourceIdentity {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.cone.encode(encoder)?;
        encoder.field(2)?;
        self.logical_path.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedSourceIdentity {
    cone: DecodedPersistentId<ConeIdentity>,
    logical_path: DecodedNormalizedSourcePath,
}

impl DecodedSourceIdentity {
    pub fn validate(
        self,
        coordinate: &ConeCoordinate,
    ) -> Result<SourceIdentity, SourceIdentityDecodeError> {
        let expected = coordinate
            .identity()
            .map_err(SourceIdentityDecodeError::Hash)?;
        let cone = self
            .cone
            .verify(expected)
            .map_err(SourceIdentityDecodeError::ConeMismatch)?;
        let path = self
            .logical_path
            .validate()
            .map_err(SourceIdentityDecodeError::Path)?;
        SourceIdentity::new(cone, path).map_err(SourceIdentityDecodeError::Identity)
    }
}

impl WireEncode for DecodedSourceIdentity {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.cone.encode(encoder)?;
        encoder.field(2)?;
        self.logical_path.encode(encoder)
    }
}

impl WireDecode for DecodedSourceIdentity {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let cone = decoder.field(1, DecodedPersistentId::decode)?;
        let logical_path = decoder.field(2, DecodedNormalizedSourcePath::decode)?;
        Ok(Self { cone, logical_path })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceIdentityError {
    InvalidSingleFilePath,
}

impl fmt::Display for SourceIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the reserved single-file Cone must use logical path 'main.scoop'")
    }
}

impl std::error::Error for SourceIdentityError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticSourceNameError {
    Hash(HashError),
    ConeMismatch,
    Allocation,
}

impl fmt::Display for SemanticSourceNameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hash(error) => write!(formatter, "failed to hash Cone coordinate: {error}"),
            Self::ConeMismatch => formatter.write_str(
                "the supplied Cone coordinate does not match the source's Cone identity",
            ),
            Self::Allocation => formatter.write_str("failed to allocate canonical source name"),
        }
    }
}

impl std::error::Error for SemanticSourceNameError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceIdentityDecodeError {
    Hash(HashError),
    ConeMismatch(PersistentIdMismatch<ConeIdentity>),
    Path(NormalizedSourcePathError),
    Identity(SourceIdentityError),
}

impl fmt::Display for SourceIdentityDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hash(error) => write!(formatter, "failed to hash Cone coordinate: {error}"),
            Self::ConeMismatch(error) => error.fmt(formatter),
            Self::Path(error) => error.fmt(formatter),
            Self::Identity(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SourceIdentityDecodeError {}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use scoop_wire::{DecodeLimits, decode_canonical, encode};

    use super::{
        NormalizedSourcePath, NormalizedSourcePathError, SourceContentDigest, SourceIdentity,
        SourceIdentityError,
    };
    use crate::{ConeCoordinate, ConeIdentity, DecodedSourceIdentity};

    #[test]
    fn source_paths_accept_only_canonical_relative_form() {
        for valid in ["main.scoop", "src/model/User.scoop", "src/模型.scoop"] {
            assert!(NormalizedSourcePath::new(valid).is_ok());
        }
        for (invalid, expected) in [
            ("", NormalizedSourcePathError::Empty),
            ("/src/main.scoop", NormalizedSourcePathError::Absolute),
            ("C:/src/main.scoop", NormalizedSourcePathError::HostPrefix),
            ("src\\main.scoop", NormalizedSourcePathError::Backslash),
            ("src//main.scoop", NormalizedSourcePathError::EmptySegment),
            (
                "src/./main.scoop",
                NormalizedSourcePathError::CurrentSegment,
            ),
            (
                "src/../main.scoop",
                NormalizedSourcePathError::ParentSegment,
            ),
        ] {
            assert_eq!(NormalizedSourcePath::new(invalid), Err(expected));
        }
    }

    #[test]
    fn single_file_pair_is_enforced_by_the_constructor() {
        assert_eq!(
            SourceIdentity::new(
                ConeIdentity::SINGLE_FILE,
                NormalizedSourcePath::new("other.scoop").unwrap(),
            ),
            Err(SourceIdentityError::InvalidSingleFilePath)
        );
        assert_eq!(
            SourceIdentity::single_file().logical_path().as_str(),
            "main.scoop"
        );
    }

    #[test]
    fn filesystem_path_is_built_from_relative_components() {
        assert_eq!(
            NormalizedSourcePath::from_relative_path(Path::new("src/model/User.scoop"))
                .unwrap()
                .as_str(),
            "src/model/User.scoop"
        );
        assert_eq!(
            NormalizedSourcePath::from_relative_path(Path::new("../User.scoop")),
            Err(NormalizedSourcePathError::ParentSegment)
        );
        assert_eq!(
            NormalizedSourcePath::from_relative_path(Path::new("/src/User.scoop")),
            Err(NormalizedSourcePathError::Absolute)
        );
    }

    #[test]
    fn source_identity_has_fixed_wire_and_semantic_name() {
        let source = SourceIdentity::single_file();
        let bytes = encode(&source).unwrap();
        assert_eq!(
            bytes,
            [
                b"\xa2\x01\x58\x20".as_slice(),
                ConeIdentity::SINGLE_FILE.as_array(),
                b"\x02\x6amain.scoop".as_slice(),
            ]
            .concat()
        );
        assert_eq!(
            source
                .canonical_semantic_name(&ConeCoordinate::reserved_single_file())
                .unwrap(),
            "scoop:single-file:0.0.0/main.scoop"
        );
    }

    #[test]
    fn source_content_digest_hashes_raw_utf8_bytes() {
        let digest = SourceContentDigest::from_utf8("line 1\n雪\r\n");

        assert_eq!(
            digest.to_string(),
            "81c8d5cf14fd9585446d1985aaf5b73bd2dd5cb17466ac6c1c9965cd38fb6acc"
        );
        assert_eq!(
            encode(&digest).unwrap(),
            [vec![0x58, 0x20], digest.as_array().to_vec(),].concat()
        );
    }

    #[test]
    fn source_decode_verifies_the_coordinate_and_path() {
        let source = SourceIdentity::single_file();
        let decoded = decode_canonical::<DecodedSourceIdentity>(
            &encode(&source).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(
            decoded
                .validate(&ConeCoordinate::reserved_single_file())
                .unwrap(),
            source
        );
    }
}
