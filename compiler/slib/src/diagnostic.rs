use std::cmp::Ordering;
use std::fmt;

use scoop_identity::{CapabilityId, ConeIdentity};
use scoop_wire::{ResourceKind, WireError, WireErrorKind, WirePath};

use crate::{
    ArchiveMemberOrdinal, ArchiveReadError, BootstrapManifestError,
    BootstrapManifestValidationError, CompatibilitySchemaKind, CompatibilityValidationError,
    GraphValidationError, IdentityFoundationDecodeError, MetadataLocation, MetadataReadError,
    MetadataSectionError, MetadataSectionValidationError, ProducerRecordError, SchemaKind,
    SemanticFingerprintError, SemanticFingerprintValidationError, SlibMemberId, SlibReadError,
};

/// Closed stable diagnostic codes for the M23-2 `.slib` reader and validators.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SlibErrorCode {
    ContainerBadMagic,
    ContainerTruncated,
    ContainerNonCanonical,
    ContainerInvalidLength,
    WireUnexpectedEnd,
    WireWrongType,
    WireIndefiniteLength,
    WireNonCanonical,
    WireTrailingData,
    WireInvalidField,
    WireUnknownTag,
    WireInvalidLength,
    WireIntegerOutOfRange,
    LimitExceeded,
    LimitAllocation,
    DirectoryMismatch,
    DirectoryNonCanonicalOrder,
    CapabilityInvalid,
    CapabilityUnsupported,
    CapabilityNativeBoundaryClosureRequired,
    CompatibilitySchema,
    CompatibilityMangling,
    CompatibilityProfile,
    IdentityMismatch,
    IdentityDuplicate,
    IdentityMissing,
    IdentityCycle,
    IdentityInvalid,
    ReferenceMissing,
    ReferenceFutureLayer,
    ReferenceInvalid,
    BridgeMismatch,
    FingerprintMismatch,
    SessionConflict,
}

impl SlibErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ContainerBadMagic => "SLIB_CONTAINER_BAD_MAGIC",
            Self::ContainerTruncated => "SLIB_CONTAINER_TRUNCATED",
            Self::ContainerNonCanonical => "SLIB_CONTAINER_NON_CANONICAL",
            Self::ContainerInvalidLength => "SLIB_CONTAINER_INVALID_LENGTH",
            Self::WireUnexpectedEnd => "SLIB_WIRE_UNEXPECTED_END",
            Self::WireWrongType => "SLIB_WIRE_WRONG_TYPE",
            Self::WireIndefiniteLength => "SLIB_WIRE_INDEFINITE_LENGTH",
            Self::WireNonCanonical => "SLIB_WIRE_NON_CANONICAL",
            Self::WireTrailingData => "SLIB_WIRE_TRAILING_DATA",
            Self::WireInvalidField => "SLIB_WIRE_INVALID_FIELD",
            Self::WireUnknownTag => "SLIB_WIRE_UNKNOWN_TAG",
            Self::WireInvalidLength => "SLIB_WIRE_INVALID_LENGTH",
            Self::WireIntegerOutOfRange => "SLIB_WIRE_INTEGER_OUT_OF_RANGE",
            Self::LimitExceeded => "SLIB_LIMIT_EXCEEDED",
            Self::LimitAllocation => "SLIB_LIMIT_ALLOCATION",
            Self::DirectoryMismatch => "SLIB_DIRECTORY_MISMATCH",
            Self::DirectoryNonCanonicalOrder => "SLIB_DIRECTORY_NON_CANONICAL_ORDER",
            Self::CapabilityInvalid => "SLIB_CAPABILITY_INVALID",
            Self::CapabilityUnsupported => "SLIB_CAPABILITY_UNSUPPORTED",
            Self::CapabilityNativeBoundaryClosureRequired => {
                "SLIB_CAPABILITY_NATIVE_BOUNDARY_CLOSURE_REQUIRED"
            }
            Self::CompatibilitySchema => "SLIB_COMPAT_SCHEMA",
            Self::CompatibilityMangling => "SLIB_COMPAT_MANGLING",
            Self::CompatibilityProfile => "SLIB_COMPAT_PROFILE",
            Self::IdentityMismatch => "SLIB_IDENTITY_MISMATCH",
            Self::IdentityDuplicate => "SLIB_IDENTITY_DUPLICATE",
            Self::IdentityMissing => "SLIB_IDENTITY_MISSING",
            Self::IdentityCycle => "SLIB_IDENTITY_CYCLE",
            Self::IdentityInvalid => "SLIB_IDENTITY_INVALID",
            Self::ReferenceMissing => "SLIB_REFERENCE_MISSING",
            Self::ReferenceFutureLayer => "SLIB_REFERENCE_FUTURE_LAYER",
            Self::ReferenceInvalid => "SLIB_REFERENCE_INVALID",
            Self::BridgeMismatch => "SLIB_BRIDGE_MISMATCH",
            Self::FingerprintMismatch => "SLIB_FINGERPRINT_MISMATCH",
            Self::SessionConflict => "SLIB_SESSION_CONFLICT",
        }
    }
}

impl fmt::Display for SlibErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Stable origin of the primary failing value when one is already trusted.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SlibPrimaryOrigin {
    Container,
    Manifest,
    Metadata(MetadataLocation),
    Member(SlibMemberId),
    Cone(ConeIdentity),
    Capability {
        location: Option<MetadataLocation>,
        capability: CapabilityId,
    },
    Identity {
        kind: &'static str,
        id: [u8; 32],
    },
}

/// Resource categories that can fail before or inside logical decoding.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SlibResourceKind {
    ArchiveBytes,
    ManifestBytes,
    ArchiveMembers,
    ProducerBytes,
    MetadataPayloadBytes,
    Decode(ResourceKind),
}

/// Stable resource failure payload. Logical limit failures always retain the
/// resource, configured limit, and observed value.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SlibResourceFailure {
    Limit {
        resource: SlibResourceKind,
        limit: u64,
        observed: u64,
    },
    Allocation {
        requested_logical_bytes: u64,
        requested_slots: u64,
    },
}

/// Renderer-independent projection of one typed `.slib` failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlibDiagnosticRecord {
    code: SlibErrorCode,
    path: WirePath,
    byte_offset: Option<u64>,
    primary_origin: Option<SlibPrimaryOrigin>,
    resource: Option<SlibResourceFailure>,
}

impl Ord for SlibDiagnosticRecord {
    fn cmp(&self, other: &Self) -> Ordering {
        self.primary_origin
            .cmp(&other.primary_origin)
            .then_with(|| self.path.cmp(&other.path))
            .then_with(|| self.code.cmp(&other.code))
            .then_with(|| self.byte_offset.cmp(&other.byte_offset))
            .then_with(|| self.resource.cmp(&other.resource))
    }
}

impl PartialOrd for SlibDiagnosticRecord {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl SlibDiagnosticRecord {
    pub const fn code(&self) -> SlibErrorCode {
        self.code
    }

    pub const fn path(&self) -> &WirePath {
        &self.path
    }

    pub const fn byte_offset(&self) -> Option<u64> {
        self.byte_offset
    }

    pub const fn primary_origin(&self) -> Option<&SlibPrimaryOrigin> {
        self.primary_origin.as_ref()
    }

    pub const fn resource(&self) -> Option<SlibResourceFailure> {
        self.resource
    }

    fn new(code: SlibErrorCode, path: WirePath) -> Self {
        Self {
            code,
            path,
            byte_offset: None,
            primary_origin: None,
            resource: None,
        }
    }

    fn with_origin(mut self, origin: SlibPrimaryOrigin) -> Self {
        self.primary_origin = Some(origin);
        self
    }
}

/// Produces stable machine-readable diagnostics without parsing `Display`.
pub trait SlibDiagnostic {
    fn diagnostic(&self) -> SlibDiagnosticRecord;
}

mod compile;
mod record_helpers;
use record_helpers::{
    cone_record_diagnostic, dependency_record_diagnostic, member_record_diagnostic,
};

pub(super) fn root_diagnostic(
    code: SlibErrorCode,
    origin: SlibPrimaryOrigin,
) -> SlibDiagnosticRecord {
    SlibDiagnosticRecord::new(code, WirePath::root()).with_origin(origin)
}

#[cfg(test)]
mod tests;

impl SlibDiagnostic for WireError {
    fn diagnostic(&self) -> SlibDiagnosticRecord {
        let (code, resource) = match self.kind() {
            WireErrorKind::UnexpectedEnd => (SlibErrorCode::WireUnexpectedEnd, None),
            WireErrorKind::WrongType { .. } => (SlibErrorCode::WireWrongType, None),
            WireErrorKind::IndefiniteLength { .. } => (SlibErrorCode::WireIndefiniteLength, None),
            WireErrorKind::NonCanonicalCbor => (SlibErrorCode::WireNonCanonical, None),
            WireErrorKind::TrailingData => (SlibErrorCode::WireTrailingData, None),
            WireErrorKind::DuplicateField { .. }
            | WireErrorKind::MissingField { .. }
            | WireErrorKind::ExtraField { .. }
            | WireErrorKind::UnexpectedField { .. } => (SlibErrorCode::WireInvalidField, None),
            WireErrorKind::UnknownTag { .. } => (SlibErrorCode::WireUnknownTag, None),
            WireErrorKind::InvalidLength { .. } => (SlibErrorCode::WireInvalidLength, None),
            WireErrorKind::IntegerOutOfRange => (SlibErrorCode::WireIntegerOutOfRange, None),
            WireErrorKind::LimitExceeded {
                resource,
                limit,
                observed,
            } => (
                SlibErrorCode::LimitExceeded,
                Some(SlibResourceFailure::Limit {
                    resource: SlibResourceKind::Decode(*resource),
                    limit: *limit,
                    observed: *observed,
                }),
            ),
            WireErrorKind::ResourceAllocation {
                requested_logical_bytes,
                requested_slots,
            } => (
                SlibErrorCode::LimitAllocation,
                Some(SlibResourceFailure::Allocation {
                    requested_logical_bytes: *requested_logical_bytes,
                    requested_slots: *requested_slots,
                }),
            ),
        };
        SlibDiagnosticRecord {
            code,
            path: self.path().clone(),
            byte_offset: self.byte_offset(),
            primary_origin: None,
            resource,
        }
    }
}

impl SlibDiagnostic for ArchiveReadError {
    fn diagnostic(&self) -> SlibDiagnosticRecord {
        match self {
            Self::ArchiveTooLarge { actual } => limit_diagnostic(
                SlibResourceKind::ArchiveBytes,
                2_147_483_648,
                *actual,
                SlibPrimaryOrigin::Container,
            ),
            Self::ManifestTooLarge { actual } => limit_diagnostic(
                SlibResourceKind::ManifestBytes,
                67_108_864,
                *actual,
                SlibPrimaryOrigin::Manifest,
            ),
            Self::TooManyMembers { actual } => limit_diagnostic(
                SlibResourceKind::ArchiveMembers,
                65_536,
                u64::try_from(*actual).unwrap_or(u64::MAX),
                SlibPrimaryOrigin::Container,
            ),
            Self::BadMagic => root_diagnostic(
                SlibErrorCode::ContainerBadMagic,
                SlibPrimaryOrigin::Container,
            ),
            Self::TruncatedHeader { member } | Self::TruncatedPayload { member } => {
                archive_member_diagnostic(SlibErrorCode::ContainerTruncated, *member)
            }
            Self::NonCanonicalHeader { member } | Self::InvalidPadding { member } => {
                archive_member_diagnostic(SlibErrorCode::ContainerNonCanonical, *member)
            }
            Self::InvalidSize { member } | Self::MemberLengthMismatch { member, .. } => {
                archive_member_diagnostic(SlibErrorCode::ContainerInvalidLength, *member)
            }
            Self::PredictedLengthMismatch { .. } | Self::LengthOverflow => root_diagnostic(
                SlibErrorCode::ContainerInvalidLength,
                SlibPrimaryOrigin::Container,
            ),
            Self::NonIncreasingDirectory { second_index, .. } => SlibDiagnosticRecord::new(
                SlibErrorCode::DirectoryNonCanonicalOrder,
                WirePath::root()
                    .field(8)
                    .index(u64::try_from(*second_index).unwrap_or(u64::MAX)),
            )
            .with_origin(SlibPrimaryOrigin::Manifest),
            Self::MemberDigestMismatch { id } => SlibDiagnosticRecord::new(
                SlibErrorCode::FingerprintMismatch,
                WirePath::root().field(8).key("slib-member", *id.as_array()),
            )
            .with_origin(SlibPrimaryOrigin::Member(*id)),
            Self::Budget(error) => error.diagnostic().with_origin(SlibPrimaryOrigin::Container),
        }
    }
}

impl SlibDiagnostic for SlibReadError {
    fn diagnostic(&self) -> SlibDiagnosticRecord {
        match self {
            Self::Container(error) | Self::Directory(error) => error.diagnostic(),
            Self::CanonicalWire(error) => {
                error.diagnostic().with_origin(SlibPrimaryOrigin::Manifest)
            }
            Self::Manifest(error) => error.diagnostic(),
        }
    }
}

impl SlibDiagnostic for BootstrapManifestValidationError {
    fn diagnostic(&self) -> SlibDiagnosticRecord {
        let manifest = SlibPrimaryOrigin::Manifest;
        match self {
            Self::BadMagic => SlibDiagnosticRecord::new(
                SlibErrorCode::WireNonCanonical,
                WirePath::root().field(1),
            )
            .with_origin(manifest),
            Self::UnsupportedSchema { kind, .. } => SlibDiagnosticRecord::new(
                SlibErrorCode::CompatibilitySchema,
                WirePath::root().field(match kind {
                    SchemaKind::Manifest => 2,
                    SchemaKind::Container => 3,
                }),
            )
            .with_origin(manifest),
            Self::Producer(ProducerRecordError::TooLong { actual }) => limit_diagnostic_at(
                SlibResourceKind::ProducerBytes,
                255,
                u64::try_from(*actual).unwrap_or(u64::MAX),
                WirePath::root().field(4).field(1),
                manifest,
            ),
            Self::Compatibility(error) => compatibility_diagnostic(error),
            Self::Cone(error) => cone_record_diagnostic(error),
            Self::Dependency { index, error } => dependency_record_diagnostic(*index, error),
            Self::NonIncreasingDependency { second_index, .. } => SlibDiagnosticRecord::new(
                SlibErrorCode::ReferenceInvalid,
                WirePath::root()
                    .field(7)
                    .index(u64::try_from(*second_index).unwrap_or(u64::MAX)),
            )
            .with_origin(manifest),
            Self::Member { index, error } => member_record_diagnostic(*index, error),
            Self::NonIncreasingMember { second_index, .. } => SlibDiagnosticRecord::new(
                SlibErrorCode::DirectoryNonCanonicalOrder,
                WirePath::root()
                    .field(8)
                    .index(u64::try_from(*second_index).unwrap_or(u64::MAX)),
            )
            .with_origin(manifest),
            Self::SemanticFingerprints(error) => SlibDiagnosticRecord::new(
                SlibErrorCode::FingerprintMismatch,
                WirePath::root().field(9).field(match error {
                    SemanticFingerprintValidationError::CodeMustBeAvailable
                    | SemanticFingerprintValidationError::CodeMustBeUnavailable => 4,
                    SemanticFingerprintValidationError::RuntimeImageMustBeAvailable
                    | SemanticFingerprintValidationError::RuntimeImageMustBeUnavailable => 5,
                }),
            )
            .with_origin(manifest),
            Self::Section { index, .. }
            | Self::NonIncreasingSection {
                second_index: index,
                ..
            } => SlibDiagnosticRecord::new(
                SlibErrorCode::CapabilityInvalid,
                WirePath::root()
                    .field(10)
                    .index(u64::try_from(*index).unwrap_or(u64::MAX)),
            )
            .with_origin(manifest),
            Self::Manifest(error) => manifest_build_diagnostic(error),
            Self::ArtifactFingerprintMismatch { .. } => SlibDiagnosticRecord::new(
                SlibErrorCode::FingerprintMismatch,
                WirePath::root().field(11),
            )
            .with_origin(manifest),
            Self::Budget(error) => error.diagnostic().with_origin(manifest),
        }
    }
}

impl SlibDiagnostic for MetadataReadError {
    fn diagnostic(&self) -> SlibDiagnosticRecord {
        match self {
            Self::CanonicalWire(error) | Self::Budget(error) => error.diagnostic(),
            Self::BadMagic { expected } => SlibDiagnosticRecord::new(
                SlibErrorCode::WireNonCanonical,
                WirePath::root().field(1),
            )
            .with_origin(SlibPrimaryOrigin::Metadata(*expected)),
            Self::UnsupportedSchema { .. } => SlibDiagnosticRecord::new(
                SlibErrorCode::CompatibilitySchema,
                WirePath::root().field(2),
            ),
            Self::Section { index, error } => {
                metadata_section_diagnostic(error).with_metadata_index(*index)
            }
            Self::NonIncreasingSection { second_index, .. } => SlibDiagnosticRecord::new(
                SlibErrorCode::CapabilityInvalid,
                WirePath::root()
                    .field(3)
                    .index(u64::try_from(*second_index).unwrap_or(u64::MAX)),
            ),
            Self::Allocation => {
                SlibDiagnosticRecord::new(SlibErrorCode::LimitAllocation, WirePath::root().field(3))
            }
        }
    }
}

impl SlibDiagnosticRecord {
    fn with_metadata_index(mut self, index: usize) -> Self {
        self.path = WirePath::root()
            .field(3)
            .index(u64::try_from(index).unwrap_or(u64::MAX));
        self
    }
}

impl SlibDiagnostic for GraphValidationError {
    fn diagnostic(&self) -> SlibDiagnosticRecord {
        match self {
            Self::SelfDependency { cone } => SlibDiagnosticRecord::new(
                SlibErrorCode::ReferenceInvalid,
                WirePath::root().field(7),
            )
            .with_origin(SlibPrimaryOrigin::Cone(*cone)),
            Self::SingleFileDependency => SlibDiagnosticRecord::new(
                SlibErrorCode::ReferenceInvalid,
                WirePath::root().field(7),
            ),
        }
    }
}

impl SlibDiagnostic for IdentityFoundationDecodeError {
    fn diagnostic(&self) -> SlibDiagnosticRecord {
        match self {
            Self::MissingMetadataMember { location } => metadata_diagnostic(
                SlibErrorCode::DirectoryMismatch,
                *location,
                WirePath::root().field(8),
            ),
            Self::MissingMemberPayload {
                location: _,
                member,
            } => SlibDiagnosticRecord::new(
                SlibErrorCode::DirectoryMismatch,
                WirePath::root()
                    .field(8)
                    .key("slib-member", *member.as_array()),
            )
            .with_origin(SlibPrimaryOrigin::Member(*member)),
            Self::OuterEnvelope { location, source } => source
                .diagnostic()
                .with_origin(SlibPrimaryOrigin::Metadata(*location)),
            Self::MissingFoundationSection {
                location,
                capability,
            }
            | Self::UnknownCompileCapability {
                location: Some(location),
                capability,
                ..
            } => SlibDiagnosticRecord::new(
                SlibErrorCode::CapabilityUnsupported,
                WirePath::root().field(3),
            )
            .with_origin(SlibPrimaryOrigin::Capability {
                location: Some(*location),
                capability: capability.clone(),
            }),
            Self::UnknownCompileCapability {
                location: None,
                index,
                capability,
            } => SlibDiagnosticRecord::new(
                SlibErrorCode::CapabilityUnsupported,
                WirePath::root()
                    .field(10)
                    .index(u64::try_from(*index).unwrap_or(u64::MAX)),
            )
            .with_origin(SlibPrimaryOrigin::Capability {
                location: None,
                capability: capability.clone(),
            }),
            Self::InnerFoundation { location, source } => source
                .diagnostic()
                .with_origin(SlibPrimaryOrigin::Metadata(*location)),
            Self::SemanticFingerprints(SemanticFingerprintError::Resource(error)) => {
                error.diagnostic().with_origin(SlibPrimaryOrigin::Manifest)
            }
            Self::SemanticFingerprints(_) => root_diagnostic(
                SlibErrorCode::FingerprintMismatch,
                SlibPrimaryOrigin::Manifest,
            ),
            Self::SemanticFingerprintMismatch { location, .. } => metadata_diagnostic(
                SlibErrorCode::FingerprintMismatch,
                *location,
                WirePath::root()
                    .field(9)
                    .field(metadata_fingerprint_field(*location)),
            ),
        }
    }
}

fn compatibility_diagnostic(error: &CompatibilityValidationError) -> SlibDiagnosticRecord {
    let (code, field) = match error {
        CompatibilityValidationError::UnsupportedSchema { kind, .. } => (
            SlibErrorCode::CompatibilitySchema,
            match kind {
                CompatibilitySchemaKind::Identity => 3,
                CompatibilitySchemaKind::Hir => 9,
                CompatibilitySchemaKind::Mir => 10,
                CompatibilitySchemaKind::Lir => 11,
            },
        ),
        CompatibilityValidationError::ManglingSchema { .. } => {
            (SlibErrorCode::CompatibilityMangling, 4)
        }
        CompatibilityValidationError::Capability(_) => (SlibErrorCode::CapabilityInvalid, 5),
        CompatibilityValidationError::TargetProfile(_)
        | CompatibilityValidationError::BackendProfile(_) => {
            (SlibErrorCode::CompatibilityProfile, 5)
        }
        CompatibilityValidationError::ArtifactProfile(_) => {
            (SlibErrorCode::CompatibilityProfile, 13)
        }
        CompatibilityValidationError::FingerprintMismatch { kind, .. } => (
            SlibErrorCode::FingerprintMismatch,
            match kind {
                crate::CompatibilityFingerprintKind::LanguageAbi => 1,
                crate::CompatibilityFingerprintKind::RuntimeAbi => 2,
                crate::CompatibilityFingerprintKind::TargetProfile => 6,
                crate::CompatibilityFingerprintKind::BackendProfile => 8,
                crate::CompatibilityFingerprintKind::CompositeIdentityAbi => 12,
                crate::CompatibilityFingerprintKind::ArtifactProfile => 14,
            },
        ),
        CompatibilityValidationError::Hash(_) => (SlibErrorCode::FingerprintMismatch, 5),
        CompatibilityValidationError::Resource(error) => {
            return error.diagnostic().with_origin(SlibPrimaryOrigin::Manifest);
        }
    };
    SlibDiagnosticRecord::new(code, WirePath::root().field(5).field(field))
        .with_origin(SlibPrimaryOrigin::Manifest)
}

fn manifest_build_diagnostic(error: &BootstrapManifestError) -> SlibDiagnosticRecord {
    let (code, path, origin) = match error {
        BootstrapManifestError::DuplicateDependency { identity } => (
            SlibErrorCode::ReferenceInvalid,
            WirePath::root().field(7),
            Some(SlibPrimaryOrigin::Cone(*identity)),
        ),
        BootstrapManifestError::DuplicateMember { id } => (
            SlibErrorCode::DirectoryMismatch,
            WirePath::root().field(8).key("slib-member", *id.as_array()),
            Some(SlibPrimaryOrigin::Member(*id)),
        ),
        BootstrapManifestError::TooManyMembers { actual } => {
            return limit_diagnostic_at(
                SlibResourceKind::ArchiveMembers,
                65_536,
                u64::try_from(*actual).unwrap_or(u64::MAX),
                WirePath::root().field(8),
                SlibPrimaryOrigin::Manifest,
            );
        }
        BootstrapManifestError::MissingMetadata { .. } => (
            SlibErrorCode::DirectoryMismatch,
            WirePath::root().field(8),
            Some(SlibPrimaryOrigin::Manifest),
        ),
        BootstrapManifestError::DuplicateSection { capability } => (
            SlibErrorCode::CapabilityInvalid,
            WirePath::root().field(10),
            Some(SlibPrimaryOrigin::Capability {
                location: None,
                capability: capability.clone(),
            }),
        ),
        BootstrapManifestError::Hash(_) => (
            SlibErrorCode::FingerprintMismatch,
            WirePath::root().field(11),
            Some(SlibPrimaryOrigin::Manifest),
        ),
        BootstrapManifestError::Resource(error) => {
            return error.diagnostic().with_origin(SlibPrimaryOrigin::Manifest);
        }
    };
    let mut diagnostic = SlibDiagnosticRecord::new(code, path);
    diagnostic.primary_origin = origin;
    diagnostic
}

fn metadata_section_diagnostic(error: &MetadataSectionValidationError) -> SlibDiagnosticRecord {
    match error {
        MetadataSectionValidationError::Capability(_) => {
            SlibDiagnosticRecord::new(SlibErrorCode::CapabilityInvalid, WirePath::root())
        }
        MetadataSectionValidationError::Section(error) => match error {
            MetadataSectionError::PayloadTooLarge { actual } => SlibDiagnosticRecord {
                code: SlibErrorCode::LimitExceeded,
                path: WirePath::root(),
                byte_offset: None,
                primary_origin: None,
                resource: Some(SlibResourceFailure::Limit {
                    resource: SlibResourceKind::MetadataPayloadBytes,
                    limit: 268_435_456,
                    observed: *actual,
                }),
            },
            MetadataSectionError::InvalidPurpose { .. }
            | MetadataSectionError::KnownCapabilityWrongLocation { .. }
            | MetadataSectionError::KnownCapabilityWrongPurpose { .. } => {
                SlibDiagnosticRecord::new(SlibErrorCode::CapabilityInvalid, WirePath::root())
            }
        },
    }
}

fn metadata_diagnostic(
    code: SlibErrorCode,
    location: MetadataLocation,
    path: WirePath,
) -> SlibDiagnosticRecord {
    SlibDiagnosticRecord::new(code, path).with_origin(SlibPrimaryOrigin::Metadata(location))
}

fn metadata_fingerprint_field(location: MetadataLocation) -> u32 {
    match location {
        MetadataLocation::Hir => 1,
        MetadataLocation::Mir => 2,
        MetadataLocation::Lir => 3,
    }
}

fn limit_diagnostic(
    resource: SlibResourceKind,
    limit: u64,
    observed: u64,
    origin: SlibPrimaryOrigin,
) -> SlibDiagnosticRecord {
    limit_diagnostic_at(resource, limit, observed, WirePath::root(), origin)
}

fn limit_diagnostic_at(
    resource: SlibResourceKind,
    limit: u64,
    observed: u64,
    path: WirePath,
    origin: SlibPrimaryOrigin,
) -> SlibDiagnosticRecord {
    SlibDiagnosticRecord {
        code: SlibErrorCode::LimitExceeded,
        path,
        byte_offset: None,
        primary_origin: Some(origin),
        resource: Some(SlibResourceFailure::Limit {
            resource,
            limit,
            observed,
        }),
    }
}

fn archive_member_diagnostic(
    code: SlibErrorCode,
    member: ArchiveMemberOrdinal,
) -> SlibDiagnosticRecord {
    let (path, origin) = match member {
        ArchiveMemberOrdinal::Manifest => (WirePath::root(), SlibPrimaryOrigin::Manifest),
        ArchiveMemberOrdinal::Directory(index) => (
            WirePath::root().field(8).index(u64::from(index)),
            SlibPrimaryOrigin::Container,
        ),
    };
    SlibDiagnosticRecord::new(code, path).with_origin(origin)
}
