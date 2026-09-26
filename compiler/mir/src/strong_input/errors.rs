use std::fmt;

use scoop_identity::{
    CallableOwner, ConeIdentity, PersistentExactTypeId, PersistentTypeId,
    SourceDeclarationIdentityError,
};
use scoop_wire::HashError;

use crate::{FunctionId, GeneratedExactTypeLocation, MirFoundationBuildError};

#[derive(Debug)]
pub enum SingleConeStrongMirInputError {
    MissingExternalCallableSelection,
    ForeignExternalCallableSelection {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    ExternalCallableCountMismatch {
        module: usize,
        selected: usize,
    },
    ForeignExternalCallable {
        index: u32,
    },
    InitializationCycleGcEffect {
        index: u32,
    },
    UnreferencedExternalCallable {
        index: u32,
    },
    DuplicateExternalImplementation {
        implementation: scoop_identity::StrongCallableDefinitionOwner,
    },
    Initialization(super::StrongInitializationUnitError),
    Foundation(MirFoundationBuildError),
    Production(crate::MirProductionBuildError),
    FoundationMismatch,
    StrongCallableSurfaceMismatch,
    NonCanonicalShapeSupportSource {
        index: usize,
        previous: PersistentTypeId,
        current: PersistentTypeId,
    },
    InvalidShapeSupportSource {
        index: usize,
    },
    ShapeSupportSourceIdentity {
        index: usize,
        error: SourceDeclarationIdentityError,
    },
    ShapeSupportExactIdentity {
        index: usize,
        error: HashError,
    },
    MissingCallableSubject(FunctionId),
    OdrCallableSubject(FunctionId),
    OdrGeneratedNominalShape(GeneratedExactTypeLocation),
    ForeignGeneratedHelperCallable(GeneratedExactTypeLocation),
    MissingGeneratedSourceExact {
        location: GeneratedExactTypeLocation,
        exact: PersistentExactTypeId,
    },
    InvalidGeneratedSourceOwner {
        location: GeneratedExactTypeLocation,
        exact: PersistentExactTypeId,
    },
    MissingShapeSupportSource {
        source: PersistentTypeId,
        exact: PersistentExactTypeId,
    },
    MissingBoxedValue(PersistentExactTypeId),
    MissingCoroutineStep(PersistentExactTypeId),
    MissingCoroutineSlot(PersistentExactTypeId),
    MissingStrongCallableBridge {
        index: usize,
        implementation: CallableOwner,
    },
    OutputMismatch,
    MissingEntryRoot(FunctionId),
    EntryImplementationMismatch {
        expected: CallableOwner,
        actual: CallableOwner,
    },
}

impl fmt::Display for SingleConeStrongMirInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot seal single-Cone strong MIR input: {self:?}"
        )
    }
}

impl std::error::Error for SingleConeStrongMirInputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Initialization(source) => Some(source),
            Self::Foundation(source) => Some(source),
            Self::Production(source) => Some(source),
            Self::ShapeSupportSourceIdentity { error, .. } => Some(error),
            Self::ShapeSupportExactIdentity { error, .. } => Some(error),
            Self::MissingExternalCallableSelection
            | Self::ForeignExternalCallableSelection { .. }
            | Self::ExternalCallableCountMismatch { .. }
            | Self::ForeignExternalCallable { .. }
            | Self::InitializationCycleGcEffect { .. }
            | Self::UnreferencedExternalCallable { .. }
            | Self::DuplicateExternalImplementation { .. }
            | Self::FoundationMismatch
            | Self::StrongCallableSurfaceMismatch
            | Self::NonCanonicalShapeSupportSource { .. }
            | Self::InvalidShapeSupportSource { .. }
            | Self::MissingCallableSubject(_)
            | Self::OdrCallableSubject(_)
            | Self::OdrGeneratedNominalShape(_)
            | Self::ForeignGeneratedHelperCallable(_)
            | Self::MissingGeneratedSourceExact { .. }
            | Self::InvalidGeneratedSourceOwner { .. }
            | Self::MissingShapeSupportSource { .. }
            | Self::MissingBoxedValue(_)
            | Self::MissingCoroutineStep(_)
            | Self::MissingCoroutineSlot(_)
            | Self::MissingStrongCallableBridge { .. }
            | Self::OutputMismatch
            | Self::MissingEntryRoot(_)
            | Self::EntryImplementationMismatch { .. } => None,
        }
    }
}
