use super::*;

#[derive(Debug)]
pub enum DefaultSourceTypeDomainError {
    Resource(WireError),
    Identity(String),
    Foundation(Box<TypeFoundationBindingError>),
    Access(Box<DefaultSourceAccessBindingError>),
    Domain(Box<DefaultSourceDomainReplayError<DefaultSourceAccessBindingError>>),
    DependencyOrder(ConeIdentity),
    IdentityGraph(ConeIdentity),
    MissingProvider(ConeIdentity),
    NominalKind(SourceNominalId),
    Arity {
        owner: SourceNominalId,
        expected: u32,
        actual: usize,
    },
    CoreRole(&'static str),
    Binder(SignatureBinderScopeError),
    Encoding(scoop_wire::cbor::EncodeError),
    PersistentDomain(PersistentAccessDomainError),
    GenericDomain(CanonicalPersistentIdSetBuildError<PersistentGenericTypeId>),
    DomainBuild(DefaultSourceAccessBuildError),
}
impl Error {
    pub(super) fn foundation(error: TypeFoundationBindingError) -> Self {
        match error {
            TypeFoundationBindingError::Resource(error) => Self::Resource(error),
            other => Self::Foundation(Box::new(other)),
        }
    }
    pub(super) fn access(error: DefaultSourceAccessBindingError) -> Self {
        match error {
            DefaultSourceAccessBindingError::Resource(error) => Self::Resource(error),
            other => Self::Access(Box::new(other)),
        }
    }
    pub(super) fn domain(
        error: DefaultSourceDomainReplayError<DefaultSourceAccessBindingError>,
    ) -> Self {
        match error {
            DefaultSourceDomainReplayError::Resource(error) => Self::Resource(error),
            DefaultSourceDomainReplayError::Authority(error) => Self::access(error),
            other => Self::Domain(Box::new(other)),
        }
    }
}
impl From<WireError> for Error {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Identity(e) => e.fmt(f),
            Self::Foundation(e) => e.fmt(f),
            Self::Access(e) => e.fmt(f),
            Self::Domain(e) => e.fmt(f),
            Self::Binder(e) => e.fmt(f),
            Self::Encoding(e) => e.fmt(f),
            Self::PersistentDomain(e) => e.fmt(f),
            Self::GenericDomain(e) => e.fmt(f),
            Self::DomainBuild(e) => e.fmt(f),
            Self::DependencyOrder(id) => write!(
                f,
                "default type source dependency {id} is duplicated or out of order"
            ),
            Self::IdentityGraph(id) => write!(
                f,
                "default type source provider {id} uses another identity graph"
            ),
            Self::MissingProvider(id) => write!(f, "missing default type source provider {id}"),
            Self::NominalKind(id) => {
                write!(f, "default type source {id:?} is not a nominal declaration")
            }
            Self::Arity {
                owner,
                expected,
                actual,
            } => write!(
                f,
                "default type source {owner:?} requires {expected} arguments, got {actual}"
            ),
            Self::CoreRole(role) => write!(
                f,
                "default type source core role {role} lacks its canonical artifact key"
            ),
        }
    }
}
impl std::error::Error for Error {}
