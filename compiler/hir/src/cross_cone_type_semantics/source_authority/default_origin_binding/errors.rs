use super::*;
use scoop_wire::WireError;
use std::fmt;

#[derive(Debug)]
pub enum DefaultSourceOriginBindingError {
    Resource(WireError),
    Coverage(DefaultSourceTemplateCoverageError),
    Foundation(TypeFoundationBindingError),
    DependencyOrder(ConeIdentity),
    IdentityGraph(ConeIdentity),
    MissingProvider(ConeIdentity),
    MissingRoot(PersistentLexicalRootV1),
    RootProvider(PersistentLexicalRootV1),
    RootOrigin(PersistentLexicalRootV1),
    Origin {
        key: ProtectedDefaultTemplateKeyV1,
        error: TypeFoundationBindingError,
    },
}
impl From<WireError> for DefaultSourceOriginBindingError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<DefaultSourceTemplateCoverageError> for DefaultSourceOriginBindingError {
    fn from(error: DefaultSourceTemplateCoverageError) -> Self {
        Self::Coverage(error)
    }
}
impl From<TypeFoundationBindingError> for DefaultSourceOriginBindingError {
    fn from(error: TypeFoundationBindingError) -> Self {
        Self::Foundation(error)
    }
}
impl fmt::Display for DefaultSourceOriginBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Coverage(error) => error.fmt(f),
            Self::Foundation(error) => error.fmt(f),
            Self::DependencyOrder(provider) => write!(
                f,
                "default source dependency {provider:?} repeats, is out of order or is the current provider"
            ),
            Self::IdentityGraph(provider) => write!(
                f,
                "default source dependency {provider:?} uses another identity graph"
            ),
            Self::MissingProvider(provider) => {
                write!(f, "default source has no artifact provider {provider:?}")
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
            Self::Origin { key, error } => write!(
                f,
                "default source {key:?} has an invalid artifact location: {error}"
            ),
        }
    }
}
impl std::error::Error for DefaultSourceOriginBindingError {}
