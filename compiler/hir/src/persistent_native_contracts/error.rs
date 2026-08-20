use super::HirSourceNativeContractOwner;
use crate::{FunctionId, GlobalId};

#[derive(Debug)]
pub enum HirSourceNativeContractError {
    UnknownExternFunction {
        function: FunctionId,
        external: u32,
    },
    DuplicateExternFunction {
        function: FunctionId,
        external: u32,
    },
    UnownedExternFunction {
        external: u32,
    },
    FunctionRelation {
        function: FunctionId,
    },
    InvalidFunctionIdentity {
        function: FunctionId,
    },
    UnitCParameter {
        function: FunctionId,
        parameter: usize,
    },
    UnknownGlobalProperty {
        global: GlobalId,
        property: u32,
    },
    InvalidGlobalProperty {
        global: GlobalId,
    },
    UnexpectedExtensionProperty {
        global: GlobalId,
    },
    InvalidSignatureType {
        owner: HirSourceNativeContractOwner,
        error: crate::HirSignatureTypeMappingError,
    },
    InvalidSymbol {
        owner: HirSourceNativeContractOwner,
        error: scoop_identity::SourceNativeSymbolError,
    },
    InvalidLibrary {
        owner: HirSourceNativeContractOwner,
        error: scoop_identity::CanonicalNativeNameError,
    },
    InvalidContract {
        owner: HirSourceNativeContractOwner,
        error: scoop_identity::SourceNativeContractError,
    },
    DuplicateIdentity {
        owner: HirSourceNativeContractOwner,
    },
}

impl HirSourceNativeContractError {
    pub const fn owner(&self) -> Option<HirSourceNativeContractOwner> {
        match self {
            Self::UnknownExternFunction { function, .. }
            | Self::DuplicateExternFunction { function, .. }
            | Self::FunctionRelation { function }
            | Self::InvalidFunctionIdentity { function }
            | Self::UnitCParameter { function, .. } => {
                Some(HirSourceNativeContractOwner::Function(*function))
            }
            Self::UnknownGlobalProperty { global, .. }
            | Self::InvalidGlobalProperty { global }
            | Self::UnexpectedExtensionProperty { global } => {
                Some(HirSourceNativeContractOwner::Global(*global))
            }
            Self::InvalidSignatureType { owner, .. }
            | Self::InvalidSymbol { owner, .. }
            | Self::InvalidLibrary { owner, .. }
            | Self::InvalidContract { owner, .. }
            | Self::DuplicateIdentity { owner } => Some(*owner),
            Self::UnownedExternFunction { .. } => None,
        }
    }
}

impl std::fmt::Display for HirSourceNativeContractError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownExternFunction { external, .. } => {
                write!(formatter, "function references missing extern {external}")
            }
            Self::DuplicateExternFunction { external, .. } => {
                write!(
                    formatter,
                    "extern {external} is owned by multiple functions"
                )
            }
            Self::UnownedExternFunction { external } => {
                write!(formatter, "extern {external} has no owning function")
            }
            Self::FunctionRelation { .. } => {
                formatter.write_str("extern function relation is inconsistent")
            }
            Self::InvalidFunctionIdentity { .. } => {
                formatter.write_str("extern function has no plain source identity")
            }
            Self::UnitCParameter { parameter, .. } => {
                write!(formatter, "C extern parameter {parameter} has Unit type")
            }
            Self::UnknownGlobalProperty { property, .. } => {
                write!(
                    formatter,
                    "extern global references missing property {property}"
                )
            }
            Self::InvalidGlobalProperty { .. } => {
                formatter.write_str("extern global property relation is inconsistent")
            }
            Self::UnexpectedExtensionProperty { .. } => {
                formatter.write_str("extern global cannot be an extension property")
            }
            Self::InvalidSignatureType { error, .. } => error.fmt(formatter),
            Self::InvalidSymbol { error, .. } => error.fmt(formatter),
            Self::InvalidLibrary { error, .. } => error.fmt(formatter),
            Self::InvalidContract { error, .. } => error.fmt(formatter),
            Self::DuplicateIdentity { .. } => {
                formatter.write_str("duplicate source-native contract identity")
            }
        }
    }
}

impl std::error::Error for HirSourceNativeContractError {}
