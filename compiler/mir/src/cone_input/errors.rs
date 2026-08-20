use std::fmt;

use scoop_identity::{
    CallableOwner, ConeIdentity, PersistentExactTypeId, PersistentTypeId,
    SourceDeclarationIdentityError,
};
use scoop_wire::HashError;

use crate::{CallableSignatureSubject, FunctionId, GeneratedExactTypeLocation};

#[derive(Debug)]
pub enum ConeMirInputError {
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
    DuplicateExternalImplementation {
        implementation: scoop_identity::StrongCallableDefinitionOwner,
    },
    ExternalCallbackSignatureMismatch {
        bridge: crate::CallbackBridgeId,
    },
    Initialization(super::StrongInitializationUnitError),
    Production(crate::MirProductionBuildError),
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
    OutputMismatch,
    MissingEntryRoot(FunctionId),
    EntryImplementationMismatch {
        expected: CallableOwner,
        actual: CallableSignatureSubject,
    },
}

impl fmt::Display for ConeMirInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot construct Cone MIR input: {self:?}")
    }
}

impl std::error::Error for ConeMirInputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Initialization(source) => Some(source),
            Self::Production(source) => Some(source),
            Self::ShapeSupportSourceIdentity { error, .. } => Some(error),
            Self::ShapeSupportExactIdentity { error, .. } => Some(error),
            Self::ForeignExternalCallableSelection { .. }
            | Self::ExternalCallableCountMismatch { .. }
            | Self::ForeignExternalCallable { .. }
            | Self::DuplicateExternalImplementation { .. }
            | Self::ExternalCallbackSignatureMismatch { .. }
            | Self::StrongCallableSurfaceMismatch
            | Self::NonCanonicalShapeSupportSource { .. }
            | Self::InvalidShapeSupportSource { .. }
            | Self::MissingCallableSubject(_)
            | Self::ForeignGeneratedHelperCallable(_)
            | Self::MissingGeneratedSourceExact { .. }
            | Self::InvalidGeneratedSourceOwner { .. }
            | Self::MissingShapeSupportSource { .. }
            | Self::MissingBoxedValue(_)
            | Self::MissingCoroutineStep(_)
            | Self::MissingCoroutineSlot(_)
            | Self::OutputMismatch
            | Self::MissingEntryRoot(_)
            | Self::EntryImplementationMismatch { .. } => None,
        }
    }
}
