use super::*;

#[derive(Debug)]
pub enum DefaultSourceDirectDomainError {
    Resource(WireError),
    Foundation(TypeFoundationBindingError),
    ProtectedOwner,
    ProtectedClass(SourceNominalId),
    ExactClass(PersistentExactTypeId),
    Identity(scoop_wire::HashError),
    Encoding(scoop_wire::cbor::EncodeError),
    Domain(PersistentAccessDomainError),
    GenericSubclasses(CanonicalPersistentIdSetBuildError<PersistentGenericTypeId>),
    Build(DefaultSourceAccessBuildError),
    Witness {
        kind: ExportDefaultReferenceKindV1,
        index: u32,
    },
}
impl From<WireError> for DomainError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for DomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Foundation(e) => e.fmt(f),
            Self::Identity(e) => e.fmt(f),
            Self::Encoding(e) => e.fmt(f),
            Self::Domain(e) => e.fmt(f),
            Self::GenericSubclasses(e) => e.fmt(f),
            Self::Build(e) => e.fmt(f),
            Self::ProtectedOwner => {
                f.write_str("protected default provider has no lexical class owner")
            }
            Self::ProtectedClass(id) => {
                write!(f, "default source protected owner {id:?} is not a class")
            }
            Self::ExactClass(id) => write!(
                f,
                "default source class exact {id} differs from its nominal key"
            ),
            Self::Witness { kind, index } => write!(
                f,
                "default source {kind} occurrence {index} direct call domain differs from its provider declaration"
            ),
        }
    }
}
impl std::error::Error for DomainError {}
