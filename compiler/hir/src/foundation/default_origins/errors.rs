use crate::PersistentLexicalRootV1;
use scoop_wire::WireError;
use std::fmt;

#[derive(Debug)]
pub enum DefaultTemplateRootOriginValidationError {
    Resource(WireError),
    Identity(String),
    CanonicalKeyMismatch(PersistentLexicalRootV1),
    MissingRoot(PersistentLexicalRootV1),
    RootProvider(PersistentLexicalRootV1),
    RootOrigin(PersistentLexicalRootV1),
}

impl From<WireError> for DefaultTemplateRootOriginValidationError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl fmt::Display for DefaultTemplateRootOriginValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Identity(error) => write!(f, "default root identity validation failed: {error}"),
            Self::CanonicalKeyMismatch(root) => {
                write!(
                    f,
                    "default source root {root:?} has a different canonical key"
                )
            }
            Self::MissingRoot(root) => write!(
                f,
                "default source root {root:?} is absent from its provider artifact"
            ),
            Self::RootProvider(root) => write!(
                f,
                "default source root {root:?} belongs to another provider"
            ),
            Self::RootOrigin(root) => write!(
                f,
                "default source root {root:?} has no matching declaration source/context"
            ),
        }
    }
}

impl std::error::Error for DefaultTemplateRootOriginValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            _ => None,
        }
    }
}
