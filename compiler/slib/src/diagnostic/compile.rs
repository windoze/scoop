use scoop_identity::{
    IdentityReferenceError, IdentityValidationError, SemanticIdentityImportError,
};
use scoop_wire::WirePath;

use crate::CompileCommitError;

use super::{
    SlibDiagnostic, SlibDiagnosticRecord, SlibErrorCode, SlibPrimaryOrigin, root_diagnostic,
};

impl SlibDiagnostic for IdentityValidationError {
    fn diagnostic(&self) -> SlibDiagnosticRecord {
        match self {
            Self::IdentityMismatch { kind, actual, .. } => {
                identity_diagnostic(SlibErrorCode::IdentityMismatch, kind, actual)
            }
            Self::DuplicateIdentity { kind, id } => {
                identity_diagnostic(SlibErrorCode::IdentityDuplicate, kind, id)
            }
            Self::UnregisteredIdentity { kind, id } | Self::UnresolvedIdentity { kind, id } => {
                identity_diagnostic(SlibErrorCode::IdentityMissing, kind, id)
            }
            Self::DependencyCycle { kind, id } => {
                identity_diagnostic(SlibErrorCode::IdentityCycle, kind, id)
            }
            Self::AlreadyResolved { kind, id }
            | Self::IdentityCollision { kind, id }
            | Self::InvalidRecord { kind, id, .. } => {
                identity_diagnostic(SlibErrorCode::IdentityInvalid, kind, id)
            }
            Self::Hash { .. } | Self::RegistrationClosed | Self::Poisoned => {
                SlibDiagnosticRecord::new(SlibErrorCode::IdentityInvalid, WirePath::root())
            }
            Self::Resource(error) => error.diagnostic(),
        }
    }
}

impl SlibDiagnostic for IdentityReferenceError {
    fn diagnostic(&self) -> SlibDiagnosticRecord {
        match self {
            Self::Missing { kind, id } | Self::KeyUnavailable { kind, id } => {
                identity_diagnostic(SlibErrorCode::ReferenceMissing, kind, id)
            }
            Self::FutureLayer { kind, id, .. } => {
                identity_diagnostic(SlibErrorCode::ReferenceFutureLayer, kind, id)
            }
            Self::StorageFailure => {
                SlibDiagnosticRecord::new(SlibErrorCode::WireIntegerOutOfRange, WirePath::root())
            }
        }
    }
}

impl SlibDiagnostic for CompileCommitError {
    fn diagnostic(&self) -> SlibDiagnosticRecord {
        match self {
            Self::Resource(error) => error.diagnostic(),
            Self::SemanticImport(error) => error.diagnostic(),
        }
    }
}

impl SlibDiagnostic for SemanticIdentityImportError {
    fn diagnostic(&self) -> SlibDiagnosticRecord {
        match self {
            Self::OriginConflict { origin } | Self::DuplicateBatchOrigin { origin } => {
                root_diagnostic(
                    SlibErrorCode::SessionConflict,
                    SlibPrimaryOrigin::Cone(*origin),
                )
            }
            Self::IdentityConflict { kind, id } => {
                identity_diagnostic(SlibErrorCode::SessionConflict, kind, id)
            }
            Self::MissingCanonicalKey => {
                SlibDiagnosticRecord::new(SlibErrorCode::IdentityMissing, WirePath::root())
            }
            Self::WorldIdExhausted => {
                SlibDiagnosticRecord::new(SlibErrorCode::WireIntegerOutOfRange, WirePath::root())
            }
            Self::Allocation { .. } => {
                SlibDiagnosticRecord::new(SlibErrorCode::Allocation, WirePath::root())
            }
        }
    }
}

fn identity_diagnostic(
    code: SlibErrorCode,
    kind: &'static str,
    id: &[u8; 32],
) -> SlibDiagnosticRecord {
    SlibDiagnosticRecord::new(code, WirePath::root().key(kind, *id))
        .with_origin(SlibPrimaryOrigin::Identity { kind, id: *id })
}
