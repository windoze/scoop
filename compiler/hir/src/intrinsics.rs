use super::*;
mod data_borrow;
pub use data_borrow::*;
mod registry;
#[cfg(test)]
mod tests;
mod wire;
pub use registry::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntrinsicStage {
    Hir,
    Mir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntrinsicFunctionKind {
    GcPinRaw,
    GcUnpinRaw,
    GcGetHandleRaw,
    GcReleaseHandleRaw,
    GcCollect,
    GcStats,
    CoroutineStart,
    CoroutineSuspend,
    CurrentSourceLocation,
    ForeignCallbackRegister,
    ForeignCallbackRetain,
    ForeignCallbackRelease,
    ForeignCallbackState,
    ForeignCallbackFailure,
    Integer(IntegerIntrinsicKind),
    PrimitiveUnary(PrimitiveUnaryKind),
    PrimitiveBinary(PrimitiveBinaryKind),
    ArrayAccess(ArrayAccessKind),
    Array(ArrayIntrinsic),
    Char(CharIntrinsic),
    Float(FloatIntrinsicKind),
    Pointer(PointerIntrinsic),
    DataBorrow(DataBorrowIntrinsic),
}

impl IntrinsicFunctionKind {
    /// Scalar equality operations with an ordinary callable entry in addition
    /// to their directly normalized call sites.
    pub const fn equality_member(self) -> Option<PrimitiveMemberIntrinsic> {
        match self {
            Self::Integer(
                kind @ IntegerIntrinsicKind::NoGcOperation {
                    operation: NoGcIntegerOperation::Equals,
                    ..
                },
            ) => Some(PrimitiveMemberIntrinsic::Integer(kind)),
            Self::Char(CharIntrinsic::Equals) => {
                Some(PrimitiveMemberIntrinsic::Char(CharIntrinsic::Equals))
            }
            Self::Float(
                kind @ FloatIntrinsicKind::Binary {
                    operation: FloatBinaryOperator::Equal,
                    ..
                },
            ) => Some(PrimitiveMemberIntrinsic::Float(kind)),
            _ => None,
        }
    }

    pub const fn is_foreign_callback(self) -> bool {
        matches!(
            self,
            Self::ForeignCallbackRegister
                | Self::ForeignCallbackRetain
                | Self::ForeignCallbackRelease
                | Self::ForeignCallbackState
                | Self::ForeignCallbackFailure
        )
    }

    pub const fn is_runtime_gc_call(self) -> bool {
        matches!(
            self,
            Self::GcPinRaw
                | Self::GcUnpinRaw
                | Self::GcGetHandleRaw
                | Self::GcReleaseHandleRaw
                | Self::GcCollect
                | Self::GcStats
        )
    }

    pub fn name(self) -> String {
        match self {
            Self::GcPinRaw => "gc_pin_raw".to_string(),
            Self::GcUnpinRaw => "gc_unpin_raw".to_string(),
            Self::GcGetHandleRaw => "gc_get_handle_raw".to_string(),
            Self::GcReleaseHandleRaw => "gc_release_handle_raw".to_string(),
            Self::GcCollect => "rt_gc_collect".to_string(),
            Self::GcStats => "rt_gc_stats".to_string(),
            Self::CoroutineStart => "coroutine_start".to_string(),
            Self::CoroutineSuspend => "coroutine_suspend".to_string(),
            Self::CurrentSourceLocation => "current_source_location".to_string(),
            Self::ForeignCallbackRegister => "foreign_callback_register".to_string(),
            Self::ForeignCallbackRetain => "foreign_callback_retain".to_string(),
            Self::ForeignCallbackRelease => "foreign_callback_release".to_string(),
            Self::ForeignCallbackState => "foreign_callback_state".to_string(),
            Self::ForeignCallbackFailure => "foreign_callback_failure".to_string(),
            Self::Integer(kind) => kind.name(),
            Self::PrimitiveUnary(kind) => kind.name().to_string(),
            Self::PrimitiveBinary(kind) => kind.name().to_string(),
            Self::ArrayAccess(kind) => kind.name().to_string(),
            Self::Array(kind) => kind.name().to_string(),
            Self::Char(kind) => kind.name().to_string(),
            Self::Float(kind) => kind.name(),
            Self::Pointer(kind) => kind.name().to_string(),
            Self::DataBorrow(kind) => kind.name().to_string(),
        }
    }

    pub const fn integer_gc_effect(self) -> Option<GcEffect> {
        match self {
            Self::Integer(kind) => Some(kind.gc_effect()),
            _ => None,
        }
    }
}

/// Primitive member operations shared by calls and bound reference invokes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveMemberIntrinsic {
    Char(CharIntrinsic),
    Float(FloatIntrinsicKind),
    Integer(IntegerIntrinsicKind),
    Unary(PrimitiveUnaryKind),
    Binary(PrimitiveBinaryKind),
}

impl PrimitiveMemberIntrinsic {
    pub const fn requires_arithmetic_exception(self) -> bool {
        matches!(
            self,
            Self::Integer(IntegerIntrinsicKind::ManagedOperation { .. })
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntegerIntrinsicKind {
    NoGcOperation {
        kind: IntegerKind,
        operation: NoGcIntegerOperation,
    },
    ManagedOperation {
        kind: IntegerKind,
        operation: IntegerDivRem,
    },
    Conversion {
        source: IntegerKind,
        target_kind: IntegerKind,
    },
}

impl IntegerIntrinsicKind {
    pub fn name(self) -> String {
        match self {
            Self::NoGcOperation { kind, operation } => {
                format!("{}_{}", kind.registry_key(), operation.registry_key())
            }
            Self::ManagedOperation { kind, operation } => {
                format!("{}_{}", kind.registry_key(), operation.registry_key())
            }
            Self::Conversion {
                source,
                target_kind,
            } => format!(
                "{}_to_{}",
                source.registry_key(),
                target_kind.registry_key()
            ),
        }
    }

    pub const fn gc_effect(self) -> GcEffect {
        match self {
            Self::NoGcOperation { .. } | Self::Conversion { .. } => GcEffect::NoGc,
            Self::ManagedOperation { .. } => GcEffect::Managed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrimitiveUnaryKind {
    BooleanNot,
}

impl PrimitiveUnaryKind {
    pub const ALL: [Self; 1] = [Self::BooleanNot];

    pub const fn name(self) -> &'static str {
        match self {
            Self::BooleanNot => "boolean_not",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrimitiveBinaryKind {
    StringConcat,
    StringCompareTo,
}

impl PrimitiveBinaryKind {
    pub const ALL: [Self; 2] = [Self::StringConcat, Self::StringCompareTo];

    pub const fn name(self) -> &'static str {
        match self {
            Self::StringConcat => "string_concat",
            Self::StringCompareTo => "string_compare_to",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArrayAccessKind {
    ImmutableGet,
    MutableGet,
    MutableSet,
}

impl ArrayAccessKind {
    pub const ALL: [Self; 3] = [Self::ImmutableGet, Self::MutableGet, Self::MutableSet];

    pub const fn name(self) -> &'static str {
        match self {
            Self::ImmutableGet => "array_get",
            Self::MutableGet => "mutable_array_get",
            Self::MutableSet => "mutable_array_set",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArrayIntrinsic {
    ToImmutable,
    ToMutable,
    ImmutableLength,
    MutableLength,
}

impl ArrayIntrinsic {
    pub const ALL: [Self; 4] = [
        Self::ToImmutable,
        Self::ToMutable,
        Self::ImmutableLength,
        Self::MutableLength,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::ToImmutable => "array_to_immutable",
            Self::ToMutable => "array_to_mutable",
            Self::ImmutableLength => "array_length",
            Self::MutableLength => "mutable_array_length",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PointerIntrinsic {
    ToULong,
    Cast,
    Load,
    LoadOffset,
    Store,
    StoreOffset,
    Plus,
    Minus,
    AddressOf,
    SizeOf,
    AlignOf,
}

impl PointerIntrinsic {
    pub const ALL: [Self; 11] = [
        Self::ToULong,
        Self::Cast,
        Self::Load,
        Self::LoadOffset,
        Self::Store,
        Self::StoreOffset,
        Self::Plus,
        Self::Minus,
        Self::AddressOf,
        Self::SizeOf,
        Self::AlignOf,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::ToULong => "ptr_to_ulong",
            Self::Cast => "ptr_cast",
            Self::Load => "ptr_load",
            Self::LoadOffset => "ptr_load_offset",
            Self::Store => "ptr_store",
            Self::StoreOffset => "ptr_store_offset",
            Self::Plus => "ptr_plus",
            Self::Minus => "ptr_minus",
            Self::AddressOf => "address_of",
            Self::SizeOf => "size_of",
            Self::AlignOf => "align_of",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntrinsicTarget {
    TopLevel,
    Member,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntrinsicEffects {
    pub no_gc: bool,
    pub unsafe_: bool,
}

impl IntrinsicEffects {
    pub const NONE: Self = Self {
        no_gc: false,
        unsafe_: false,
    };
    pub const NO_GC: Self = Self {
        no_gc: true,
        unsafe_: false,
    };
    pub const UNSAFE: Self = Self {
        no_gc: false,
        unsafe_: true,
    };
    pub const NO_GC_UNSAFE: Self = Self {
        no_gc: true,
        unsafe_: true,
    };
}

mod characters;
pub use characters::CharIntrinsic;
