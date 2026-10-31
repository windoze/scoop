use scoop_wire::WirePath;

use crate::{
    ConeRecordValidationError, DependencyRecordValidationError, SlibMemberRoleValidationError,
};

use super::{SlibDiagnostic, SlibDiagnosticRecord, SlibErrorCode, SlibPrimaryOrigin};

pub(super) fn cone_record_diagnostic(error: &ConeRecordValidationError) -> SlibDiagnosticRecord {
    let (code, field, origin) = match error {
        ConeRecordValidationError::Record(crate::ConeRecordError::WindowsSubsystemOnLibrary) => (
            SlibErrorCode::ReferenceInvalid,
            5,
            SlibPrimaryOrigin::Manifest,
        ),
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
        ConeRecordValidationError::UnknownWindowsSubsystem { .. } => (
            SlibErrorCode::WireUnknownTag,
            5,
            SlibPrimaryOrigin::Manifest,
        ),
        ConeRecordValidationError::Resource(error) => {
            return error.diagnostic().with_origin(SlibPrimaryOrigin::Manifest);
        }
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
        DependencyRecordValidationError::Resource(error) => {
            error.diagnostic().with_origin(SlibPrimaryOrigin::Manifest)
        }
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
    error: &SlibMemberRoleValidationError,
) -> SlibDiagnosticRecord {
    let path = WirePath::root()
        .field(8)
        .index(u64::try_from(index).unwrap_or(u64::MAX))
        .field(3);
    let code = match error {
        SlibMemberRoleValidationError::UnsupportedWireSchema { .. } => {
            SlibErrorCode::CompatibilitySchema
        }
        SlibMemberRoleValidationError::Capability(_)
        | SlibMemberRoleValidationError::InvalidExtensionRequirement { .. } => {
            SlibErrorCode::CapabilityInvalid
        }
        SlibMemberRoleValidationError::NativeSource(_) => SlibErrorCode::ReferenceInvalid,
    };
    SlibDiagnosticRecord::new(code, path).with_origin(SlibPrimaryOrigin::Manifest)
}
