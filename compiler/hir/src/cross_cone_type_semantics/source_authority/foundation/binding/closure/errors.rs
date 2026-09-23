use super::*;

#[derive(Debug)]
pub enum TypeFoundationReplayError {
    Resource(WireError),
    Binding(TypeFoundationBindingError),
    PublicProvider {
        source: ConeIdentity,
        public: ConeIdentity,
    },
    DependencyOrder,
    MissingProvider(ConeIdentity),
    DuplicateNominal(SourceNominalId),
    DuplicateFact(PersistentExactTypeId),
    SharedKeyMismatch,
    PublicNominal(SourceNominalId),
    PublicSourceShape(PersistentTypeId),
    MissingRepresentation(PersistentTypeId),
    DependencyFact {
        provider: ConeIdentity,
        exact: PersistentExactTypeId,
    },
}

impl From<WireError> for TypeFoundationReplayError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl From<TypeFoundationBindingError> for TypeFoundationReplayError {
    fn from(error: TypeFoundationBindingError) -> Self {
        Self::Binding(error)
    }
}

impl fmt::Display for TypeFoundationReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Binding(error) => error.fmt(f),
            Self::PublicProvider { source, public } => write!(
                f,
                "source provider {source} differs from checked public provider {public}"
            ),
            Self::DependencyOrder => f.write_str(
                "source dependencies must be strictly ordered and exclude the current provider",
            ),
            Self::MissingProvider(provider) => write!(f, "missing source provider {provider}"),
            Self::DuplicateNominal(owner) => {
                write!(f, "source nominal {owner:?} has multiple owners")
            }
            Self::DuplicateFact(exact) => write!(f, "source fact {exact} has multiple owners"),
            Self::SharedKeyMismatch => f.write_str("shared source identity keys differ"),
            Self::PublicNominal(owner) => write!(f, "public nominal {owner:?} has no source root"),
            Self::PublicSourceShape(owner) => write!(
                f,
                "public value {owner} differs from its source representation"
            ),
            Self::MissingRepresentation(owner) => {
                write!(f, "missing source representation for {owner}")
            }
            Self::DependencyFact { provider, exact } => write!(
                f,
                "source dependency fact {exact} is not locally owned by provider {provider}"
            ),
        }
    }
}

impl std::error::Error for TypeFoundationReplayError {}
