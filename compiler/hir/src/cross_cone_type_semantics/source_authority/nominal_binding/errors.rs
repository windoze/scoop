use super::*;
use std::fmt;

#[derive(Debug)]
pub enum NominalSourceBindingError {
    Resource(WireError),
    Foundation(TypeFoundationBindingError),
    Inventory(&'static str),
    MissingSource(SourceNominalId),
    MissingField(PersistentFieldId),
    FieldOwner {
        owner: SourceNominalId,
        field: PersistentFieldId,
    },
    MissingVariant(PersistentEnumVariantId),
    MissingVariantField(PersistentEnumVariantFieldId),
    MissingObject(PersistentObjectValueId),
    Origin(DefinitionOriginSubject),
    Identity(String),
    Contract {
        owner: SourceNominalId,
        reason: String,
    },
}

impl From<WireError> for NominalSourceBindingError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<TypeFoundationBindingError> for NominalSourceBindingError {
    fn from(error: TypeFoundationBindingError) -> Self {
        match error {
            TypeFoundationBindingError::Resource(error) => Self::Resource(error),
            other => Self::Foundation(other),
        }
    }
}
impl fmt::Display for NominalSourceBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Foundation(error) => error.fmt(f),
            Self::Inventory(table) => write!(f, "nominal source inventory mismatch: {table}"),
            Self::MissingSource(owner) => write!(f, "missing nominal source {owner:?}"),
            Self::FieldOwner { owner, field } => write!(
                f,
                "field {field} is not declared by source struct {owner:?}"
            ),
            Self::MissingField(id) => write!(f, "missing artifact-owned struct field {id}"),
            Self::MissingVariant(id) => write!(f, "missing artifact-owned enum variant {id}"),
            Self::MissingVariantField(id) => {
                write!(f, "missing artifact-owned enum variant field {id}")
            }
            Self::MissingObject(id) => write!(f, "missing artifact-owned object value {id}"),
            Self::Origin(subject) => write!(
                f,
                "invalid nominal source definition origin for {subject:?}"
            ),
            Self::Identity(error) => write!(f, "invalid nominal source identity: {error}"),
            Self::Contract { owner, reason } => {
                write!(f, "invalid nominal source {owner:?}: {reason}")
            }
        }
    }
}
impl std::error::Error for NominalSourceBindingError {}
