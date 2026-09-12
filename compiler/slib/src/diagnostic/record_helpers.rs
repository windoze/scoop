use scoop_wire::WirePath;

use crate::{
    ConeRecordValidationError, DependencyRecordValidationError, MemberStableKeyValidationError,
    SlibMemberRecordValidationError, SlibMemberRoleValidationError,
};

use super::{SlibDiagnosticRecord, SlibErrorCode, SlibPrimaryOrigin};

pub(super) fn cone_record_diagnostic(error: &ConeRecordValidationError) -> SlibDiagnosticRecord {
    let (code, field, origin) = match error {
        ConeRecordValidationError::Coordinate(_) | ConeRecordValidationError::Record(_) => (
            SlibErrorCode::IdentityInvalid,
            1,
            SlibPrimaryOrigin::Manifest,
        ),
        ConeRecordValidationError::Hash(_) => (
            SlibErrorCode::FingerprintMismatch,
            1,
            SlibPrimaryOrigin::Manifest,
        ),
        ConeRecordValidationError::Identity(mismatch) => (
            SlibErrorCode::IdentityMismatch,
            2,
            SlibPrimaryOrigin::Identity {
                kind: "cone",
                id: *mismatch.expected().as_array(),
            },
        ),
        ConeRecordValidationError::UnknownKind { .. } => (
            SlibErrorCode::WireUnknownTag,
            3,
            SlibPrimaryOrigin::Manifest,
        ),
        ConeRecordValidationError::UnknownSourceForm { .. } => (
            SlibErrorCode::WireUnknownTag,
            4,
            SlibPrimaryOrigin::Manifest,
        ),
    };
    SlibDiagnosticRecord::new(code, WirePath::root().field(6).field(field)).with_origin(origin)
}

pub(super) fn dependency_record_diagnostic(
    index: usize,
    error: &DependencyRecordValidationError,
) -> SlibDiagnosticRecord {
    let path = WirePath::root()
        .field(7)
        .index(u64::try_from(index).unwrap_or(u64::MAX));
    match error {
        DependencyRecordValidationError::Coordinate(_) => {
            SlibDiagnosticRecord::new(SlibErrorCode::ReferenceInvalid, path.field(1))
                .with_origin(SlibPrimaryOrigin::Manifest)
        }
        DependencyRecordValidationError::Hash(_) => {
            SlibDiagnosticRecord::new(SlibErrorCode::FingerprintMismatch, path.field(1))
                .with_origin(SlibPrimaryOrigin::Manifest)
        }
        DependencyRecordValidationError::Identity(mismatch) => {
            SlibDiagnosticRecord::new(SlibErrorCode::IdentityMismatch, path.field(2)).with_origin(
                SlibPrimaryOrigin::Identity {
                    kind: "cone",
                    id: *mismatch.expected().as_array(),
                },
            )
        }
    }
}

pub(super) fn member_record_diagnostic(
    index: usize,
    error: &SlibMemberRecordValidationError,
) -> SlibDiagnosticRecord {
    let path = WirePath::root()
        .field(8)
        .index(u64::try_from(index).unwrap_or(u64::MAX));
    let code = match error {
        SlibMemberRecordValidationError::StableKey(MemberStableKeyValidationError::Capability(
            _,
        )) => SlibErrorCode::CapabilityInvalid,
        SlibMemberRecordValidationError::StableKey(MemberStableKeyValidationError::LogicalKey(
            _,
        ))
        | SlibMemberRecordValidationError::KeyRole(_) => SlibErrorCode::DirectoryMismatch,
        SlibMemberRecordValidationError::Role(
            SlibMemberRoleValidationError::UnsupportedWireSchema { .. },
        ) => SlibErrorCode::CompatibilitySchema,
        SlibMemberRecordValidationError::Role(SlibMemberRoleValidationError::Capability(_)) => {
            SlibErrorCode::CapabilityInvalid
        }
        SlibMemberRecordValidationError::Role(
            SlibMemberRoleValidationError::TargetProfile(_)
            | SlibMemberRoleValidationError::ObjectFormat(_),
        ) => SlibErrorCode::CompatibilityProfile,
        SlibMemberRecordValidationError::Role(
            SlibMemberRoleValidationError::InvalidExtensionRequirement { .. },
        ) => SlibErrorCode::CapabilityInvalid,
        SlibMemberRecordValidationError::Hash(_) => SlibErrorCode::FingerprintMismatch,
        SlibMemberRecordValidationError::IdMismatch { expected, .. } => {
            return SlibDiagnosticRecord::new(SlibErrorCode::IdentityMismatch, path.field(1))
                .with_origin(SlibPrimaryOrigin::Member(*expected));
        }
    };
    SlibDiagnosticRecord::new(code, path).with_origin(SlibPrimaryOrigin::Manifest)
}
