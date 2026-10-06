use super::*;

/// Closed semantic identity of every compiler-represented nominal type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntrinsicTypeKind {
    Unit,
    Integer(IntegerKind),
    Boolean,
    Char,
    String,
    Array,
    MutableArray,
    Ptr,
    FunPtr,
}

impl IntrinsicTypeKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Unit => "core_unit",
            Self::Integer(kind) => kind.intrinsic_name(),
            Self::Boolean => "core_boolean",
            Self::Char => "core_char",
            Self::String => "core_string",
            Self::Array => "core_array",
            Self::MutableArray => "core_mutable_array",
            Self::Ptr => "core_ptr",
            Self::FunPtr => "core_fun_ptr",
        }
    }

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Unit => "Unit",
            Self::Integer(kind) => kind.canonical_name(),
            Self::Boolean => "Boolean",
            Self::Char => "Char",
            Self::String => "String",
            Self::Array => "Array",
            Self::MutableArray => "MutableArray",
            Self::Ptr => "Ptr",
            Self::FunPtr => "FunPtr",
        }
    }

    pub const fn target(self) -> IntrinsicTypeTarget {
        match self {
            Self::Unit
            | Self::Integer(_)
            | Self::Boolean
            | Self::Char
            | Self::Ptr
            | Self::FunPtr => IntrinsicTypeTarget::Struct,
            Self::String | Self::Array | Self::MutableArray => IntrinsicTypeTarget::Class,
        }
    }

    pub const fn parameters(self) -> IntrinsicTypeParameters {
        match self {
            Self::Unit | Self::Integer(_) | Self::Boolean | Self::Char | Self::String => {
                IntrinsicTypeParameters::None
            }
            Self::Array | Self::MutableArray => IntrinsicTypeParameters::OneInvariantUnconstrained,
            Self::Ptr => IntrinsicTypeParameters::OneInvariantValue,
            Self::FunPtr => IntrinsicTypeParameters::OneInvariantUnconstrained,
        }
    }

    pub fn application(self, arguments: &[TypeId]) -> IntrinsicTypeRepresentation {
        match (self, arguments) {
            (Self::Unit, []) => IntrinsicTypeRepresentation::Unit,
            (Self::Integer(kind), []) => IntrinsicTypeRepresentation::Integer(kind),
            (Self::Boolean, []) => IntrinsicTypeRepresentation::Boolean,
            (Self::Char, []) => IntrinsicTypeRepresentation::Char,
            (Self::String, []) => IntrinsicTypeRepresentation::String,
            (Self::Array, [element]) => IntrinsicTypeRepresentation::Array { element: *element },
            (Self::MutableArray, [element]) => {
                IntrinsicTypeRepresentation::MutableArray { element: *element }
            }
            (Self::Ptr, [pointee]) => IntrinsicTypeRepresentation::Ptr { pointee: *pointee },
            (Self::FunPtr, [function]) => IntrinsicTypeRepresentation::FunPtr {
                function: *function,
            },
            _ => unreachable!("HIR validates the intrinsic declaration contract before use"),
        }
    }
}

/// Complete compiler representation of one nominal application. Generic
/// family variants contain their concrete element type directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntrinsicTypeRepresentation {
    Unit,
    Integer(IntegerKind),
    Boolean,
    Char,
    String,
    Array { element: TypeId },
    MutableArray { element: TypeId },
    Ptr { pointee: TypeId },
    FunPtr { function: TypeId },
}
