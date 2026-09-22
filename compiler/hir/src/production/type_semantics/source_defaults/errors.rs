use crate::*;
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::WireError;
use std::fmt;
#[derive(Debug)]
pub enum NominalDefaultSourceProductionError {
    Resource(WireError),
    Sources(CrossConeTypeSemanticsProductionError),
    Body(DefaultSourceBodyProductionError),
    Template(DefaultSourceTemplateBuildError),
    Table(DefaultSourceTemplateTableBuildError),
    Coverage(DefaultSourceTemplateCoverageError),
    Profiles(SourceInventoryError),
    MissingOwner(CallableTemplateOrigin),
    DuplicateOwner(CallableTemplateOrigin),
    PositionOverflow(CallableTemplateOrigin),
}
impl fmt::Display for NominalDefaultSourceProductionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Sources(e) => e.fmt(f),
            Self::Body(e) => e.fmt(f),
            Self::Template(e) => e.fmt(f),
            Self::Table(e) => e.fmt(f),
            Self::Coverage(e) => e.fmt(f),
            Self::Profiles(e) => e.fmt(f),
            Self::MissingOwner(owner) => {
                write!(f, "nominal default source has no sealed owner {owner:?}")
            }
            Self::DuplicateOwner(owner) => {
                write!(f, "duplicate nominal default source owner {owner:?}")
            }
            Self::PositionOverflow(owner) => write!(
                f,
                "nominal source parameter position exceeds u32 for {owner:?}"
            ),
        }
    }
}
impl std::error::Error for NominalDefaultSourceProductionError {}
