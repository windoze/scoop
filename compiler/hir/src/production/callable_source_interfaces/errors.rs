use std::fmt;

use scoop_identity::{CallableTemplateOrigin, CanonicalIdentifier, SignatureTypeKey};

use crate::{
    CallableInterfaceBuildError, CallableSourceInterfaceBuildError,
    CallableSourceInterfaceSetBuildError, CallableSourceParameterListBuildError,
    HirDefinitionSourceProjectionError, HirInterfaceSignatureProjectionError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceCallableOwnerProjectionError {
    UnknownDeclaration,
    UnknownNominalOwner,
    MissingIdentity,
    NonSourceFunction,
    ConstructorOwnerMismatch,
    TooManyVariants,
    MissingCallableInterface(CallableTemplateOrigin),
    Signature(HirInterfaceSignatureProjectionError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableSourceParameterProjectionError {
    MissingCallableInterface(CallableTemplateOrigin),
    MissingInterface,
    DuplicateInterface,
    Arity {
        expected: usize,
        actual: usize,
    },
    TooManyParameters,
    InvalidName {
        position: u32,
        source: scoop_identity::CanonicalIdentifierError,
    },
    NameMismatch {
        position: u32,
        expected: CanonicalIdentifier,
        actual: CanonicalIdentifier,
    },
    UnknownVarargType {
        position: u32,
        parameter_type: u32,
    },
    UnknownDefaultSource {
        position: u32,
        source: u32,
    },
    Signature {
        position: u32,
        source: HirInterfaceSignatureProjectionError,
    },
    TypeMismatch {
        position: u32,
        expected: Box<SignatureTypeKey>,
        actual: Box<SignatureTypeKey>,
    },
    DefinitionOrigin {
        position: u32,
        source: HirDefinitionSourceProjectionError,
    },
    ParameterList(CallableSourceParameterListBuildError),
    Record(CallableSourceInterfaceBuildError),
}

#[derive(Debug)]
pub enum CallableSourceInterfaceProductionError {
    CallableInterfaces(CallableInterfaceBuildError),
    Owner {
        subject: crate::CallableProjectionSubject,
        error: SourceCallableOwnerProjectionError,
    },
    Parameters {
        subject: crate::CallableProjectionSubject,
        error: CallableSourceParameterProjectionError,
    },
    MissingInterface(CallableTemplateOrigin),
    PropertyAccessorInterface(CallableTemplateOrigin),
    Table(CallableSourceInterfaceSetBuildError),
}

impl fmt::Display for SourceCallableOwnerProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownDeclaration => formatter.write_str("local declaration is unknown"),
            Self::UnknownNominalOwner => formatter.write_str("nominal owner is unknown"),
            Self::MissingIdentity => formatter.write_str("persistent identity is missing"),
            Self::NonSourceFunction => formatter.write_str("function is not source-declared"),
            Self::ConstructorOwnerMismatch => {
                formatter.write_str("constructor is not attached to its declared nominal owner")
            }
            Self::TooManyVariants => formatter.write_str("enum variant count exceeds u32"),
            Self::MissingCallableInterface(declaration) => write!(
                formatter,
                "public callable {declaration:?} has no declaration interface"
            ),
            Self::Signature(source) => source.fmt(formatter),
        }
    }
}

impl fmt::Display for CallableSourceParameterProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCallableInterface(declaration) => write!(
                formatter,
                "public callable {declaration:?} has no declaration interface"
            ),
            Self::MissingInterface => formatter.write_str("source parameter interface is missing"),
            Self::DuplicateInterface => {
                formatter.write_str("source parameter interface is duplicated")
            }
            Self::Arity { expected, actual } => write!(
                formatter,
                "source parameter count {actual} does not match callable interface count {expected}"
            ),
            Self::TooManyParameters => formatter.write_str("source parameter count exceeds u32"),
            Self::InvalidName { position, source } => {
                write!(
                    formatter,
                    "invalid source parameter name at {position}: {source}"
                )
            }
            Self::NameMismatch {
                position,
                expected,
                actual,
            } => write!(
                formatter,
                "source parameter {position} name `{actual}` does not match callable interface name `{expected}`"
            ),
            Self::UnknownVarargType {
                position,
                parameter_type,
            } => write!(
                formatter,
                "source parameter {position} references unknown vararg type {parameter_type}"
            ),
            Self::UnknownDefaultSource { position, source } => write!(
                formatter,
                "source parameter {position} references unknown default source {source}"
            ),
            Self::Signature { position, source } => {
                write!(
                    formatter,
                    "cannot project source parameter {position}: {source}"
                )
            }
            Self::TypeMismatch {
                position,
                expected,
                actual,
            } => write!(
                formatter,
                "source parameter {position} type {actual:?} does not match callable interface type {expected:?}"
            ),
            Self::DefinitionOrigin { position, source } => write!(
                formatter,
                "cannot project definition origin of source parameter {position}: {source}"
            ),
            Self::ParameterList(source) => source.fmt(formatter),
            Self::Record(source) => source.fmt(formatter),
        }
    }
}

impl fmt::Display for CallableSourceInterfaceProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CallableInterfaces(source) => {
                write!(formatter, "cannot project callable authority: {source}")
            }
            Self::Owner { subject, error } => {
                write!(formatter, "cannot identify public {subject}: {error}")
            }
            Self::Parameters { subject, error } => {
                write!(
                    formatter,
                    "cannot project source protocol for {subject}: {error}"
                )
            }
            Self::MissingInterface(declaration) => write!(
                formatter,
                "public source callable {declaration:?} has no source interface"
            ),
            Self::PropertyAccessorInterface(declaration) => write!(
                formatter,
                "property accessor {declaration:?} unexpectedly has a source interface"
            ),
            Self::Table(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CallableSourceInterfaceProductionError {}
