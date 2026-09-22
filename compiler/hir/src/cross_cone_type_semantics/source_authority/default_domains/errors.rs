use super::*;

#[derive(Debug)]
pub enum DefaultSourceDomainError {
    Resource(WireError),
    Identity(String),
    Target(Box<DefaultSourceTargetSubjectError>),
    Access(Box<DefaultSourceAccessBindingError>),
    Domain(Box<DefaultSourceDomainReplayError<DefaultSourceAccessBindingError>>),
    DependencyOrder(ConeIdentity),
    IdentityGraph(ConeIdentity),
    MissingProvider(ConeIdentity),
    NominalKind(SourceNominalId),
    EqualityShape,
    EqualityKind(SourceNominalId),
    NestedAttachment,
    NestedOccurrence(DefaultNestedCallableIdentityV1),
    CallableDescriptor(DefaultCallableDeclarationV1),
    LocalReference(scoop_identity::CallableTemplateOrigin),
    Arity {
        owner: SourceNominalId,
        expected: u32,
        actual: usize,
    },
    Binder(SignatureBinderScopeError),
    Encoding(scoop_wire::cbor::EncodeError),
    PersistentDomain(PersistentAccessDomainError),
    GenericDomain(CanonicalPersistentIdSetBuildError<PersistentGenericTypeId>),
    DomainBuild(DefaultSourceAccessBuildError),
}
impl Error {
    pub(super) fn target(error: DefaultSourceTargetSubjectError) -> Self {
        match error {
            DefaultSourceTargetSubjectError::Resource(error) => Self::Resource(error),
            other => Self::Target(Box::new(other)),
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
            Self::Target(e) => e.fmt(f),
            Self::Access(e) => e.fmt(f),
            Self::Domain(e) => e.fmt(f),
            Self::Binder(e) => e.fmt(f),
            Self::Encoding(e) => e.fmt(f),
            Self::PersistentDomain(e) => e.fmt(f),
            Self::GenericDomain(e) => e.fmt(f),
            Self::DomainBuild(e) => e.fmt(f),
            Self::DependencyOrder(id) => write!(
                f,
                "default source dependency {id} is duplicated or out of order"
            ),
            Self::IdentityGraph(id) => write!(
                f,
                "default source provider {id} uses another identity graph"
            ),
            Self::MissingProvider(id) => write!(f, "missing default source provider {id}"),
            Self::NominalKind(id) => {
                write!(f, "default type source {id:?} is not a nominal declaration")
            }
            Self::EqualityShape => {
                f.write_str("default equality owner is not a nominal or structural equality source")
            }
            Self::EqualityKind(id) => {
                write!(f, "default equality source {id:?} is not a struct or enum")
            }
            Self::NestedAttachment => {
                f.write_str("default callable has no nested descriptor at its body attachment")
            }
            Self::NestedOccurrence(id) => write!(
                f,
                "default callable {id:?} has no matching bound source descriptor"
            ),
            Self::CallableDescriptor(id) => write!(
                f,
                "default callable {id:?} does not match its source descriptor"
            ),
            Self::LocalReference(id) => write!(
                f,
                "default local reference {id:?} differs from its actual callee"
            ),
            Self::Arity {
                owner,
                expected,
                actual,
            } => write!(
                f,
                "default type source {owner:?} requires {expected} arguments, got {actual}"
            ),
        }
    }
}
impl std::error::Error for Error {}
