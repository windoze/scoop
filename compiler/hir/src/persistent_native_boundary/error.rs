use std::fmt;

use scoop_identity::{
    PersistentCallableApplicationId, PersistentExactTypeId, PersistentInitializationUnitId,
};

use crate::{
    HirSignatureTypeMappingError, NativeBoundaryDefinitionError, NativeBoundaryNominalOwner,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirNativeBoundaryTypeDefinitionError {
    DuplicateSourceNominal {
        owner: NativeBoundaryNominalOwner,
    },
    MissingSourceNominal {
        owner: NativeBoundaryNominalOwner,
    },
    MissingCallableApplication {
        application: PersistentCallableApplicationId,
    },
    MissingInitializationApplication {
        unit: PersistentInitializationUnitId,
    },
    MissingExactType {
        exact: PersistentExactTypeId,
    },
    UnexpectedGeneratedNominal,
    TooManyTypeParameters,
    SourceOrderOverflow,
    InvalidStructField {
        structure: u32,
        field: usize,
    },
    InvalidEnumVariant {
        enumeration: u32,
        variant: usize,
    },
    InvalidEnumVariantField {
        enumeration: u32,
        variant: usize,
        field: usize,
    },
    InvalidSignatureType(HirSignatureTypeMappingError),
    InvalidDefinition(NativeBoundaryDefinitionError),
}

impl fmt::Display for HirNativeBoundaryTypeDefinitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateSourceNominal { owner } => {
                write!(
                    formatter,
                    "native boundary source owner {owner:?} is duplicated"
                )
            }
            Self::MissingSourceNominal { owner } => write!(
                formatter,
                "native-boundary-closure-required: source owner {owner:?} has no definition in the current artifact"
            ),
            Self::MissingCallableApplication { application } => write!(
                formatter,
                "native boundary references missing callable application {application}"
            ),
            Self::MissingInitializationApplication { unit } => write!(
                formatter,
                "native boundary references missing initialization application {unit}"
            ),
            Self::MissingExactType { exact } => {
                write!(
                    formatter,
                    "native boundary references missing exact type {exact}"
                )
            }
            Self::UnexpectedGeneratedNominal => formatter
                .write_str("native boundary type witness requires a source nominal declaration"),
            Self::TooManyTypeParameters => formatter
                .write_str("native boundary nominal has more than u32::MAX type parameters"),
            Self::SourceOrderOverflow => formatter
                .write_str("native boundary source declaration order exceeds u32::MAX entries"),
            Self::InvalidStructField { structure, field } => write!(
                formatter,
                "native boundary struct {structure} has invalid source field {field}"
            ),
            Self::InvalidEnumVariant {
                enumeration,
                variant,
            } => write!(
                formatter,
                "native boundary enum {enumeration} has invalid source variant {variant}"
            ),
            Self::InvalidEnumVariantField {
                enumeration,
                variant,
                field,
            } => write!(
                formatter,
                "native boundary enum {enumeration} variant {variant} has invalid source field {field}"
            ),
            Self::InvalidSignatureType(error) => error.fmt(formatter),
            Self::InvalidDefinition(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for HirNativeBoundaryTypeDefinitionError {}
