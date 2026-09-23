use scoop_hir::{
    DefaultNominalReceiverBuildError, DefaultTemplateDeclarationContractError,
    DefaultTemplateProviderParameterBuildError, DefaultTemplateProviderShapeBuildError,
    ExportDefaultTemplateKeyV1,
};
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::WireError;

use super::DefaultMetadataNominalError;
use crate::cross_cone_hir_authority::CrossConeHirNominalAuthorityError;

#[derive(Debug)]
pub enum CrossConeHirDefaultProviderContractError {
    Resource(WireError),
    Nominal(DefaultMetadataNominalError),
    Provider(Box<CrossConeHirNominalAuthorityError>),
    MissingDeclaration(CallableTemplateOrigin),
    MissingProtocol(CallableTemplateOrigin),
    MissingDefaultParameter {
        declaration: CallableTemplateOrigin,
        position: usize,
    },
    DefinitionPath,
    Shape(DefaultTemplateProviderShapeBuildError),
    Parameter(DefaultTemplateProviderParameterBuildError),
    ReceiverShape(DefaultNominalReceiverBuildError),
    Contract(Box<DefaultTemplateDeclarationContractError<DefaultMetadataNominalError>>),
    Template {
        index: usize,
        key: ExportDefaultTemplateKeyV1,
        source: Box<Self>,
    },
}

impl From<WireError> for CrossConeHirDefaultProviderContractError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<DefaultMetadataNominalError> for CrossConeHirDefaultProviderContractError {
    fn from(error: DefaultMetadataNominalError) -> Self {
        Self::Nominal(error)
    }
}
impl std::fmt::Display for CrossConeHirDefaultProviderContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Nominal(error) => error.fmt(f),
            Self::Provider(error) => error.fmt(f),
            Self::MissingDeclaration(declaration) => {
                write!(f, "default provider declaration {declaration:?} is absent")
            }
            Self::MissingProtocol(declaration) => write!(
                f,
                "default provider declaration {declaration:?} has no source parameter protocol"
            ),
            Self::MissingDefaultParameter {
                declaration,
                position,
            } => write!(
                f,
                "default provider {declaration:?} has no default at parameter {position}"
            ),
            Self::DefinitionPath => {
                f.write_str("default path does not identify a provider default parameter")
            }
            Self::Shape(error) => error.fmt(f),
            Self::Parameter(error) => error.fmt(f),
            Self::ReceiverShape(error) => error.fmt(f),
            Self::Contract(error) => error.fmt(f),
            Self::Template { index, key, source } => write!(
                f,
                "default template[{index}] {key:?} has an invalid provider contract: {source}"
            ),
        }
    }
}
impl std::error::Error for CrossConeHirDefaultProviderContractError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Nominal(error) => Some(error),
            Self::Provider(error) => Some(error.as_ref()),
            Self::Shape(error) => Some(error),
            Self::Parameter(error) => Some(error),
            Self::ReceiverShape(error) => Some(error),
            Self::Contract(error) => Some(error.as_ref()),
            Self::Template { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}
