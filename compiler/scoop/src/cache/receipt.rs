use std::cmp::Ordering;
use std::fmt;

use scoop_identity::{ArtifactCapabilityProfileId, CapabilityIdError, DecodedCapabilityId};
use scoop_lir::{TargetProfileId, ValidatedLirTargetSelection};
use scoop_protocol::{DiagnosticOriginV1, DiagnosticSeverityV1, StructuredDiagnosticV1};
use scoop_slib::{
    ArtifactFingerprint, ConeRecord, ConeRecordValidationError, DecodedConeRecord,
    DecodedDependencyRecord, DependencyRecord, DependencyRecordValidationError,
};
use scoop_wire::{
    Decoder, Digest256, Encoder, HashError, WireDecode, WireEncode, WireError, decode_canonical,
    domain_separated_cbor_hash, encode,
};

use super::ConeCompileCacheKeyV1;
use crate::PairedCompilerFingerprintV1;
use crate::discovery::compare_coordinates;

const RECEIPT_FINGERPRINT_DOMAIN: &str = "scoop-cache-receipt-v1";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CacheReceiptFingerprintV1([u8; 32]);

impl CacheReceiptFingerprintV1 {
    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}

impl WireEncode for CacheReceiptFingerprintV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl fmt::Display for CacheReceiptFingerprintV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(&self.0, formatter)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CacheArtifactFingerprintClaimV1([u8; 32]);

impl CacheArtifactFingerprintClaimV1 {
    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn matches(self, actual: ArtifactFingerprint) -> bool {
        &self.0 == actual.as_array()
    }
}

impl From<ArtifactFingerprint> for CacheArtifactFingerprintClaimV1 {
    fn from(value: ArtifactFingerprint) -> Self {
        Self(*value.as_array())
    }
}

impl WireEncode for CacheArtifactFingerprintClaimV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheTargetSelectionV1(ValidatedLirTargetSelection);

impl CacheTargetSelectionV1 {
    pub const fn new(selection: ValidatedLirTargetSelection) -> Self {
        Self(selection)
    }

    pub const fn selection(self) -> ValidatedLirTargetSelection {
        self.0
    }
}

impl WireEncode for CacheTargetSelectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (target, backend) = match self.0.target().id() {
            TargetProfileId::DarwinAarch64 => (1, 1),
            TargetProfileId::LinuxX86_64Gnu => (2, 2),
            TargetProfileId::LinuxX86_64Musl => (3, 2),
        };
        DecodedCacheTargetSelectionV1 { target, backend }.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheReceiptBodyV1 {
    cache_key: ConeCompileCacheKeyV1,
    artifact_fingerprint: CacheArtifactFingerprintClaimV1,
    cone: ConeRecord,
    target_selection: CacheTargetSelectionV1,
    direct_dependencies: Vec<DependencyRecord>,
    compiler: PairedCompilerFingerprintV1,
    artifact_profile: ArtifactCapabilityProfileId,
    structured_warnings: Vec<StructuredDiagnosticV1>,
}

impl CacheReceiptBodyV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cache_key: ConeCompileCacheKeyV1,
        artifact_fingerprint: ArtifactFingerprint,
        cone: ConeRecord,
        target_selection: ValidatedLirTargetSelection,
        mut direct_dependencies: Vec<DependencyRecord>,
        compiler: PairedCompilerFingerprintV1,
        artifact_profile: ArtifactCapabilityProfileId,
        structured_warnings: Vec<StructuredDiagnosticV1>,
    ) -> Result<Self, CacheReceiptValidationError> {
        direct_dependencies
            .sort_by(|left, right| compare_coordinates(left.coordinate(), right.coordinate()));
        validate_dependency_order(&direct_dependencies)?;
        let structured_warnings = canonical_warnings(structured_warnings)?;
        validate_profile(&artifact_profile)?;
        Ok(Self {
            cache_key,
            artifact_fingerprint: artifact_fingerprint.into(),
            cone,
            target_selection: CacheTargetSelectionV1::new(target_selection),
            direct_dependencies,
            compiler,
            artifact_profile,
            structured_warnings,
        })
    }

    pub const fn cache_key(&self) -> ConeCompileCacheKeyV1 {
        self.cache_key
    }

    pub const fn artifact_fingerprint(&self) -> CacheArtifactFingerprintClaimV1 {
        self.artifact_fingerprint
    }

    pub const fn cone(&self) -> &ConeRecord {
        &self.cone
    }

    pub const fn target_selection(&self) -> CacheTargetSelectionV1 {
        self.target_selection
    }

    pub fn direct_dependencies(&self) -> &[DependencyRecord] {
        &self.direct_dependencies
    }

    pub const fn compiler(&self) -> PairedCompilerFingerprintV1 {
        self.compiler
    }

    pub const fn artifact_profile(&self) -> &ArtifactCapabilityProfileId {
        &self.artifact_profile
    }

    pub fn structured_warnings(&self) -> &[StructuredDiagnosticV1] {
        &self.structured_warnings
    }

    fn fingerprint(&self) -> Result<CacheReceiptFingerprintV1, HashError> {
        domain_separated_cbor_hash(RECEIPT_FINGERPRINT_DOMAIN, self)
            .map(|digest| CacheReceiptFingerprintV1(*digest.as_array()))
    }
}

impl WireEncode for CacheReceiptBodyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(9)?;
        encoder.field(1)?;
        encoder.unsigned(1)?;
        encoder.field(2)?;
        self.cache_key.encode(encoder)?;
        encoder.field(3)?;
        self.artifact_fingerprint.encode(encoder)?;
        encoder.field(4)?;
        self.cone.encode(encoder)?;
        encoder.field(5)?;
        self.target_selection.encode(encoder)?;
        encoder.field(6)?;
        encode_array(&self.direct_dependencies, encoder)?;
        encoder.field(7)?;
        self.compiler.encode(encoder)?;
        encoder.field(8)?;
        self.artifact_profile.encode(encoder)?;
        encoder.field(9)?;
        encode_array(&self.structured_warnings, encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheReceiptV1 {
    body: CacheReceiptBodyV1,
    fingerprint: CacheReceiptFingerprintV1,
}

impl CacheReceiptV1 {
    pub fn new(body: CacheReceiptBodyV1) -> Result<Self, HashError> {
        let fingerprint = body.fingerprint()?;
        Ok(Self { body, fingerprint })
    }

    pub const fn body(&self) -> &CacheReceiptBodyV1 {
        &self.body
    }

    pub const fn fingerprint(&self) -> CacheReceiptFingerprintV1 {
        self.fingerprint
    }
}

impl WireEncode for CacheReceiptV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.body.encode(encoder)?;
        encoder.field(2)?;
        self.fingerprint.encode(encoder)
    }
}

pub fn decode_cache_receipt_v1(bytes: &[u8]) -> Result<CacheReceiptV1, CacheReceiptDecodeError> {
    let decoded: DecodedCacheReceiptV1 =
        decode_canonical(bytes).map_err(CacheReceiptDecodeError::Wire)?;
    let receipt = decoded.validate()?;
    Ok(receipt)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedCacheReceiptV1 {
    body: DecodedCacheReceiptBodyV1,
    fingerprint: Digest256,
}

impl DecodedCacheReceiptV1 {
    fn validate(self) -> Result<CacheReceiptV1, CacheReceiptDecodeError> {
        let body = self.body.validate()?;

        let receipt = CacheReceiptV1::new(body).map_err(CacheReceiptDecodeError::Hash)?;
        if receipt.fingerprint.as_array() != self.fingerprint.as_array() {
            return Err(CacheReceiptDecodeError::FingerprintMismatch {
                expected: receipt.fingerprint,
                actual: CacheReceiptFingerprintV1(*self.fingerprint.as_array()),
            });
        }
        Ok(receipt)
    }
}

impl WireEncode for DecodedCacheReceiptV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.body.encode(encoder)?;
        encoder.field(2)?;
        self.fingerprint.encode(encoder)
    }
}

impl WireDecode for DecodedCacheReceiptV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            body: decoder.field(1, DecodedCacheReceiptBodyV1::decode)?,
            fingerprint: decoder.field(2, Digest256::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedCacheReceiptBodyV1 {
    schema: u32,
    cache_key: Digest256,
    artifact_fingerprint: Digest256,
    cone: DecodedConeRecord,
    target_selection: DecodedCacheTargetSelectionV1,
    direct_dependencies: Vec<DecodedDependencyRecord>,
    compiler: DecodedPairedCompilerFingerprintV1,
    artifact_profile: DecodedCapabilityId,
    structured_warnings: Vec<StructuredDiagnosticV1>,
}

impl DecodedCacheReceiptBodyV1 {
    fn validate(self) -> Result<CacheReceiptBodyV1, CacheReceiptDecodeError> {
        if self.schema != 1 {
            return Err(CacheReceiptDecodeError::UnsupportedSchema(self.schema));
        }
        let cone = self
            .cone
            .validate()
            .map_err(|source| CacheReceiptDecodeError::Cone(Box::new(source)))?;
        let mut dependencies = Vec::with_capacity(self.direct_dependencies.len());
        for (index, dependency) in self.direct_dependencies.into_iter().enumerate() {
            dependencies.push(dependency.validate().map_err(|source| {
                CacheReceiptDecodeError::Dependency {
                    index,
                    source: Box::new(source),
                }
            })?);
        }
        validate_dependency_order(&dependencies).map_err(CacheReceiptDecodeError::Validation)?;
        validate_warning_order(&self.structured_warnings)
            .map_err(CacheReceiptDecodeError::Validation)?;
        let capability = self
            .artifact_profile
            .validate()
            .map_err(CacheReceiptDecodeError::Capability)?;
        let artifact_profile = ArtifactCapabilityProfileId::refine(capability)
            .map_err(|_| CacheReceiptDecodeError::UnknownArtifactProfile)?;
        validate_profile(&artifact_profile).map_err(CacheReceiptDecodeError::Validation)?;
        Ok(CacheReceiptBodyV1 {
            cache_key: ConeCompileCacheKeyV1::from_digest(self.cache_key),
            artifact_fingerprint: CacheArtifactFingerprintClaimV1(
                *self.artifact_fingerprint.as_array(),
            ),
            cone,
            target_selection: self
                .target_selection
                .validate()
                .map_err(CacheReceiptDecodeError::Validation)?,
            direct_dependencies: dependencies,
            compiler: self.compiler.validate(),
            artifact_profile,
            structured_warnings: self.structured_warnings,
        })
    }
}

impl WireEncode for DecodedCacheReceiptBodyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(9)?;
        encoder.field(1)?;
        encoder.unsigned(u64::from(self.schema))?;
        encoder.field(2)?;
        self.cache_key.encode(encoder)?;
        encoder.field(3)?;
        self.artifact_fingerprint.encode(encoder)?;
        encoder.field(4)?;
        self.cone.encode(encoder)?;
        encoder.field(5)?;
        self.target_selection.encode(encoder)?;
        encoder.field(6)?;
        encode_array(&self.direct_dependencies, encoder)?;
        encoder.field(7)?;
        self.compiler.encode(encoder)?;
        encoder.field(8)?;
        self.artifact_profile.encode(encoder)?;
        encoder.field(9)?;
        encode_array(&self.structured_warnings, encoder)
    }
}

impl WireDecode for DecodedCacheReceiptBodyV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(9)?;
        Ok(Self {
            schema: decoder.field(1, Decoder::u32)?,
            cache_key: decoder.field(2, Digest256::decode)?,
            artifact_fingerprint: decoder.field(3, Digest256::decode)?,
            cone: decoder.field(4, DecodedConeRecord::decode)?,
            target_selection: decoder.field(5, DecodedCacheTargetSelectionV1::decode)?,
            direct_dependencies: decoder.field(6, |decoder| {
                decoder.decode_array(|decoder, _| DecodedDependencyRecord::decode(decoder))
            })?,
            compiler: decoder.field(7, DecodedPairedCompilerFingerprintV1::decode)?,
            artifact_profile: decoder.field(8, DecodedCapabilityId::decode)?,
            structured_warnings: decoder.field(9, |decoder| {
                decoder.decode_array(|decoder, _| StructuredDiagnosticV1::decode(decoder))
            })?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DecodedCacheTargetSelectionV1 {
    target: u32,
    backend: u32,
}

impl DecodedCacheTargetSelectionV1 {
    fn validate(self) -> Result<CacheTargetSelectionV1, CacheReceiptValidationError> {
        let id = match (self.target, self.backend) {
            (1, 1) => TargetProfileId::DarwinAarch64,
            (2, 2) => TargetProfileId::LinuxX86_64Gnu,
            (3, 2) => TargetProfileId::LinuxX86_64Musl,
            _ => {
                return Err(CacheReceiptValidationError::UnsupportedTargetSelection {
                    target: self.target,
                    backend: self.backend,
                });
            }
        };
        Ok(CacheTargetSelectionV1::new(
            ValidatedLirTargetSelection::from_id(id),
        ))
    }
}

impl WireEncode for DecodedCacheTargetSelectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.unsigned(u64::from(self.target))?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.backend))
    }
}

impl WireDecode for DecodedCacheTargetSelectionV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            target: decoder.field(1, Decoder::u32)?,
            backend: decoder.field(2, Decoder::u32)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DecodedPairedCompilerFingerprintV1 {
    executable: Digest256,
    distribution: Digest256,
    build: Digest256,
}

impl DecodedPairedCompilerFingerprintV1 {
    const fn validate(self) -> PairedCompilerFingerprintV1 {
        PairedCompilerFingerprintV1::from_parts(self.executable, self.distribution, self.build)
    }
}

impl WireEncode for DecodedPairedCompilerFingerprintV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.executable.encode(encoder)?;
        encoder.field(2)?;
        self.distribution.encode(encoder)?;
        encoder.field(3)?;
        self.build.encode(encoder)
    }
}

impl WireDecode for DecodedPairedCompilerFingerprintV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            executable: decoder.field(1, Digest256::decode)?,
            distribution: decoder.field(2, Digest256::decode)?,
            build: decoder.field(3, Digest256::decode)?,
        })
    }
}

struct WarningKey<'a> {
    code: &'a str,
    origin: &'a DiagnosticOriginV1,
}

impl WireEncode for WarningKey<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.text(self.code)?;
        encoder.field(2)?;
        self.origin.encode(encoder)
    }
}

pub(crate) fn canonical_warnings(
    mut warnings: Vec<StructuredDiagnosticV1>,
) -> Result<Vec<StructuredDiagnosticV1>, CacheReceiptValidationError> {
    for warning in &warnings {
        validate_warning(warning)?;
    }
    let mut keyed = warnings
        .drain(..)
        .map(|warning| warning_key(&warning).map(|key| (key, warning)))
        .collect::<Result<Vec<_>, _>>()?;
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    validate_keyed_warnings(&keyed)?;
    Ok(keyed.into_iter().map(|(_, warning)| warning).collect())
}

pub(crate) fn validate_warning_order(
    warnings: &[StructuredDiagnosticV1],
) -> Result<(), CacheReceiptValidationError> {
    let mut keyed = Vec::with_capacity(warnings.len());
    for warning in warnings {
        validate_warning(warning)?;
        let key = warning_key(warning)?;
        keyed.push((key, warning));
    }
    for pair in keyed.windows(2) {
        match pair[0].0.cmp(&pair[1].0) {
            Ordering::Less => {}
            Ordering::Equal => return Err(warning_collision(pair[0].1, pair[1].1)),
            Ordering::Greater => return Err(CacheReceiptValidationError::WarningOrder),
        }
    }
    Ok(())
}

fn validate_keyed_warnings(
    warnings: &[(Vec<u8>, StructuredDiagnosticV1)],
) -> Result<(), CacheReceiptValidationError> {
    for pair in warnings.windows(2) {
        if pair[0].0 == pair[1].0 {
            return Err(warning_collision(&pair[0].1, &pair[1].1));
        }
    }
    Ok(())
}

fn warning_collision(
    left: &StructuredDiagnosticV1,
    right: &StructuredDiagnosticV1,
) -> CacheReceiptValidationError {
    if left == right {
        CacheReceiptValidationError::DuplicateWarningKey(left.code().to_owned())
    } else {
        CacheReceiptValidationError::ConflictingWarningKey(left.code().to_owned())
    }
}

fn warning_key(warning: &StructuredDiagnosticV1) -> Result<Vec<u8>, CacheReceiptValidationError> {
    encode(&WarningKey {
        code: warning.code(),
        origin: warning.origin(),
    })
    .map_err(|_| CacheReceiptValidationError::WarningKeyEncoding)
}

fn validate_warning(warning: &StructuredDiagnosticV1) -> Result<(), CacheReceiptValidationError> {
    if warning.severity() != DiagnosticSeverityV1::Warning {
        return Err(CacheReceiptValidationError::NonWarningDiagnostic(
            warning.code().to_owned(),
        ));
    }
    validate_cached_origin(warning.origin())?;
    for note in warning.notes() {
        validate_cached_origin(note.origin())?;
    }
    Ok(())
}

fn validate_cached_origin(origin: &DiagnosticOriginV1) -> Result<(), CacheReceiptValidationError> {
    match origin {
        DiagnosticOriginV1::None | DiagnosticOriginV1::SemanticSourceSpan { .. } => Ok(()),
        DiagnosticOriginV1::HostPathSpan { .. } | DiagnosticOriginV1::ArtifactPath { .. } => {
            Err(CacheReceiptValidationError::HostPathDiagnosticOrigin)
        }
    }
}

fn validate_dependency_order(
    dependencies: &[DependencyRecord],
) -> Result<(), CacheReceiptValidationError> {
    for pair in dependencies.windows(2) {
        match compare_coordinates(pair[0].coordinate(), pair[1].coordinate()) {
            Ordering::Less => {}
            Ordering::Equal => {
                return Err(CacheReceiptValidationError::DuplicateDependency(
                    pair[0].identity(),
                ));
            }
            Ordering::Greater => return Err(CacheReceiptValidationError::DependencyOrder),
        }
    }
    Ok(())
}

pub(crate) fn validate_profile(
    profile: &ArtifactCapabilityProfileId,
) -> Result<(), CacheReceiptValidationError> {
    if profile != &ArtifactCapabilityProfileId::cross_cone_generic() {
        return Err(CacheReceiptValidationError::UnsupportedArtifactProfile);
    }
    Ok(())
}

fn encode_array<T: WireEncode>(
    values: &[T],
    encoder: &mut Encoder,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

fn write_hex(bytes: &[u8; 32], formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}

#[derive(Debug)]
pub enum CacheReceiptValidationError {
    NonWarningDiagnostic(String),
    HostPathDiagnosticOrigin,
    WarningKeyEncoding,
    DuplicateWarningKey(String),
    ConflictingWarningKey(String),
    WarningOrder,
    DuplicateDependency(scoop_identity::ConeIdentity),
    DependencyOrder,
    UnsupportedTargetSelection { target: u32, backend: u32 },
    UnsupportedArtifactProfile,
    Resource(WireError),
}

impl fmt::Display for CacheReceiptValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonWarningDiagnostic(code) => {
                write!(
                    formatter,
                    "cache receipt diagnostic {code} is not a warning"
                )
            }
            Self::HostPathDiagnosticOrigin => {
                formatter.write_str("cache receipt warning retains a temporary or host-path origin")
            }
            Self::WarningKeyEncoding => {
                formatter.write_str("cannot encode cache receipt warning key")
            }
            Self::DuplicateWarningKey(code) => {
                write!(formatter, "cache receipt repeats warning key {code}")
            }
            Self::ConflictingWarningKey(code) => {
                write!(
                    formatter,
                    "cache receipt has conflicting warning key {code}"
                )
            }
            Self::WarningOrder => {
                formatter.write_str("cache receipt warnings are not in canonical key order")
            }
            Self::DuplicateDependency(identity) => {
                write!(formatter, "cache receipt repeats dependency {identity}")
            }
            Self::DependencyOrder => {
                formatter.write_str("cache receipt dependencies are not in canonical order")
            }
            Self::UnsupportedTargetSelection { target, backend } => write!(
                formatter,
                "unsupported cache receipt target/backend tags {target}/{backend}"
            ),
            Self::UnsupportedArtifactProfile => {
                formatter.write_str("cache receipt does not use the strong artifact profile")
            }
            Self::Resource(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CacheReceiptValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum CacheReceiptDecodeError {
    Wire(WireError),
    UnsupportedSchema(u32),
    Cone(Box<ConeRecordValidationError>),
    Dependency {
        index: usize,
        source: Box<DependencyRecordValidationError>,
    },
    Capability(CapabilityIdError),
    UnknownArtifactProfile,
    Validation(CacheReceiptValidationError),
    Hash(HashError),
    FingerprintMismatch {
        expected: CacheReceiptFingerprintV1,
        actual: CacheReceiptFingerprintV1,
    },
}

impl fmt::Display for CacheReceiptDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wire(error) => error.fmt(formatter),
            Self::UnsupportedSchema(schema) => {
                write!(formatter, "unsupported cache receipt schema {schema}")
            }
            Self::Cone(error) => write!(formatter, "invalid cache receipt Cone: {error}"),
            Self::Dependency { index, source } => {
                write!(
                    formatter,
                    "invalid cache receipt dependency {index}: {source}"
                )
            }
            Self::Capability(error) => error.fmt(formatter),
            Self::UnknownArtifactProfile => {
                formatter.write_str("cache receipt names an unknown artifact profile")
            }
            Self::Validation(error) => error.fmt(formatter),
            Self::Hash(error) => error.fmt(formatter),
            Self::FingerprintMismatch { expected, actual } => write!(
                formatter,
                "cache receipt fingerprint mismatch: expected {expected}, found {actual}"
            ),
        }
    }
}

impl std::error::Error for CacheReceiptDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Wire(error) => Some(error),
            Self::Cone(error) => Some(error),
            Self::Dependency { source, .. } => Some(source),
            Self::Capability(error) => Some(error),
            Self::Validation(error) => Some(error),
            Self::Hash(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
