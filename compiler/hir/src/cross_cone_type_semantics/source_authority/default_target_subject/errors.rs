use super::*;

#[derive(Debug)]
pub enum DefaultSourceTargetSubjectError {
    Resource(WireError),
    Foundation(TypeFoundationBindingError),
    Encoding(scoop_wire::cbor::EncodeError),
    Identity(SourceDeclarationIdentityError),
    MissingTarget(Target),
    Role(Target),
    MissingGenerated(PersistentTypeId),
    MissingDeclaration(Subject),
    DeclarationRole(Subject),
    ForeignDeclaration(Subject),
    NominalKind {
        owner: SourceNominalId,
        expected: SourceDeclarationKind,
    },
    PropertyOwner {
        field: PersistentFieldId,
        property: PersistentPropertyId,
    },
}
impl From<WireError> for Error {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<TypeFoundationBindingError> for Error {
    fn from(error: TypeFoundationBindingError) -> Self {
        match error {
            TypeFoundationBindingError::Resource(error) => Self::Resource(error),
            other => Self::Foundation(other),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Foundation(e) => e.fmt(f),
            Self::Encoding(e) => e.fmt(f),
            Self::Identity(e) => e.fmt(f),
            Self::MissingTarget(target) => {
                write!(f, "artifact has no default access target key {target:?}")
            }
            Self::Role(target) => write!(
                f,
                "default access target has incompatible identity role {target:?}"
            ),
            Self::MissingGenerated(id) => write!(f, "artifact has no generated field owner {id}"),
            Self::MissingDeclaration(id) => {
                write!(f, "artifact has no indirect access declaration {id:?}")
            }
            Self::DeclarationRole(id) => {
                write!(f, "invalid indirect access declaration role {id:?}")
            }
            Self::ForeignDeclaration(id) => write!(
                f,
                "indirect access declaration belongs to another provider {id:?}"
            ),
            Self::NominalKind { owner, expected } => {
                write!(f, "indirect access owner {owner:?} is not {expected:?}")
            }
            Self::PropertyOwner { field, property } => write!(
                f,
                "default field {field} and property {property} have different owners"
            ),
        }
    }
}
impl std::error::Error for Error {}
