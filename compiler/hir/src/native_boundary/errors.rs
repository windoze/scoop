use std::fmt;

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeBoundaryDefinitionError {
    ExpectedSourceField,
    ExpectedSourceVariant,
    ShapeKindMismatch,
    CAbiProjectionMismatch,
    IntrinsicArityMismatch { expected: u32, actual: u32 },
    FieldOwnerMismatch,
    VariantOwnerMismatch,
    VariantFieldOwnerMismatch,
    DuplicateField,
    DuplicateVariant,
    DuplicateVariantField,
    EmptyBinderStack,
    BinderStackHeadMismatch { expected: u32, actual: u32 },
    BinderDepthOutOfRange { depth: u32 },
    BinderIndexOutOfRange { depth: u32, index: u32, count: u32 },
    DeclarationIdentity(SourceDeclarationIdentityError),
    FieldIdentity(FieldIdentityError),
    VariantIdentity(EnumVariantIdentityError),
    Hash(HashError),
}

impl fmt::Display for NativeBoundaryDefinitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedSourceField => {
                formatter.write_str("native boundary field must be a source field")
            }
            Self::ExpectedSourceVariant => {
                formatter.write_str("native boundary variant must be a source enum variant")
            }
            Self::ShapeKindMismatch => {
                formatter.write_str("native boundary shape does not match the source nominal kind")
            }
            Self::CAbiProjectionMismatch => formatter
                .write_str("native C projection does not select the complete declared shape"),
            Self::IntrinsicArityMismatch { expected, actual } => write!(
                formatter,
                "native intrinsic parameter count mismatch: expected {expected}, found {actual}"
            ),
            Self::FieldOwnerMismatch => {
                formatter.write_str("native boundary field belongs to another nominal")
            }
            Self::VariantOwnerMismatch => {
                formatter.write_str("native boundary variant belongs to another enum")
            }
            Self::VariantFieldOwnerMismatch => {
                formatter.write_str("native boundary variant field belongs to another variant")
            }
            Self::DuplicateField => formatter.write_str("native boundary struct repeats a field"),
            Self::DuplicateVariant => formatter.write_str("native boundary enum repeats a variant"),
            Self::DuplicateVariantField => {
                formatter.write_str("native boundary enum variant repeats a field")
            }
            Self::EmptyBinderStack => {
                formatter.write_str("native boundary binder stack must contain its owner frame")
            }
            Self::BinderStackHeadMismatch { expected, actual } => write!(
                formatter,
                "native boundary owner binder count mismatch: expected {expected}, found {actual}"
            ),
            Self::BinderDepthOutOfRange { depth } => {
                write!(
                    formatter,
                    "native boundary binder depth {depth} is out of range"
                )
            }
            Self::BinderIndexOutOfRange {
                depth,
                index,
                count,
            } => write!(
                formatter,
                "native boundary binder index {index} is out of range for depth {depth} with {count} parameters"
            ),
            Self::DeclarationIdentity(error) => error.fmt(formatter),
            Self::FieldIdentity(error) => error.fmt(formatter),
            Self::VariantIdentity(error) => error.fmt(formatter),
            Self::Hash(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for NativeBoundaryDefinitionError {}
