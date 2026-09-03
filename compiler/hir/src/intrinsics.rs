use super::*;

/// Registry-approved intrinsic type declaration shapes. This table owns both
/// the raw annotation spelling accepted at the AST boundary and the complete
/// nominal declaration contract emitted as typed HIR.
pub const INTRINSIC_TYPE_REGISTRY: &[IntrinsicTypeSpec] = &[
    IntrinsicTypeSpec {
        name: "core_int",
        kind: IntrinsicTypeKind::Int,
    },
    IntrinsicTypeSpec {
        name: "core_uint",
        kind: IntrinsicTypeKind::UInt,
    },
    IntrinsicTypeSpec {
        name: "core_boolean",
        kind: IntrinsicTypeKind::Boolean,
    },
    IntrinsicTypeSpec {
        name: "core_string",
        kind: IntrinsicTypeKind::String,
    },
    IntrinsicTypeSpec {
        name: "core_array",
        kind: IntrinsicTypeKind::Array,
    },
    IntrinsicTypeSpec {
        name: "core_mutable_array",
        kind: IntrinsicTypeKind::MutableArray,
    },
];

pub struct IntrinsicTypeSpec {
    pub name: &'static str,
    pub kind: IntrinsicTypeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntrinsicTypeTarget {
    Struct,
    Class,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntrinsicTypeParameters {
    None,
    OneInvariantUnconstrained,
}

pub fn intrinsic_type_spec(name: &str) -> Option<&'static IntrinsicTypeSpec> {
    INTRINSIC_TYPE_REGISTRY
        .iter()
        .find(|spec| spec.name == name)
}

/// The compiler's intrinsic registry (impl spec 2.10). Signature rules
/// live with hir-lower; this table is the single source of truth for
/// valid names, expansion stage, and backend kind.
pub const INTRINSIC_REGISTRY: &[IntrinsicSpec] = &[
    IntrinsicSpec {
        name: "gc_pin_raw",
        stage: IntrinsicStage::Mir,
        kind: IntrinsicFunctionKind::GcPinRaw,
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::UNSAFE,
    },
    IntrinsicSpec {
        name: "gc_unpin_raw",
        stage: IntrinsicStage::Mir,
        kind: IntrinsicFunctionKind::GcUnpinRaw,
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::UNSAFE,
    },
    IntrinsicSpec {
        name: "gc_get_handle_raw",
        stage: IntrinsicStage::Mir,
        kind: IntrinsicFunctionKind::GcGetHandleRaw,
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::UNSAFE,
    },
    IntrinsicSpec {
        name: "gc_release_handle_raw",
        stage: IntrinsicStage::Mir,
        kind: IntrinsicFunctionKind::GcReleaseHandleRaw,
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::UNSAFE,
    },
    IntrinsicSpec {
        name: "rt_gc_collect",
        stage: IntrinsicStage::Mir,
        kind: IntrinsicFunctionKind::GcCollect,
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::NONE,
    },
    IntrinsicSpec {
        name: "rt_gc_stats",
        stage: IntrinsicStage::Mir,
        kind: IntrinsicFunctionKind::GcStats,
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::NONE,
    },
    IntrinsicSpec {
        name: "coroutine_start",
        stage: IntrinsicStage::Mir,
        kind: IntrinsicFunctionKind::CoroutineStart,
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::NONE,
    },
    IntrinsicSpec {
        name: "coroutine_suspend",
        stage: IntrinsicStage::Mir,
        kind: IntrinsicFunctionKind::CoroutineSuspend,
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::NONE,
    },
    IntrinsicSpec {
        name: "current_source_location",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::CurrentSourceLocation,
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::NONE,
    },
    intrinsic_method(
        PrimitiveUnaryKind::IntUnaryPlus.name(),
        IntrinsicFunctionKind::PrimitiveUnary(PrimitiveUnaryKind::IntUnaryPlus),
    ),
    intrinsic_method(
        PrimitiveUnaryKind::IntUnaryMinus.name(),
        IntrinsicFunctionKind::PrimitiveUnary(PrimitiveUnaryKind::IntUnaryMinus),
    ),
    intrinsic_method(
        PrimitiveUnaryKind::IntInc.name(),
        IntrinsicFunctionKind::PrimitiveUnary(PrimitiveUnaryKind::IntInc),
    ),
    intrinsic_method(
        PrimitiveUnaryKind::IntDec.name(),
        IntrinsicFunctionKind::PrimitiveUnary(PrimitiveUnaryKind::IntDec),
    ),
    intrinsic_method(
        PrimitiveUnaryKind::UIntUnaryPlus.name(),
        IntrinsicFunctionKind::PrimitiveUnary(PrimitiveUnaryKind::UIntUnaryPlus),
    ),
    intrinsic_method(
        PrimitiveUnaryKind::UIntInc.name(),
        IntrinsicFunctionKind::PrimitiveUnary(PrimitiveUnaryKind::UIntInc),
    ),
    intrinsic_method(
        PrimitiveUnaryKind::UIntDec.name(),
        IntrinsicFunctionKind::PrimitiveUnary(PrimitiveUnaryKind::UIntDec),
    ),
    intrinsic_method(
        PrimitiveUnaryKind::BooleanNot.name(),
        IntrinsicFunctionKind::PrimitiveUnary(PrimitiveUnaryKind::BooleanNot),
    ),
    intrinsic_method(
        PrimitiveBinaryKind::IntAdd.name(),
        IntrinsicFunctionKind::PrimitiveBinary(PrimitiveBinaryKind::IntAdd),
    ),
    intrinsic_method(
        PrimitiveBinaryKind::IntSub.name(),
        IntrinsicFunctionKind::PrimitiveBinary(PrimitiveBinaryKind::IntSub),
    ),
    intrinsic_method(
        PrimitiveBinaryKind::IntMul.name(),
        IntrinsicFunctionKind::PrimitiveBinary(PrimitiveBinaryKind::IntMul),
    ),
    intrinsic_method(
        PrimitiveBinaryKind::IntDiv.name(),
        IntrinsicFunctionKind::PrimitiveBinary(PrimitiveBinaryKind::IntDiv),
    ),
    intrinsic_method(
        PrimitiveBinaryKind::IntRem.name(),
        IntrinsicFunctionKind::PrimitiveBinary(PrimitiveBinaryKind::IntRem),
    ),
    intrinsic_method(
        PrimitiveBinaryKind::IntCompareTo.name(),
        IntrinsicFunctionKind::PrimitiveBinary(PrimitiveBinaryKind::IntCompareTo),
    ),
    intrinsic_method(
        PrimitiveBinaryKind::UIntAdd.name(),
        IntrinsicFunctionKind::PrimitiveBinary(PrimitiveBinaryKind::UIntAdd),
    ),
    intrinsic_method(
        PrimitiveBinaryKind::UIntSub.name(),
        IntrinsicFunctionKind::PrimitiveBinary(PrimitiveBinaryKind::UIntSub),
    ),
    intrinsic_method(
        PrimitiveBinaryKind::UIntMul.name(),
        IntrinsicFunctionKind::PrimitiveBinary(PrimitiveBinaryKind::UIntMul),
    ),
    intrinsic_method(
        PrimitiveBinaryKind::UIntDiv.name(),
        IntrinsicFunctionKind::PrimitiveBinary(PrimitiveBinaryKind::UIntDiv),
    ),
    intrinsic_method(
        PrimitiveBinaryKind::UIntRem.name(),
        IntrinsicFunctionKind::PrimitiveBinary(PrimitiveBinaryKind::UIntRem),
    ),
    intrinsic_method(
        PrimitiveBinaryKind::UIntCompareTo.name(),
        IntrinsicFunctionKind::PrimitiveBinary(PrimitiveBinaryKind::UIntCompareTo),
    ),
    intrinsic_method(
        PrimitiveBinaryKind::StringConcat.name(),
        IntrinsicFunctionKind::PrimitiveBinary(PrimitiveBinaryKind::StringConcat),
    ),
    intrinsic_method(
        PrimitiveBinaryKind::StringCompareTo.name(),
        IntrinsicFunctionKind::PrimitiveBinary(PrimitiveBinaryKind::StringCompareTo),
    ),
    intrinsic_method(
        ArrayAccessKind::ImmutableGet.name(),
        IntrinsicFunctionKind::ArrayAccess(ArrayAccessKind::ImmutableGet),
    ),
    intrinsic_method(
        ArrayAccessKind::MutableGet.name(),
        IntrinsicFunctionKind::ArrayAccess(ArrayAccessKind::MutableGet),
    ),
    intrinsic_method(
        ArrayAccessKind::MutableSet.name(),
        IntrinsicFunctionKind::ArrayAccess(ArrayAccessKind::MutableSet),
    ),
    IntrinsicSpec {
        name: "ptr_to_uint",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::Pointer(PointerIntrinsic::ToUInt),
        target: IntrinsicTarget::Member,
        effects: IntrinsicEffects::NO_GC_UNSAFE,
    },
    IntrinsicSpec {
        name: "ptr_cast",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::Pointer(PointerIntrinsic::Cast),
        target: IntrinsicTarget::Member,
        effects: IntrinsicEffects::NO_GC_UNSAFE,
    },
    IntrinsicSpec {
        name: "ptr_load",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::Pointer(PointerIntrinsic::Load),
        target: IntrinsicTarget::Member,
        effects: IntrinsicEffects::NO_GC_UNSAFE,
    },
    IntrinsicSpec {
        name: "ptr_load_offset",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::Pointer(PointerIntrinsic::LoadOffset),
        target: IntrinsicTarget::Member,
        effects: IntrinsicEffects::NO_GC_UNSAFE,
    },
    IntrinsicSpec {
        name: "ptr_store",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::Pointer(PointerIntrinsic::Store),
        target: IntrinsicTarget::Member,
        effects: IntrinsicEffects::NO_GC_UNSAFE,
    },
    IntrinsicSpec {
        name: "ptr_store_offset",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::Pointer(PointerIntrinsic::StoreOffset),
        target: IntrinsicTarget::Member,
        effects: IntrinsicEffects::NO_GC_UNSAFE,
    },
    IntrinsicSpec {
        name: "ptr_plus",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::Pointer(PointerIntrinsic::Plus),
        target: IntrinsicTarget::Member,
        effects: IntrinsicEffects::NO_GC_UNSAFE,
    },
    IntrinsicSpec {
        name: "ptr_minus",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::Pointer(PointerIntrinsic::Minus),
        target: IntrinsicTarget::Member,
        effects: IntrinsicEffects::NO_GC_UNSAFE,
    },
    IntrinsicSpec {
        name: "address_of",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::Pointer(PointerIntrinsic::AddressOf),
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::UNSAFE,
    },
    IntrinsicSpec {
        name: "size_of",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::Pointer(PointerIntrinsic::SizeOf),
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::NO_GC,
    },
    IntrinsicSpec {
        name: "align_of",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::Pointer(PointerIntrinsic::AlignOf),
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::NO_GC,
    },
    IntrinsicSpec {
        name: "array_to_immutable",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::Array(ArrayIntrinsic::ToImmutable),
        target: IntrinsicTarget::Member,
        effects: IntrinsicEffects::NONE,
    },
    IntrinsicSpec {
        name: "array_to_mutable",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::Array(ArrayIntrinsic::ToMutable),
        target: IntrinsicTarget::Member,
        effects: IntrinsicEffects::NONE,
    },
    IntrinsicSpec {
        name: "foreign_callback_register",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::ForeignCallbackRegister,
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::UNSAFE,
    },
    IntrinsicSpec {
        name: "foreign_callback_retain",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::ForeignCallbackRetain,
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::UNSAFE,
    },
    IntrinsicSpec {
        name: "foreign_callback_release",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::ForeignCallbackRelease,
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::UNSAFE,
    },
    IntrinsicSpec {
        name: "foreign_callback_state",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::ForeignCallbackState,
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::UNSAFE,
    },
    IntrinsicSpec {
        name: "foreign_callback_failure",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::ForeignCallbackFailure,
        target: IntrinsicTarget::TopLevel,
        effects: IntrinsicEffects::UNSAFE,
    },
];

/// One entry of the intrinsic registry.
pub struct IntrinsicSpec {
    pub name: &'static str,
    pub stage: IntrinsicStage,
    pub kind: IntrinsicFunctionKind,
    pub target: IntrinsicTarget,
    pub effects: IntrinsicEffects,
}

const fn intrinsic_method(name: &'static str, kind: IntrinsicFunctionKind) -> IntrinsicSpec {
    IntrinsicSpec {
        name,
        stage: IntrinsicStage::Hir,
        kind,
        target: IntrinsicTarget::Member,
        effects: IntrinsicEffects::NONE,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntrinsicStage {
    Hir,
    Mir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    PrimitiveUnary(PrimitiveUnaryKind),
    PrimitiveBinary(PrimitiveBinaryKind),
    ArrayAccess(ArrayAccessKind),
    Array(ArrayIntrinsic),
    Pointer(PointerIntrinsic),
}

impl IntrinsicFunctionKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::GcPinRaw => "gc_pin_raw",
            Self::GcUnpinRaw => "gc_unpin_raw",
            Self::GcGetHandleRaw => "gc_get_handle_raw",
            Self::GcReleaseHandleRaw => "gc_release_handle_raw",
            Self::GcCollect => "rt_gc_collect",
            Self::GcStats => "rt_gc_stats",
            Self::CoroutineStart => "coroutine_start",
            Self::CoroutineSuspend => "coroutine_suspend",
            Self::CurrentSourceLocation => "current_source_location",
            Self::ForeignCallbackRegister => "foreign_callback_register",
            Self::ForeignCallbackRetain => "foreign_callback_retain",
            Self::ForeignCallbackRelease => "foreign_callback_release",
            Self::ForeignCallbackState => "foreign_callback_state",
            Self::ForeignCallbackFailure => "foreign_callback_failure",
            Self::PrimitiveUnary(kind) => kind.name(),
            Self::PrimitiveBinary(kind) => kind.name(),
            Self::ArrayAccess(kind) => kind.name(),
            Self::Array(kind) => kind.name(),
            Self::Pointer(kind) => kind.name(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitiveUnaryKind {
    IntUnaryPlus,
    IntUnaryMinus,
    IntInc,
    IntDec,
    UIntUnaryPlus,
    UIntInc,
    UIntDec,
    BooleanNot,
}

impl PrimitiveUnaryKind {
    pub const ALL: [Self; 8] = [
        Self::IntUnaryPlus,
        Self::IntUnaryMinus,
        Self::IntInc,
        Self::IntDec,
        Self::UIntUnaryPlus,
        Self::UIntInc,
        Self::UIntDec,
        Self::BooleanNot,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::IntUnaryPlus => "int_unary_plus",
            Self::IntUnaryMinus => "int_unary_minus",
            Self::IntInc => "int_inc",
            Self::IntDec => "int_dec",
            Self::UIntUnaryPlus => "uint_unary_plus",
            Self::UIntInc => "uint_inc",
            Self::UIntDec => "uint_dec",
            Self::BooleanNot => "boolean_not",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitiveBinaryKind {
    IntAdd,
    IntSub,
    IntMul,
    IntDiv,
    IntRem,
    IntCompareTo,
    UIntAdd,
    UIntSub,
    UIntMul,
    UIntDiv,
    UIntRem,
    UIntCompareTo,
    StringConcat,
    StringCompareTo,
}

impl PrimitiveBinaryKind {
    pub const ALL: [Self; 14] = [
        Self::IntAdd,
        Self::IntSub,
        Self::IntMul,
        Self::IntDiv,
        Self::IntRem,
        Self::IntCompareTo,
        Self::UIntAdd,
        Self::UIntSub,
        Self::UIntMul,
        Self::UIntDiv,
        Self::UIntRem,
        Self::UIntCompareTo,
        Self::StringConcat,
        Self::StringCompareTo,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::IntAdd => "int_add",
            Self::IntSub => "int_sub",
            Self::IntMul => "int_mul",
            Self::IntDiv => "int_div",
            Self::IntRem => "int_rem",
            Self::IntCompareTo => "int_compare_to",
            Self::UIntAdd => "uint_add",
            Self::UIntSub => "uint_sub",
            Self::UIntMul => "uint_mul",
            Self::UIntDiv => "uint_div",
            Self::UIntRem => "uint_rem",
            Self::UIntCompareTo => "uint_compare_to",
            Self::StringConcat => "string_concat",
            Self::StringCompareTo => "string_compare_to",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArrayIntrinsic {
    ToImmutable,
    ToMutable,
}

impl ArrayIntrinsic {
    pub const fn name(self) -> &'static str {
        match self {
            Self::ToImmutable => "array_to_immutable",
            Self::ToMutable => "array_to_mutable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointerIntrinsic {
    ToUInt,
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
    pub const fn name(self) -> &'static str {
        match self {
            Self::ToUInt => "ptr_to_uint",
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

pub fn intrinsic_spec(name: &str) -> Option<&'static IntrinsicSpec> {
    INTRINSIC_REGISTRY.iter().find(|spec| spec.name == name)
}
