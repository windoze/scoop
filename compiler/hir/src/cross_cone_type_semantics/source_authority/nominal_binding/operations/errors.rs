use super::*;

#[derive(Debug)]
pub enum DefaultSourceNominalOperationError {
    Resource(WireError),
    Nominal(Box<NominalSourceBindingError>),
    Target(Box<DefaultSourceTargetSubjectError>),
    NonNominalOwner,
    Kind {
        owner: SourceNominalId,
        expected: PublicNominalKindV1,
        actual: PublicNominalKindV1,
    },
    Arity {
        owner: SourceNominalId,
        expected: u32,
        actual: usize,
    },
    CallableArity {
        owner: SourceNominalId,
        expected: u32,
        actual: usize,
    },
    StructRepresentation(SourceNominalId),
    VariantOwner {
        variant: PersistentEnumVariantId,
        owner: SourceNominalId,
    },
    Singleton(PersistentObjectValueId),
    FieldPosition,
    Mapping(BinderUseListBuildError),
    Binders(DefaultTemplateProviderShapeBuildError),
    Substitution(DefaultTemplateTypeSubstitutionError),
}
impl Error {
    pub(super) fn transform(error: MeteredDefaultTemplateTypeSubstitutionError) -> Self {
        match error {
            MeteredDefaultTemplateTypeSubstitutionError::Resource(error) => Self::Resource(error),
            MeteredDefaultTemplateTypeSubstitutionError::Substitution(error) => {
                Self::Substitution(error)
            }
        }
    }
    pub(super) fn target(error: DefaultSourceTargetSubjectError) -> Self {
        match error {
            DefaultSourceTargetSubjectError::Resource(error) => Self::Resource(error),
            error => Self::Target(Box::new(error)),
        }
    }
}
impl From<WireError> for Error {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<NominalSourceBindingError> for Error {
    fn from(error: NominalSourceBindingError) -> Self {
        match error {
            NominalSourceBindingError::Resource(error) => Self::Resource(error),
            error => Self::Nominal(Box::new(error)),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Nominal(error) => error.fmt(f),
            Self::Target(error) => error.fmt(f),
            Self::Mapping(error) => error.fmt(f),
            Self::Binders(error) => error.fmt(f),
            Self::Substitution(error) => error.fmt(f),
            Self::NonNominalOwner => {
                f.write_str("default nominal operation owner is not a source nominal type")
            }
            Self::Kind {
                owner,
                expected,
                actual,
            } => write!(
                f,
                "default operation owner {owner:?} has kind {actual:?}, expected {expected:?}"
            ),
            Self::Arity {
                owner,
                expected,
                actual,
            } => write!(
                f,
                "default operation owner {owner:?} requires {expected} arguments, got {actual}"
            ),
            Self::CallableArity {
                owner,
                expected,
                actual,
            } => write!(
                f,
                "default callable under {owner:?} requires {expected} own arguments, got {actual}"
            ),
            Self::StructRepresentation(owner) => write!(
                f,
                "default struct operation requires ordinary source representation for {owner:?}"
            ),
            Self::VariantOwner { variant, owner } => write!(
                f,
                "default enum variant {variant} does not belong to {owner:?}"
            ),
            Self::Singleton(value) => write!(
                f,
                "default singleton {value} does not identify its source object"
            ),
            Self::FieldPosition => f.write_str("default source field position exceeds u32"),
        }
    }
}
impl std::error::Error for Error {}
