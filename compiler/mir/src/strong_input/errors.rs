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
    UnreferencedImportedCoreCallable {
        index: u32,
    },
    MissingImportedDependencyAuthority,
    CoreCannotImportDependency,
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
    UnreferencedImportedDependencyCallable {
        index: u32,
    },
    Foundation(MirFoundationBuildError),
    FoundationMismatch,
    StrongCallableSurfaceMismatch,
    CoreBranchMismatch,
    CoreShapeSupportSourceBranchMismatch,
    CoreShapeSupportSourceCountMismatch {
        expected: usize,
        actual: usize,
    },
    InvalidCoreShapeSupportSource {
        index: usize,
    },
    CoreShapeSupportSourceIdentity {
        index: usize,
        error: SourceDeclarationIdentityError,
    },
    CoreShapeSupportExactIdentity {
        index: usize,
        error: HashError,
    },
    CoreShapeSupportSourceMismatch(Box<CoreShapeSupportSourceMismatch>),
    MissingCallableSubject(FunctionId),
    OdrCallableSubject(FunctionId),
    OdrGeneratedNominalShape(GeneratedExactTypeLocation),
    MissingCoreShapeSupportSource {
        source: PersistentTypeId,
        exact: PersistentExactTypeId,
    },
    MissingCoreBoxedValue(PersistentExactTypeId),
    MissingCoreCoroutineStep(PersistentExactTypeId),
    MissingCoreCoroutineSlot(PersistentExactTypeId),
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
        match self {
            Self::CoreShapeSupportSourceMismatch(mismatch) => write!(
                formatter,
                "cannot seal single-Cone strong MIR input: core shape source {} expected source {:?} exact {:?}, found source {:?} exact {:?}",
                mismatch.index,
                mismatch.expected_source,
                mismatch.expected_exact,
                mismatch.actual_source,
                mismatch.actual_exact,
            ),
            _ => write!(
                formatter,
                "cannot seal single-Cone strong MIR input: {self:?}"
            ),
        }
    }
}

impl std::error::Error for SingleConeStrongMirInputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Foundation(source) => Some(source),
            Self::CoreShapeSupportSourceIdentity { error, .. } => Some(error),
            Self::CoreShapeSupportExactIdentity { error, .. } => Some(error),
            Self::MissingImportedCoreAuthority
            | Self::CoreCannotImportCore
            | Self::ImportedCoreCallableCountMismatch { .. }
            | Self::ForeignImportedCoreCallable { .. }
            | Self::DuplicateImportedCoreCallable { .. }
            | Self::UnsupportedImportedCoreCallableShape { .. }
            | Self::UnreferencedImportedCoreCallable { .. }
            | Self::MissingImportedDependencyAuthority
            | Self::CoreCannotImportDependency
            | Self::ForeignImportedDependencySelection { .. }
            | Self::ImportedDependencyCallableCountMismatch { .. }
            | Self::ForeignImportedDependencyCallable { .. }
            | Self::DuplicateImportedDependencyCallable { .. }
            | Self::UnreferencedImportedDependencyCallable { .. }
            | Self::FoundationMismatch
            | Self::StrongCallableSurfaceMismatch
            | Self::CoreBranchMismatch
            | Self::CoreShapeSupportSourceBranchMismatch
            | Self::CoreShapeSupportSourceCountMismatch { .. }
            | Self::InvalidCoreShapeSupportSource { .. }
            | Self::CoreShapeSupportSourceMismatch(_)
            | Self::MissingCallableSubject(_)
            | Self::OdrCallableSubject(_)
            | Self::OdrGeneratedNominalShape(_)
            | Self::MissingCoreShapeSupportSource { .. }
            | Self::MissingCoreBoxedValue(_)
            | Self::MissingCoreCoroutineStep(_)
            | Self::MissingCoreCoroutineSlot(_)
            | Self::MissingStrongCallableBridge { .. }
            | Self::OutputMismatch
            | Self::MissingEntryRoot(_)
            | Self::EntryImplementationMismatch { .. } => None,
        }
    }
}

#[derive(Debug)]
pub struct CoreShapeSupportSourceMismatch {
    pub(super) index: usize,
    pub(super) expected_source: PersistentTypeId,
    pub(super) expected_exact: PersistentExactTypeId,
    pub(super) actual_source: PersistentTypeId,
    pub(super) actual_exact: PersistentExactTypeId,
}
