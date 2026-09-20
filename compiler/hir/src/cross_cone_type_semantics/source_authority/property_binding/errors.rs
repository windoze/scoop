use super::*;
use std::fmt;

#[derive(Debug)]
pub enum InheritancePropertyBindingError {
    Resource(WireError),
    Foundation(TypeFoundationBindingError),
    Dispatch(InheritanceDispatchBindingError),
    Inheritance(InheritanceGraphError<TypeFoundationBindingError>),
    Domain(AccessDomainSemanticError),
    Identity(String),
    Inventory,
    GenericOwner,
    MissingKey(PersistentPropertyId),
    MissingSource(PersistentPropertyId),
    NonRuntime(PersistentPropertyId),
    Owner(PersistentPropertyId),
    Accessor(PersistentPropertyAccessorId),
    Visibility(PersistentPropertyId),
    DefinitionOrigin(DefinitionOriginSubject),
    Access {
        declaration: PersistentPropertyId,
        reason: String,
    },
    Signature(PersistentPropertyId),
    Slots(PersistentPropertyId),
    SetterDomain(PersistentPropertyId),
}

impl From<WireError> for InheritancePropertyBindingError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<TypeFoundationBindingError> for InheritancePropertyBindingError {
    fn from(error: TypeFoundationBindingError) -> Self {
        match error {
            TypeFoundationBindingError::Resource(error) => Self::Resource(error),
            other => Self::Foundation(other),
        }
    }
}
impl From<InheritanceDispatchBindingError> for InheritancePropertyBindingError {
    fn from(error: InheritanceDispatchBindingError) -> Self {
        match error {
            InheritanceDispatchBindingError::Resource(error) => Self::Resource(error),
            other => Self::Dispatch(other),
        }
    }
}
impl From<AccessDomainSemanticError> for InheritancePropertyBindingError {
    fn from(error: AccessDomainSemanticError) -> Self {
        match error {
            AccessDomainSemanticError::Resource(error) => Self::Resource(error),
            other => Self::Domain(other),
        }
    }
}
impl fmt::Display for InheritancePropertyBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Foundation(e) => e.fmt(f),
            Self::Dispatch(e) => e.fmt(f),
            Self::Inheritance(e) => e.fmt(f),
            Self::Domain(e) => e.fmt(f),
            Self::Identity(e) => e.fmt(f),
            Self::Inventory => {
                f.write_str("inheritance property sources differ from the required inventory")
            }
            Self::GenericOwner => {
                f.write_str("inheritance property source requires a param-free owner")
            }
            Self::MissingKey(id) => write!(f, "property {id} is absent from the owning foundation"),
            Self::MissingSource(id) => write!(f, "missing inheritance property source {id}"),
            Self::NonRuntime(id) => {
                write!(f, "inheritance property {id} is not a runtime property")
            }
            Self::Owner(id) => write!(f, "property {id} has an inconsistent source owner"),
            Self::Accessor(id) => write!(
                f,
                "accessor {id} has an inconsistent logical property or role"
            ),
            Self::Visibility(id) => write!(
                f,
                "property {id} disagrees with required accessor visibility"
            ),
            Self::DefinitionOrigin(id) => {
                write!(f, "property source differs from foundation origin {id:?}")
            }
            Self::Access {
                declaration,
                reason,
            } => write!(
                f,
                "invalid property source access for {declaration}: {reason}"
            ),
            Self::Signature(id) => write!(
                f,
                "property {id} differs from its dispatch accessor signature"
            ),
            Self::Slots(id) => write!(f, "property {id} differs from its original accessor slots"),
            Self::SetterDomain(id) => {
                write!(f, "property {id} setter widens the getter lookup domain")
            }
        }
    }
}
impl std::error::Error for InheritancePropertyBindingError {}
