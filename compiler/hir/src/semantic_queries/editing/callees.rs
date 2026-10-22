use crate::{EnumVariantRef, SemanticCallable, SemanticConstructor};

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
