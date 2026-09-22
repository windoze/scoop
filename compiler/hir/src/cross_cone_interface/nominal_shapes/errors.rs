use std::fmt;

use scoop_identity::{PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentFieldId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnumSourceVariantBuildError {
    TooManyFields,
    UnitFields,
    DuplicateField(PersistentEnumVariantFieldId),
}

impl fmt::Display for EnumSourceVariantBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyFields => formatter.write_str("enum variant field count exceeds u32"),
            Self::UnitFields => formatter.write_str("unit enum variant must not contain fields"),
            Self::DuplicateField(field) => {
                write!(formatter, "duplicate enum variant field identity {field}")
            }
        }
    }
}

impl std::error::Error for EnumSourceVariantBuildError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NominalSourceShapeBuildError {
    TooManyStructFields,
    EmptyCLayout,
    DuplicateStructField(PersistentFieldId),
    TooManyEnumVariants,
    DuplicateEnumVariant(PersistentEnumVariantId),
}

impl fmt::Display for NominalSourceShapeBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCLayout => {
                formatter.write_str("CLayout struct must declare at least one field")
            }
            Self::TooManyStructFields => formatter.write_str("struct field count exceeds u32"),
            Self::DuplicateStructField(field) => {
                write!(formatter, "duplicate struct field identity {field}")
            }
            Self::TooManyEnumVariants => formatter.write_str("enum variant count exceeds u32"),
            Self::DuplicateEnumVariant(variant) => {
                write!(formatter, "duplicate enum variant identity {variant}")
            }
        }
    }
}

impl std::error::Error for NominalSourceShapeBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum StructSourceFieldResolutionError<E> {
    Field(E),
    ValueType(E),
}

impl<E: fmt::Display> fmt::Display for StructSourceFieldResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Field(error) => write!(formatter, "invalid struct field identity: {error}"),
            Self::ValueType(error) => write!(formatter, "invalid struct field type: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for StructSourceFieldResolutionError<E> {}

#[derive(Debug, Eq, PartialEq)]
pub enum EnumSourceFieldResolutionError<E> {
    Field(E),
    ValueType(E),
}

impl<E: fmt::Display> fmt::Display for EnumSourceFieldResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Field(error) => write!(formatter, "invalid enum field identity: {error}"),
            Self::ValueType(error) => write!(formatter, "invalid enum field type: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for EnumSourceFieldResolutionError<E> {}

#[derive(Debug, Eq, PartialEq)]
pub enum EnumSourceVariantResolutionError<E> {
    Variant(E),
    TooManyFields,
    UnitFields,
    Field {
        index: usize,
        error: EnumSourceFieldResolutionError<E>,
    },
    DuplicateField {
        index: usize,
        field: PersistentEnumVariantFieldId,
    },
}

impl<E: fmt::Display> fmt::Display for EnumSourceVariantResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Variant(error) => write!(formatter, "invalid enum variant identity: {error}"),
            Self::TooManyFields => formatter.write_str("enum variant field count exceeds u32"),
            Self::UnitFields => formatter.write_str("unit enum variant must not contain fields"),
            Self::Field { index, error } => {
                write!(formatter, "invalid enum field {index}: {error}")
            }
            Self::DuplicateField { index, field } => {
                write!(formatter, "duplicate enum field {field} at index {index}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for EnumSourceVariantResolutionError<E> {}

#[derive(Debug, Eq, PartialEq)]
pub enum NominalSourceShapeResolutionError<E> {
    TooManyStructFields,
    EmptyCLayout,
    StructField {
        index: usize,
        error: StructSourceFieldResolutionError<E>,
    },
    DuplicateStructField {
        index: usize,
        field: PersistentFieldId,
    },
    TooManyEnumVariants,
    EnumVariant {
        index: usize,
        error: EnumSourceVariantResolutionError<E>,
    },
    DuplicateEnumVariant {
        index: usize,
        variant: PersistentEnumVariantId,
    },
    ObjectValue(E),
}

impl<E: fmt::Display> fmt::Display for NominalSourceShapeResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCLayout => {
                formatter.write_str("CLayout struct must declare at least one field")
            }
            Self::TooManyStructFields => formatter.write_str("struct field count exceeds u32"),
            Self::StructField { index, error } => {
                write!(formatter, "invalid struct field {index}: {error}")
            }
            Self::DuplicateStructField { index, field } => {
                write!(formatter, "duplicate struct field {field} at index {index}")
            }
            Self::TooManyEnumVariants => formatter.write_str("enum variant count exceeds u32"),
            Self::EnumVariant { index, error } => {
                write!(formatter, "invalid enum variant {index}: {error}")
            }
            Self::DuplicateEnumVariant { index, variant } => {
                write!(
                    formatter,
                    "duplicate enum variant {variant} at index {index}"
                )
            }
            Self::ObjectValue(error) => write!(formatter, "invalid object value identity: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for NominalSourceShapeResolutionError<E> {}
