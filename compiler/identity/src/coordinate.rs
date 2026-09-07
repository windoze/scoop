//! Cone coordinates and identities (DESIGN sections 1.1 and 3.1).
//!
//! A coordinate is the stable textual identity used in manifests,
//! dependencies and diagnostics. No normalization is performed anywhere:
//! a coordinate that does not already match the canonical grammar is
//! rejected, so two coordinates are equal exactly when their texts are.

use core::fmt;
use std::cmp::Ordering;

use crate::digest::DomainHasher;
use crate::{CONE_IDENTITY_DOMAIN, CborWriter};

/// Error constructing a [`ConeCoordinate`] from non-canonical text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoordinateError {
    EmptyComponent(&'static str),
    EmptySegment(&'static str),
    InvalidCharacter { component: &'static str, byte: u8 },
    SegmentDoesNotStartWithLowercase { component: &'static str },
    EmptyVersion,
    VersionDetail(&'static str),
    VersionTooLarge(String),
}

impl fmt::Display for CoordinateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoordinateError::EmptyComponent(component) => {
                write!(f, "{component} must not be empty")
            }
            CoordinateError::EmptySegment(component) => {
                write!(f, "{component} contains an empty '.'-separated segment")
            }
            CoordinateError::InvalidCharacter { component, byte } => {
                write!(f, "{component} contains invalid byte 0x{byte:02x}")
            }
            CoordinateError::SegmentDoesNotStartWithLowercase { component } => {
                write!(
                    f,
                    "each {component} segment must start with a lowercase ASCII letter"
                )
            }
            CoordinateError::EmptyVersion => write!(f, "version must not be empty"),
            CoordinateError::VersionDetail(detail) => {
                write!(f, "invalid SemVer 2.0.0 version: {detail}")
            }
            CoordinateError::VersionTooLarge(component) => {
                write!(f, "version component {component} does not fit in u64")
            }
        }
    }
}

impl std::error::Error for CoordinateError {}

/// Validates one `.`-separated group/name component: lowercase ASCII
/// segments matching `[a-z][a-z0-9-]*`.
fn validate_component(component: &'static str, text: &str) -> Result<(), CoordinateError> {
    if text.is_empty() {
        return Err(CoordinateError::EmptyComponent(component));
    }
    for segment in text.split('.') {
        let mut chars = segment.bytes();
        match chars.next() {
            None => return Err(CoordinateError::EmptySegment(component)),
            Some(first) if !first.is_ascii_lowercase() => {
                return Err(CoordinateError::SegmentDoesNotStartWithLowercase { component });
            }
            Some(_) => {}
        }
        for byte in chars {
            if !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-') {
                return Err(CoordinateError::InvalidCharacter { component, byte });
            }
        }
    }
    Ok(())
}

/// One dot-separated pre-release identifier, either numeric or
/// alphanumeric; the two orders differently under SemVer precedence.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Identifier {
    Numeric(u64),
    Alphanumeric(String),
}

/// A canonical SemVer 2.0.0 version with its components pre-parsed for
/// ordering. The original text is preserved verbatim because it
/// participates in Cone identity.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Version {
    text: String,
    major: u64,
    minor: u64,
    patch: u64,
    prerelease: Vec<Identifier>,
    build: Vec<String>,
}

impl Version {
    /// Parses canonical SemVer 2.0.0 text. Leading zeros in numeric
    /// components, a `v` prefix, and any non-canonical spelling are
    /// errors; there is no fallback normalization.
    pub fn parse(text: &str) -> Result<Self, CoordinateError> {
        let invalid = |detail: &'static str| CoordinateError::VersionDetail(detail);
        if text.is_empty() {
            return Err(CoordinateError::EmptyVersion);
        }
        let (core_and_pre, build) = match text.split_once('+') {
            Some((head, build)) => {
                if build.is_empty() {
                    return Err(invalid("empty build metadata"));
                }
                (head, Some(build))
            }
            None => (text, None),
        };
        let (core, prerelease) = match core_and_pre.split_once('-') {
            Some((head, pre)) => {
                if pre.is_empty() {
                    return Err(invalid("empty pre-release"));
                }
                (head, Some(pre))
            }
            None => (core_and_pre, None),
        };
        let mut core_parts = core.split('.');
        let numbers = [
            core_parts.next().unwrap_or_default(),
            core_parts.next().unwrap_or_default(),
            core_parts.next().unwrap_or_default(),
        ];
        if core_parts.next().is_some() || !core.contains('.') {
            return Err(invalid("core must be exactly major.minor.patch"));
        }
        let parsed = [
            parse_numeric(numbers[0], "core")?,
            parse_numeric(numbers[1], "core")?,
            parse_numeric(numbers[2], "core")?,
        ];
        let prerelease = match prerelease {
            Some(text) => text
                .split('.')
                .map(parse_prerelease_identifier)
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };
        let build = match build {
            Some(text) => text
                .split('.')
                .map(|identifier| {
                    if identifier.is_empty()
                        || !identifier
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    {
                        Err(invalid("invalid build identifier"))
                    } else {
                        Ok(identifier.to_owned())
                    }
                })
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };
        Ok(Version {
            text: text.to_owned(),
            major: parsed[0],
            minor: parsed[1],
            patch: parsed[2],
            prerelease,
            build,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn major(&self) -> u64 {
        self.major
    }

    pub fn minor(&self) -> u64 {
        self.minor
    }

    pub fn patch(&self) -> u64 {
        self.patch
    }
}

fn parse_numeric(text: &str, role: &'static str) -> Result<u64, CoordinateError> {
    if text.is_empty() {
        return Err(CoordinateError::VersionDetail("empty numeric component"));
    }
    if !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(CoordinateError::VersionDetail(match role {
            "core" => "non-digit character in major/minor/patch",
            _ => "non-digit character",
        }));
    }
    if text != "0" && text.starts_with('0') {
        return Err(CoordinateError::VersionDetail(
            "numeric component has a leading zero",
        ));
    }
    text.parse::<u64>()
        .map_err(|_| CoordinateError::VersionTooLarge(text.to_owned()))
}

fn parse_prerelease_identifier(text: &str) -> Result<Identifier, CoordinateError> {
    let invalid = CoordinateError::VersionDetail("invalid pre-release identifier");
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return Err(invalid);
    }
    if text.bytes().all(|b| b.is_ascii_digit()) {
        if text != "0" && text.starts_with('0') {
            return Err(CoordinateError::VersionDetail(
                "numeric pre-release identifier has a leading zero",
            ));
        }
        text.parse::<u64>()
            .map(Identifier::Numeric)
            .map_err(|_| CoordinateError::VersionTooLarge(text.to_owned()))
    } else {
        Ok(Identifier::Alphanumeric(text.to_owned()))
    }
}

/// Orders identifiers under SemVer precedence: numeric identifiers are
/// lower than alphanumeric ones; numerics compare numerically and
/// alphanumerics by ASCII bytes.
fn identifier_order(left: &Identifier, right: &Identifier) -> Ordering {
    match (left, right) {
        (Identifier::Numeric(a), Identifier::Numeric(b)) => a.cmp(b),
        (Identifier::Numeric(_), Identifier::Alphanumeric(_)) => Ordering::Less,
        (Identifier::Alphanumeric(_), Identifier::Numeric(_)) => Ordering::Greater,
        (Identifier::Alphanumeric(a), Identifier::Alphanumeric(b)) => a.cmp(b),
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Total order extending SemVer precedence: build metadata is ignored by
/// SemVer precedence but still orders (by ASCII bytes) so that `Ord` and
/// `Eq` stay consistent. Graphs reject two versions of one `group:name`
/// anyway, so this tiebreak never picks between meaningful candidates.
impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.major
            .cmp(&other.major)
            .then_with(|| self.minor.cmp(&other.minor))
            .then_with(|| self.patch.cmp(&other.patch))
            .then_with(
                || match (self.prerelease.is_empty(), other.prerelease.is_empty()) {
                    (true, true) => Ordering::Equal,
                    (true, false) => Ordering::Greater,
                    (false, true) => Ordering::Less,
                    (false, false) => {
                        for (left, right) in self.prerelease.iter().zip(&other.prerelease) {
                            let order = identifier_order(left, right);
                            if order != Ordering::Equal {
                                return order;
                            }
                        }
                        self.prerelease.len().cmp(&other.prerelease.len())
                    }
                },
            )
            .then_with(|| self.build.cmp(&other.build))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl fmt::Debug for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Version({:?})", self.text)
    }
}

/// `group:name:version` — the stable textual Cone identity.
///
/// Ordering is the stable display order of DESIGN 1.1: group UTF-8 bytes,
/// then name bytes, then canonical version precedence (with build
/// metadata as a final tiebreak to keep `Ord`/`Eq` consistent).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ConeCoordinate {
    group: String,
    name: String,
    version: Version,
}

impl ConeCoordinate {
    pub fn new(group: &str, name: &str, version: &str) -> Result<Self, CoordinateError> {
        validate_component("cone group", group)?;
        validate_component("cone name", name)?;
        let version = Version::parse(version)?;
        Ok(ConeCoordinate {
            group: group.to_owned(),
            name: name.to_owned(),
            version,
        })
    }

    pub fn group(&self) -> &str {
        &self.group
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> &Version {
        &self.version
    }

    /// `group:name:version`, the diagnostic spelling.
    pub fn display(&self) -> String {
        format!("{}:{}:{}", self.group, self.name, self.version.text)
    }

    /// Canonical CBOR product: `{1: group text, 2: name text,
    /// 3: version text}`.
    pub fn canonical_cbor(&self) -> Vec<u8> {
        let mut writer = CborWriter::new();
        writer.map(3);
        writer.field(1).text(&self.group);
        writer.field(2).text(&self.name);
        writer.field(3).text(&self.version.text);
        writer.into_bytes()
    }
}

impl PartialOrd for ConeCoordinate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ConeCoordinate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.group
            .cmp(&other.group)
            .then_with(|| self.name.cmp(&other.name))
            .then_with(|| self.version.cmp(&other.version))
    }
}

impl fmt::Display for ConeCoordinate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display())
    }
}

impl fmt::Debug for ConeCoordinate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ConeCoordinate({:?})", self.display())
    }
}

impl ConeIdentity {
    /// `SHA-256("scoop-cone-id-v1" || canonical(ConeCoordinate))`.
    pub fn of(coordinate: &ConeCoordinate) -> Self {
        let digest = DomainHasher::new(CONE_IDENTITY_DOMAIN)
            .field(&coordinate.canonical_cbor())
            .finish();
        ConeIdentity(digest)
    }

    /// Wraps a digest that was already validated against a coordinate.
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        ConeIdentity(crate::Digest256::from_bytes(*bytes))
    }
}

/// The persistent identity digest of one Cone: `ConeIdentity` includes
/// the version, so two versions of one source tree are distinct
/// nominal/callable identities (DESIGN section 1.1).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConeIdentity(crate::Digest256);

impl ConeIdentity {
    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }
}

impl fmt::Debug for ConeIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ConeIdentity({})", self.0)
    }
}

impl fmt::Display for ConeIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The reserved core Cone coordinate that every non-core Cone depends on
/// implicitly (DESIGN sections 1.2 and 5.4).
pub const CORE_CONE_GROUP: &str = "scoop";
pub const CORE_CONE_NAME: &str = "scoop.core";
pub const CORE_CONE_VERSION: &str = "0.1.0";

impl ConeCoordinate {
    /// The reserved `scoop:scoop.core:0.1.0` coordinate.
    pub fn reserved_core() -> Self {
        ConeCoordinate::new(CORE_CONE_GROUP, CORE_CONE_NAME, CORE_CONE_VERSION)
            .expect("reserved core coordinate is canonical")
    }

    pub fn is_reserved_core(&self) -> bool {
        *self == Self::reserved_core()
    }
}
