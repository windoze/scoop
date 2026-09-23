//! Typed failures for strong-profile Compile validation.

use super::*;

#[derive(Debug)]
pub enum StrongProfileFoundationError {
    HirStructure(HirFoundationValidationError),
    MirStructure(MirFoundationValidationError),
    LirStructure(LirFoundationValidationError),
    HirOdr(OdrFreeHirFoundationError),
    MirOdr(OdrFreeMirFoundationError),
    LirOdr(OdrFreeLirFoundationError),
}

impl fmt::Display for StrongProfileFoundationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid strong-profile foundation: {self:?}")
    }
}

impl std::error::Error for StrongProfileFoundationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::HirStructure(error) => error,
            Self::MirStructure(error) => error,
            Self::LirStructure(error) => error,
            Self::HirOdr(error) => error,
            Self::MirOdr(error) => error,
            Self::LirOdr(error) => error,
        })
    }
}

#[derive(Debug)]
pub enum StrongProfileLocalProductionError {
    Hir(CoreBootstrapInterfaceValidationError),
    Mir(MirProductionValidationError),
}

#[derive(Debug)]
pub enum StrongProfileProductionError {
    Local(StrongProfileLocalProductionError),
    Relation(StrongProfileRelationError),
    Lir(StrongProfileLirProductionError),
}

impl fmt::Display for StrongProfileProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid strong-profile production: {self:?}")
    }
}

impl std::error::Error for StrongProfileProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Local(error) => error,
            Self::Relation(error) => error,
            Self::Lir(error) => error,
        })
    }
}

impl fmt::Display for StrongProfileLocalProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong-profile local production: {self:?}"
        )
    }
}

impl std::error::Error for StrongProfileLocalProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Hir(error) => error,
            Self::Mir(error) => error,
        })
    }
}

#[derive(Debug)]
pub enum StrongProfileLirProductionError {
    ShapeSources(PublicNominalShapeProjectionError),
    Production(StrongProductionSectionValidationError),
    InitializationAbiRelation(StrongProfileInitializationAbiRelationError),
}

impl fmt::Display for StrongProfileLirProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid strong-profile LIR production: {self:?}")
    }
}

impl std::error::Error for StrongProfileLirProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::ShapeSources(error) => error,
            Self::Production(error) => error,
            Self::InitializationAbiRelation(error) => error,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongProfileInitializationAbiRelationError {
    PresenceMismatch,
    InvalidInitializationCycleOwner,
    InitializationCycleMismatch,
    InitializationCycleSignatureMismatch,
}

impl fmt::Display for StrongProfileInitializationAbiRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid MIR-to-LIR initialization ABI relation: {self:?}"
        )
    }
}

impl std::error::Error for StrongProfileInitializationAbiRelationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongProfileRelationError {
    ShapeSources(PublicNominalShapeProjectionError),
    OutputMismatch,
    InitializationCycleRoleMismatch,
    InvalidInitializationCycleDefinition,
    InitializationCycleMismatch,
    InitializationCycleSignatureMismatch,
}

impl fmt::Display for StrongProfileRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong-profile cross-layer relation: {self:?}"
        )
    }
}

impl std::error::Error for StrongProfileRelationError {}

pub type SingleConeCompileSectionDecodeError = CompileSectionDecodeError;

#[derive(Debug)]
pub enum StrongCompileArtifactValidationError {
    Decode(Box<SingleConeCompileSectionDecodeError>),
    Identities(Box<IdentityValidationError>),
    Foundations(Box<StrongProfileFoundationError>),
    LocalProduction(Box<StrongProfileLocalProductionError>),
    Relations(Box<StrongProfileRelationError>),
    ExternalBridges(Box<StrongExternalLirBridgeReconstructionError>),
    LirProduction(Box<StrongProfileLirProductionError>),
    NativeBoundary(Box<NativeBoundaryCompileError>),
    Commit(Box<CompileCommitError>),
}

impl fmt::Display for StrongCompileArtifactValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid SingleConeStrong Compile artifact: {self:?}"
        )
    }
}

impl std::error::Error for StrongCompileArtifactValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Decode(error) => error.as_ref(),
            Self::Identities(error) => error.as_ref(),
            Self::Foundations(error) => error.as_ref(),
            Self::LocalProduction(error) => error.as_ref(),
            Self::Relations(error) => error.as_ref(),
            Self::ExternalBridges(error) => error.as_ref(),
            Self::LirProduction(error) => error.as_ref(),
            Self::NativeBoundary(error) => error.as_ref(),
            Self::Commit(error) => error.as_ref(),
        })
    }
}
