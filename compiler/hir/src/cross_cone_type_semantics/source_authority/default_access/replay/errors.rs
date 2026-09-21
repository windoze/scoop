use super::*;

#[derive(Debug)]
pub enum DefaultSourceDomainReplayError<E> {
    Resource(WireError),
    Authority(E),
    ProtectedOwner,
    ProtectedClass(SourceNominalId),
    ExactClass(PersistentExactTypeId),
    Identity(scoop_wire::HashError),
    Encoding(scoop_wire::cbor::EncodeError),
    Domain(PersistentAccessDomainError),
    GenericSubclasses(CanonicalPersistentIdSetBuildError<PersistentGenericTypeId>),
    Build(DefaultSourceAccessBuildError),
}
impl<E> From<WireError> for Error<E> {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl<E: std::fmt::Display> std::fmt::Display for Error<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Authority(e) => e.fmt(f),
            Self::Identity(e) => e.fmt(f),
            Self::Encoding(e) => e.fmt(f),
            Self::Domain(e) => e.fmt(f),
            Self::GenericSubclasses(e) => e.fmt(f),
            Self::Build(e) => e.fmt(f),
            Self::ProtectedOwner => {
                f.write_str("protected source declaration has no lexical class owner")
            }
            Self::ProtectedClass(id) => {
                write!(f, "default source protected owner {id:?} is not a class")
            }
            Self::ExactClass(id) => write!(
                f,
                "default source class exact {id} differs from its nominal key"
            ),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for Error<E> {}
