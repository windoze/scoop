use std::cmp::Ordering;
use std::fmt;

use scoop_identity::{CapabilityId, ConeIdentity};
use scoop_wire::{WireError, WireErrorKind, WirePath};

use crate::{
    ArchiveMemberOrdinal, ArchiveReadError, BootstrapManifestError,
    BootstrapManifestValidationError, CompatibilitySchemaKind, CompatibilityValidationError,
    GraphValidationError, MetadataLocation, MetadataReadError, MetadataSectionError,
    MetadataSectionValidationError, ProducerRecordError, SchemaKind,
    SemanticFingerprintValidationError, SlibMemberId, SlibReadError,
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

    Allocation,
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

            Self::Allocation => "SLIB_ALLOCATION",
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

/// Renderer-independent projection of one typed `.slib` failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlibDiagnosticRecord {
    code: SlibErrorCode,
    path: WirePath,
    byte_offset: Option<u64>,
    primary_origin: Option<SlibPrimaryOrigin>,
}

impl Ord for SlibDiagnosticRecord {
    fn cmp(&self, other: &Self) -> Ordering {
        self.primary_origin
            .cmp(&other.primary_origin)
            .then_with(|| self.path.cmp(&other.path))
            .then_with(|| self.code.cmp(&other.code))
            .then_with(|| self.byte_offset.cmp(&other.byte_offset))
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

    fn new(code: SlibErrorCode, path: WirePath) -> Self {
        Self {
            code,
            path,
            byte_offset: None,
            primary_origin: None,
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
mod location;
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
        let code = match self.kind() {
            WireErrorKind::UnexpectedEnd => SlibErrorCode::WireUnexpectedEnd,
            WireErrorKind::WrongType { .. } => SlibErrorCode::WireWrongType,
            WireErrorKind::IndefiniteLength { .. } => SlibErrorCode::WireIndefiniteLength,
            WireErrorKind::NonCanonicalCbor => SlibErrorCode::WireNonCanonical,
            WireErrorKind::TrailingData => SlibErrorCode::WireTrailingData,
            WireErrorKind::DuplicateField { .. }
            | WireErrorKind::MissingField { .. }
            | WireErrorKind::ExtraField { .. }
            | WireErrorKind::UnexpectedField { .. } => SlibErrorCode::WireInvalidField,
            WireErrorKind::UnknownTag { .. } => SlibErrorCode::WireUnknownTag,
            WireErrorKind::InvalidLength { .. } => SlibErrorCode::WireInvalidLength,
            WireErrorKind::IntegerOutOfRange => SlibErrorCode::WireIntegerOutOfRange,

            WireErrorKind::Allocation => SlibErrorCode::Allocation,
        };
        SlibDiagnosticRecord {
            code,
            path: self.path().clone(),
            byte_offset: self.byte_offset(),
            primary_origin: None,
        }
    }
}

impl SlibDiagnostic for ArchiveReadError {
    fn diagnostic(&self) -> SlibDiagnosticRecord {
        match self {
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
            Self::Allocation(error) => error.diagnostic().with_origin(SlibPrimaryOrigin::Container),
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
            Self::Producer(ProducerRecordError::TooLong { .. }) => SlibDiagnosticRecord::new(
                SlibErrorCode::WireInvalidLength,
                WirePath::root().field(4).field(1),
            )
            .with_origin(manifest),
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
                    SemanticFingerprintValidationError::CodeMustBeAvailable => 4,
                    SemanticFingerprintValidationError::RuntimeImageMustBeAvailable => 5,
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
            Self::Wire(error) => error.diagnostic().with_origin(manifest),
        }
    }
}

impl SlibDiagnostic for MetadataReadError {
    fn diagnostic(&self) -> SlibDiagnosticRecord {
        match self {
            Self::CanonicalWire(error) => error.diagnostic(),
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
                SlibDiagnosticRecord::new(SlibErrorCode::Allocation, WirePath::root().field(3))
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
            MetadataSectionError::InvalidPurpose { .. }
            | MetadataSectionError::KnownCapabilityWrongLocation { .. }
            | MetadataSectionError::KnownCapabilityWrongPurpose { .. } => {
                SlibDiagnosticRecord::new(SlibErrorCode::CapabilityInvalid, WirePath::root())
            }
        },
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
