use super::*;
use scoop_wire::WireError;
use std::fmt;

#[derive(Debug)]
pub enum InheritanceSourceBindingError {
    Resource(WireError),
    Foundation(Box<TypeFoundationBindingError>),
    Constructors(Box<InheritanceConstructorBindingError>),
    Protected(Box<InheritanceProtectedCallableBindingError>),
    Nominal(Box<NominalSourceBindingError>),
    Slots(Box<InheritanceSlotSourceBindingError>),
    ConstructorSemantics(Box<NominalSupportCallableSemanticError<Self>>),
    FoundationMismatch,
    Inventory,
    MissingOwner(PersistentExactTypeId),
    Modality(SourceNominalId),
}
impl From<WireError> for InheritanceSourceBindingError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
macro_rules! convert {
    ($source:ident, $variant:ident) => {
        impl From<$source> for InheritanceSourceBindingError {
            fn from(error: $source) -> Self {
                match error {
                    $source::Resource(error) => Self::Resource(error),
                    other => Self::$variant(Box::new(other)),
                }
            }
        }
    };
}
convert!(TypeFoundationBindingError, Foundation);
convert!(InheritanceConstructorBindingError, Constructors);
convert!(InheritanceProtectedCallableBindingError, Protected);
convert!(NominalSourceBindingError, Nominal);
convert!(InheritanceSlotSourceBindingError, Slots);

impl InheritanceSourceBindingError {
    pub(super) fn from_constructor(error: NominalSupportCallableSemanticError<Self>) -> Self {
        match error {
            NominalSupportCallableSemanticError::Foundation(error)
            | NominalSupportCallableSemanticError::Signature(
                ProtectedCallableSemanticError::Foundation(error),
            ) => error,
            NominalSupportCallableSemanticError::Signature(
                ProtectedCallableSemanticError::Resource(error),
            )
            | NominalSupportCallableSemanticError::Signature(
                ProtectedCallableSemanticError::Signature(
                    MeteredSignatureTypeSemanticError::Resource(error),
                ),
            )
            | NominalSupportCallableSemanticError::Signature(
                ProtectedCallableSemanticError::Source(InheritanceGraphError::Resource(error)),
            ) => Self::Resource(error),
            other => Self::ConstructorSemantics(Box::new(other)),
        }
    }
}
impl fmt::Display for InheritanceSourceBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Foundation(error) => error.fmt(f),
            Self::Constructors(error) => error.fmt(f),
            Self::Protected(error) => error.fmt(f),
            Self::Nominal(error) => error.fmt(f),
            Self::Slots(error) => error.fmt(f),
            Self::ConstructorSemantics(error) => error.fmt(f),
            Self::FoundationMismatch => {
                f.write_str("inheritance sources use different foundation bindings")
            }
            Self::Inventory => {
                f.write_str("inheritance sources have different declaration inventories")
            }
            Self::MissingOwner(owner) => write!(f, "missing inheritance source owner {owner}"),
            Self::Modality(owner) => write!(
                f,
                "nominal source {owner:?} has a different inheritance modality"
            ),
        }
    }
}
impl std::error::Error for InheritanceSourceBindingError {}
