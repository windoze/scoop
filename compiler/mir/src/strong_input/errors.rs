use std::fmt;

use scoop_identity::{
    CallableOwner, ConeIdentity, PersistentExactTypeId, PersistentTypeId,
    SourceDeclarationIdentityError,
};
use scoop_wire::HashError;

use crate::{FunctionId, GeneratedExactTypeLocation, MirFoundationBuildError};

#[derive(Debug)]
pub enum SingleConeStrongMirInputError {
    MissingImportedCoreAuthority,
    CoreCannotImportCore,
    ImportedCoreCallableCountMismatch {
        module: usize,
        selected: usize,
    },
    ForeignImportedCoreCallable {
        index: u32,
    },
    DuplicateImportedCoreCallable {
        index: u32,
    },
    UnsupportedImportedCoreCallableShape {
        index: u32,
    },
    UnreferencedExternalCallable {
        index: u32,
    },
    DuplicateExternalImplementation {
        implementation: scoop_identity::StrongCallableDefinitionOwner,
    },
    MissingImportedDependencyAuthority,
    ForeignImportedDependencySelection {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    ImportedDependencyCallableCountMismatch {
        module: usize,
        selected: usize,
    },
    ForeignImportedDependencyCallable {
        index: u32,
    },
    DuplicateImportedDependencyCallable {
        index: u32,
    },
    Initialization(super::StrongInitializationUnitError),
    Foundation(MirFoundationBuildError),
    FoundationMismatch,
    StrongCallableSurfaceMismatch,
    CoreBranchMismatch,
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
            Self::ShapeSupportSourceIdentity { error, .. } => Some(error),
            Self::ShapeSupportExactIdentity { error, .. } => Some(error),
            Self::MissingImportedCoreAuthority
            | Self::CoreCannotImportCore
            | Self::ImportedCoreCallableCountMismatch { .. }
            | Self::ForeignImportedCoreCallable { .. }
            | Self::DuplicateImportedCoreCallable { .. }
            | Self::UnsupportedImportedCoreCallableShape { .. }
            | Self::UnreferencedExternalCallable { .. }
            | Self::DuplicateExternalImplementation { .. }
            | Self::MissingImportedDependencyAuthority
            | Self::ForeignImportedDependencySelection { .. }
            | Self::ImportedDependencyCallableCountMismatch { .. }
            | Self::ForeignImportedDependencyCallable { .. }
            | Self::DuplicateImportedDependencyCallable { .. }
            | Self::FoundationMismatch
            | Self::StrongCallableSurfaceMismatch
            | Self::CoreBranchMismatch
            | Self::NonCanonicalShapeSupportSource { .. }
            | Self::InvalidShapeSupportSource { .. }
            | Self::MissingCallableSubject(_)
            | Self::OdrCallableSubject(_)
            | Self::OdrGeneratedNominalShape(_)
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
