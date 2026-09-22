use crate::{
    CallableAbiValidationError, ExternalTypeDescriptorDecodeError,
    ExternalTypeDescriptorValidationError,
};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongExternalLirBridgeBuildError {
    Callable(crate::ParamFreeLirCallableBuildError),
    CallableContract(CallableAbiValidationError),
    TypeDescriptor(ExternalTypeDescriptorValidationError),
    SelfImport {
        provider: scoop_identity::ConeIdentity,
    },
    InvalidInitializationTarget(scoop_identity::StrongCallableDefinitionOwner),
    MissingRuntimeStringDescriptor,
    DuplicateTarget((u8, [u8; 32])),
}

impl fmt::Display for StrongExternalLirBridgeBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot build strong external LIR bridge surface: {self:?}"
        )
    }
}

impl std::error::Error for StrongExternalLirBridgeBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TypeDescriptor(error) => Some(error),
            Self::Callable(error) => Some(error),
            Self::CallableContract(error) => Some(error),
            Self::SelfImport { .. }
            | Self::InvalidInitializationTarget(_)
            | Self::MissingRuntimeStringDescriptor
            | Self::DuplicateTarget(_) => None,
        }
    }
}

#[derive(Debug)]
pub enum StrongExternalLirBridgeValidationError {
    Wire(scoop_wire::cbor::EncodeError),
    SurfaceMismatch,
}

#[derive(Debug)]
pub enum StrongExternalLirBridgeReconstructionError {
    Callable(crate::SelectedDependencyLirCallableResolutionError),
    TypeDescriptor(ExternalTypeDescriptorDecodeError),
    Surface(StrongExternalLirBridgeBuildError),
}

impl fmt::Display for StrongExternalLirBridgeReconstructionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot reconstruct strong external LIR bridge surface: {self:?}"
        )
    }
}

impl std::error::Error for StrongExternalLirBridgeReconstructionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Callable(error) => error,
            Self::TypeDescriptor(error) => error,
            Self::Surface(error) => error,
        })
    }
}

impl fmt::Display for StrongExternalLirBridgeValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong external LIR bridge surface: {self:?}"
        )
    }
}

impl std::error::Error for StrongExternalLirBridgeValidationError {}
