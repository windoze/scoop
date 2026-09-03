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
            Self::Array(kind) => kind.name(),
            Self::Pointer(kind) => kind.name(),
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
