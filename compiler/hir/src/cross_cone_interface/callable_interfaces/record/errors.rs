use std::fmt;

use scoop_identity::CallableTemplateOrigin;

use super::{
    BinderListValidationError, CallableModalityV1, PublicDeclarationOwnerV1, PublicLookupAccessV1,
    SourceParameterListValidationError,
};
use crate::CallableSourceEffectsBuildError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableInterfaceRecordBuildError {
    MissingTypeParameters(CallableTemplateOrigin),
    UnexpectedTypeParameters(CallableTemplateOrigin),
    MissingExtensionReceiver,
    UnexpectedReceiver(PublicDeclarationOwnerV1),
    NominalOwnerRequired {
        declaration: CallableTemplateOrigin,
        actual: PublicDeclarationOwnerV1,
    },
    SlotAccessRequired(CallableModalityV1),
    DirectCallableContract {
        declaration: CallableTemplateOrigin,
        owner: PublicDeclarationOwnerV1,
        modality: CallableModalityV1,
        access: PublicLookupAccessV1,
    },
    InvalidExternTarget {
        declaration: CallableTemplateOrigin,
        owner: PublicDeclarationOwnerV1,
    },
}

impl fmt::Display for CallableInterfaceRecordBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTypeParameters(declaration) => {
                write!(
                    formatter,
                    "generic callable {declaration:?} has no type parameters"
                )
            }
            Self::UnexpectedTypeParameters(declaration) => write!(
                formatter,
                "non-generic callable {declaration:?} has type parameters"
            ),
            Self::MissingExtensionReceiver => {
                formatter.write_str("extension callable has no receiver")
            }
            Self::UnexpectedReceiver(owner) => {
                write!(
                    formatter,
                    "{owner:?} callable cannot declare an extension receiver"
                )
            }
            Self::NominalOwnerRequired {
                declaration,
                actual,
            } => write!(
                formatter,
                "callable {declaration:?} requires a nominal owner, found {actual:?}"
            ),
            Self::SlotAccessRequired(modality) => {
                write!(
                    formatter,
                    "{modality:?} callable requires public slot access"
                )
            }
            Self::DirectCallableContract {
                declaration,
                owner,
                modality,
                access,
            } => write!(
                formatter,
                "callable {declaration:?} owned by {owner:?} requires Final + DirectOnly, found {modality:?} + {access:?}"
            ),
            Self::InvalidExternTarget { declaration, owner } => write!(
                formatter,
                "source extern callable must be a non-generic top-level function, found {declaration:?} owned by {owner:?}"
            ),
        }
    }
}

impl std::error::Error for CallableInterfaceRecordBuildError {}

#[derive(Debug)]
pub enum CallableInterfaceRecordResolutionError<E> {
    Declaration(E),
    Owner(E),
    TypeParameters(BinderListValidationError<E>),
    Receiver(E),
    Parameters(SourceParameterListValidationError<E>),
    Result(E),
    Effects(CallableSourceEffectsBuildError),
    Record(CallableInterfaceRecordBuildError),
}

impl<E: fmt::Display> fmt::Display for CallableInterfaceRecordResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Declaration(error) => write!(formatter, "invalid callable declaration: {error}"),
            Self::Owner(error) => write!(formatter, "invalid callable owner: {error}"),
            Self::TypeParameters(error) => {
                write!(formatter, "invalid callable type parameters: {error}")
            }
            Self::Receiver(error) => write!(formatter, "invalid callable receiver: {error}"),
            Self::Parameters(error) => write!(formatter, "invalid callable parameters: {error}"),
            Self::Result(error) => write!(formatter, "invalid callable result: {error}"),
            Self::Effects(error) => write!(formatter, "invalid callable effects: {error}"),
            Self::Record(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for CallableInterfaceRecordResolutionError<E>
{
}
