use super::*;

#[derive(Debug)]
pub enum DefaultNestedIdentityValidationError {
    Resource(WireError),
    Foundation(TypeFoundationBindingError),
    Identity {
        identity: DefaultNestedCallableIdentityV1,
        reason: DefaultSourceNestedIdentityFailureV1,
    },
}
impl From<WireError> for Error {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Foundation(error) => error.fmt(f),
            Self::Identity { identity, reason } => {
                write!(f, "default nested identity {identity:?}: {reason:?}")
            }
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Foundation(error) => Some(error),
            Self::Identity { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultSourceNestedIdentityFailureV1 {
    MissingArtifactRecord,
    Kind,
    DefinitionPath,
    DefinitionSource,
    LexicalParent,
    DefinitionContext,
    OwnerBinderArity { expected: u32, actual: u32 },
    BodyBinderArity { expected: u32, actual: u32 },
}
