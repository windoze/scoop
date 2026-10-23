use crate::{EnumVariantRef, SemanticCallable, SemanticConstructor};

/// A unit variant retained by successful static initializer evaluation.
#[derive(Clone, Debug)]
pub struct EditingStaticVariantReference {
    pub origin: crate::SemanticSourceLocation,
    pub target: SemanticVariant,
    pub name: String,
    pub value_type: crate::TypeId,
    pub definition: Option<crate::SemanticSourceLocation>,
    pub imports: Vec<crate::EditingImportSource>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticVariant {
    Current(EnumVariantRef),
    Imported(scoop_identity::PersistentEnumVariantId),
}

impl From<SemanticVariant> for EditingCallee {
    fn from(variant: SemanticVariant) -> Self {
        match variant {
            SemanticVariant::Current(variant) => Self::CurrentVariant(variant),
            SemanticVariant::Imported(variant) => Self::ImportedVariant(variant),
        }
    }
}

/// The declaration selected by a source call or callable reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditingCallee {
    Callable(SemanticCallable),
    Constructor(SemanticConstructor),
    CurrentVariant(EnumVariantRef),
    ImportedVariant(scoop_identity::PersistentEnumVariantId),
}

impl EditingCallee {
    pub fn callable(self) -> Option<SemanticCallable> {
        match self {
            Self::Callable(callable) => Some(callable),
            Self::Constructor(_) | Self::CurrentVariant(_) | Self::ImportedVariant(_) => None,
        }
    }
}

impl From<SemanticCallable> for EditingCallee {
    fn from(callable: SemanticCallable) -> Self {
        Self::Callable(callable)
    }
}
