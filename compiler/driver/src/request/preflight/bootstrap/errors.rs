use super::super::{CurrentConeLirStageError, CurrentConeMirStageError};
use super::*;

#[derive(Debug)]
pub enum CoreBootstrapProductionError {
    Hir(CurrentConeHirStageError),
    Mir(CurrentConeMirStageError),
    Lir(CurrentConeLirStageError),
    StrongProfile(CurrentConeStrongProfileError),
    Warnings(super::CurrentConeDiagnosticSetError),
    Producer(scoop_slib::ProducerRecordError),
    Cone(scoop_slib::ConeRecordError),
    Artifact(crate::CrossConeStrongIrArtifactProductionError),
    Publication(crate::CrossConeArtifactProductionError),
}

impl fmt::Display for CoreBootstrapProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hir(source) => source.fmt(formatter),
            Self::Mir(source) => source.fmt(formatter),
            Self::Lir(source) => source.fmt(formatter),
            Self::StrongProfile(source) => source.fmt(formatter),
            Self::Warnings(source) => source.fmt(formatter),
            Self::Producer(source) => source.fmt(formatter),
            Self::Cone(source) => source.fmt(formatter),
            Self::Artifact(source) => source.fmt(formatter),
            Self::Publication(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CoreBootstrapProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hir(source) => Some(source),
            Self::Mir(source) => Some(source),
            Self::Lir(source) => Some(source),
            Self::StrongProfile(source) => Some(source),
            Self::Warnings(source) => Some(source),
            Self::Producer(source) => Some(source),
            Self::Cone(source) => Some(source),
            Self::Artifact(source) => Some(source),
            Self::Publication(source) => Some(source),
        }
    }
}
