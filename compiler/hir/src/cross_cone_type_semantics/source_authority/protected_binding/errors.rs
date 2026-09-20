use super::*;
use std::fmt;

#[derive(Debug)]
pub enum InheritanceProtectedCallableBindingError {
    Resource(WireError),
    Foundation(TypeFoundationBindingError),
    Property(Box<InheritancePropertyBindingError>),
    Inheritance(InheritanceGraphError<TypeFoundationBindingError>),
    Semantic(Box<ProtectedCallableSemanticError<Self>>),
    Identity(String),
    Inventory,
    CoreUnit,
    MissingKey(CallableTemplateOrigin),
    MissingSource(CallableTemplateOrigin),
    MissingProperty(PersistentPropertyId),
    RepeatedOwner(CallableTemplateOrigin),
    Owner(CallableTemplateOrigin),
    Declaration(CallableTemplateOrigin),
    DefinitionOrigin(DefinitionOriginSubject),
    AccessorAccess(PersistentPropertyAccessorId),
    NominalKind,
}
impl From<WireError> for InheritanceProtectedCallableBindingError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<TypeFoundationBindingError> for InheritanceProtectedCallableBindingError {
    fn from(error: TypeFoundationBindingError) -> Self {
        match error {
            TypeFoundationBindingError::Resource(error) => Self::Resource(error),
            other => Self::Foundation(other),
        }
    }
}
impl From<InheritancePropertyBindingError> for InheritanceProtectedCallableBindingError {
    fn from(error: InheritancePropertyBindingError) -> Self {
        match error {
            InheritancePropertyBindingError::Resource(error) => Self::Resource(error),
            other => Self::Property(Box::new(other)),
        }
    }
}
impl fmt::Display for InheritanceProtectedCallableBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Foundation(e) => e.fmt(f),
            Self::Property(e) => e.fmt(f),
            Self::Inheritance(e) => e.fmt(f),
            Self::Semantic(e) => e.fmt(f),
            Self::Identity(e) => e.fmt(f),
            Self::Inventory => {
                f.write_str("protected callable sources differ from the required inventory")
            }
            Self::CoreUnit => f.write_str("protected callable Unit role is not from trusted core"),
            Self::MissingKey(id) => write!(
                f,
                "protected callable {id:?} is absent from the owning foundation"
            ),
            Self::MissingSource(id) => write!(f, "missing protected callable source {id:?}"),
            Self::MissingProperty(id) => write!(f, "missing bound property source {id}"),
            Self::RepeatedOwner(id) => write!(
                f,
                "protected callable {id:?} belongs to multiple inventory owners"
            ),
            Self::Owner(id) => write!(f, "protected callable {id:?} differs from its source owner"),
            Self::Declaration(id) => write!(f, "invalid protected callable source role {id:?}"),
            Self::DefinitionOrigin(id) => write!(
                f,
                "protected callable origin differs from foundation subject {id:?}"
            ),
            Self::AccessorAccess(id) => write!(
                f,
                "protected accessor {id} differs from bound property access"
            ),
            Self::NominalKind => {
                f.write_str("signature nominal key has another source declaration kind")
            }
        }
    }
}
impl std::error::Error for InheritanceProtectedCallableBindingError {}
