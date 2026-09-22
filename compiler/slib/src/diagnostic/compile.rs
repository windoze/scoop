use scoop_hir::{HirFoundationValidationError, NativeBoundaryNominalOwner};
use scoop_identity::{
    IdentityReferenceError, IdentityValidationError, SemanticIdentityImportError,
};
use scoop_lir::LirFoundationValidationError;
use scoop_mir::MirFoundationValidationError;
use scoop_wire::WirePath;

use crate::{
    CompileCommitError, FoundationStructureValidationError, MetadataLocation,
    NativeBoundaryCompileError, NativeBoundaryTargetError,
};

use super::{
    SlibDiagnostic, SlibDiagnosticRecord, SlibErrorCode, SlibPrimaryOrigin, SlibResourceFailure,
    root_diagnostic,
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
            Self::ResourceLimit => {
                SlibDiagnosticRecord::new(SlibErrorCode::LimitExceeded, WirePath::root())
            }
        }
    }
}

impl SlibDiagnostic for FoundationStructureValidationError {
    fn diagnostic(&self) -> SlibDiagnosticRecord {
        match self {
            Self::Hir(error) => hir_structure_diagnostic(error),
            Self::Mir(error) => mir_structure_diagnostic(error),
            Self::Lir(error) => lir_structure_diagnostic(error),
        }
    }
}

impl SlibDiagnostic for NativeBoundaryCompileError {
    fn diagnostic(&self) -> SlibDiagnosticRecord {
        match self {
            Self::Identity(error) => error.diagnostic(),
            Self::Resource(error) => error.diagnostic(),
            Self::Encoding(_) => {
                SlibDiagnosticRecord::new(SlibErrorCode::BridgeMismatch, WirePath::root().field(30))
            }
            Self::ConflictingExactType { exact } => identity_diagnostic(
                SlibErrorCode::BridgeMismatch,
                "exact-type",
                exact.as_array(),
            ),
            Self::ConflictingTypeWitness { owner } => {
                native_owner_diagnostic(SlibErrorCode::BridgeMismatch, *owner)
            }
            Self::MissingCallableApplication { application } => identity_diagnostic(
                SlibErrorCode::ReferenceMissing,
                "callable-application",
                application.as_array(),
            ),
            Self::MissingInitializationUnit { unit } => identity_diagnostic(
                SlibErrorCode::ReferenceMissing,
                "initialization-unit",
                unit.as_array(),
            ),
            Self::MissingExactType { exact } => identity_diagnostic(
                SlibErrorCode::ReferenceMissing,
                "exact-type",
                exact.as_array(),
            ),
            Self::ClosureRequired { owner } => native_owner_diagnostic(
                SlibErrorCode::CapabilityNativeBoundaryClosureRequired,
                *owner,
            ),
            Self::UnrelatedDefinition { owner } => {
                native_owner_diagnostic(SlibErrorCode::BridgeMismatch, *owner)
            }
            Self::Target(error) => error.diagnostic(),
        }
    }
}

impl SlibDiagnostic for NativeBoundaryTargetError {
    fn diagnostic(&self) -> SlibDiagnosticRecord {
        match self {
            Self::MissingExactType { exact } => identity_diagnostic(
                SlibErrorCode::ReferenceMissing,
                "exact-type",
                exact.as_array(),
            ),
            Self::MissingCallbackApplication { application } => identity_diagnostic(
                SlibErrorCode::ReferenceMissing,
                "callback-application",
                application.as_array(),
            ),
            Self::MissingCallableApplication { application } => identity_diagnostic(
                SlibErrorCode::ReferenceMissing,
                "callable-application",
                application.as_array(),
            ),
            Self::MissingCallbackRegistration { registration } => identity_diagnostic(
                SlibErrorCode::ReferenceMissing,
                "callback-registration",
                registration.as_array(),
            ),
            Self::MissingInitializationUnit { unit } => identity_diagnostic(
                SlibErrorCode::ReferenceMissing,
                "initialization-unit",
                unit.as_array(),
            ),
            Self::BinderDepthOutOfRange { .. } | Self::BinderIndexOutOfRange { .. } => {
                SlibDiagnosticRecord::new(SlibErrorCode::ReferenceInvalid, WirePath::root())
            }
            _ => SlibDiagnosticRecord::new(SlibErrorCode::BridgeMismatch, WirePath::root()),
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
                SlibDiagnosticRecord::new(SlibErrorCode::LimitExceeded, WirePath::root())
            }
            Self::Allocation { requested_slots } => SlibDiagnosticRecord {
                code: SlibErrorCode::LimitAllocation,
                path: WirePath::root(),
                byte_offset: None,
                primary_origin: None,
                resource: Some(SlibResourceFailure::Allocation {
                    requested_logical_bytes: 0,
                    requested_slots: u64::try_from(*requested_slots).unwrap_or(u64::MAX),
                }),
            },
        }
    }
}

fn hir_structure_diagnostic(error: &HirFoundationValidationError) -> SlibDiagnosticRecord {
    match error {
        HirFoundationValidationError::Identity(error) => error.diagnostic(),
        HirFoundationValidationError::Resource(error) => error.diagnostic(),
        HirFoundationValidationError::SourceNativeContract { .. }
        | HirFoundationValidationError::DefinitionOrigin { .. }
        | HirFoundationValidationError::Origin(_)
        | HirFoundationValidationError::NativeBoundaryType { .. }
        | HirFoundationValidationError::NativeBoundaryShapeCoverage(_) => {
            SlibDiagnosticRecord::new(SlibErrorCode::BridgeMismatch, WirePath::root())
                .with_origin(SlibPrimaryOrigin::Metadata(MetadataLocation::Hir))
        }
        _ => SlibDiagnosticRecord::new(SlibErrorCode::ReferenceInvalid, WirePath::root())
            .with_origin(SlibPrimaryOrigin::Metadata(MetadataLocation::Hir)),
    }
}

fn mir_structure_diagnostic(error: &MirFoundationValidationError) -> SlibDiagnosticRecord {
    match error {
        MirFoundationValidationError::Identity(error) => error.diagnostic(),
        MirFoundationValidationError::Resource(error) => error.diagnostic(),
        MirFoundationValidationError::CallbackRelation(_) => {
            SlibDiagnosticRecord::new(SlibErrorCode::BridgeMismatch, WirePath::root())
                .with_origin(SlibPrimaryOrigin::Metadata(MetadataLocation::Mir))
        }
        _ => SlibDiagnosticRecord::new(SlibErrorCode::ReferenceInvalid, WirePath::root())
            .with_origin(SlibPrimaryOrigin::Metadata(MetadataLocation::Mir)),
    }
}

fn lir_structure_diagnostic(error: &LirFoundationValidationError) -> SlibDiagnosticRecord {
    match error {
        LirFoundationValidationError::Identity(error) => error.diagnostic(),
        LirFoundationValidationError::Resource(error) => error.diagnostic(),
        LirFoundationValidationError::RuntimeType {
            error: scoop_lir::RuntimeTypeMappingResolutionError::Resource(error),
            ..
        }
        | LirFoundationValidationError::Safepoint {
            error: scoop_lir::SafepointMappingResolutionError::Resource(error),
            ..
        } => error.diagnostic(),
        LirFoundationValidationError::NativeContract { .. }
        | LirFoundationValidationError::CAbiSignature { .. }
        | LirFoundationValidationError::CAbiLayout { .. }
        | LirFoundationValidationError::CallbackBridge { .. }
        | LirFoundationValidationError::NativeContractRelation(_)
        | LirFoundationValidationError::BridgeRelation(_) => {
            SlibDiagnosticRecord::new(SlibErrorCode::BridgeMismatch, WirePath::root())
                .with_origin(SlibPrimaryOrigin::Metadata(MetadataLocation::Lir))
        }
        _ => SlibDiagnosticRecord::new(SlibErrorCode::ReferenceInvalid, WirePath::root())
            .with_origin(SlibPrimaryOrigin::Metadata(MetadataLocation::Lir)),
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

fn native_owner_diagnostic(
    code: SlibErrorCode,
    owner: NativeBoundaryNominalOwner,
) -> SlibDiagnosticRecord {
    let kind = match owner {
        NativeBoundaryNominalOwner::Concrete(_) => "type",
        NativeBoundaryNominalOwner::GenericTemplate(_) => "generic-type",
    };
    identity_diagnostic(code, kind, &owner.raw_id())
}
