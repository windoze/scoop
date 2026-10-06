//! Source intrinsic registry and complete semantic role inventory.

use super::*;

/// Registry-approved intrinsic type declaration shapes. This table owns both
/// the raw annotation spelling accepted at the AST boundary and the complete
/// nominal declaration contract emitted as typed HIR.
pub const INTRINSIC_TYPE_REGISTRY: &[IntrinsicTypeSpec] = &[
    IntrinsicTypeSpec {
        name: "core_float",
        kind: IntrinsicTypeKind::Float(crate::FloatKind::F32),
    },
    IntrinsicTypeSpec {
        name: "core_double",
        kind: IntrinsicTypeKind::Float(crate::FloatKind::F64),
    },
    IntrinsicTypeSpec {
        name: "core_unit",
        kind: IntrinsicTypeKind::Unit,
    },
    IntrinsicTypeSpec {
        name: "core_char",
        kind: IntrinsicTypeKind::Char,
    },
    IntrinsicTypeSpec {
        name: "core_int8",
        kind: IntrinsicTypeKind::Integer(IntegerKind::SIGNED_8),
    },
    IntrinsicTypeSpec {
        name: "core_int16",
        kind: IntrinsicTypeKind::Integer(IntegerKind::SIGNED_16),
    },
    IntrinsicTypeSpec {
        name: "core_int",
        kind: IntrinsicTypeKind::Integer(IntegerKind::SIGNED_32),
    },
    IntrinsicTypeSpec {
        name: "core_long",
        kind: IntrinsicTypeKind::Integer(IntegerKind::SIGNED_64),
    },
    IntrinsicTypeSpec {
        name: "core_uint8",
        kind: IntrinsicTypeKind::Integer(IntegerKind::UNSIGNED_8),
    },
    IntrinsicTypeSpec {
        name: "core_uint16",
        kind: IntrinsicTypeKind::Integer(IntegerKind::UNSIGNED_16),
    },
    IntrinsicTypeSpec {
        name: "core_uint",
        kind: IntrinsicTypeKind::Integer(IntegerKind::UNSIGNED_32),
    },
    IntrinsicTypeSpec {
        name: "core_ulong",
        kind: IntrinsicTypeKind::Integer(IntegerKind::UNSIGNED_64),
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
    IntrinsicTypeSpec {
        name: "core_ptr",
        kind: IntrinsicTypeKind::Ptr,
    },
    IntrinsicTypeSpec {
        name: "core_fun_ptr",
        kind: IntrinsicTypeKind::FunPtr,
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
    OneInvariantValue,
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
        name: "char_code",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::Char(CharIntrinsic::Code),
        target: IntrinsicTarget::Member,
        effects: IntrinsicEffects::NO_GC,
    },
    IntrinsicSpec {
        name: "char_from_code_unchecked",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::Char(CharIntrinsic::FromCodeUnchecked),
        target: IntrinsicTarget::Member,
        effects: IntrinsicEffects::NO_GC_UNSAFE,
    },
    IntrinsicSpec {
        name: "char_equals",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::Char(CharIntrinsic::Equals),
        target: IntrinsicTarget::Member,
        effects: IntrinsicEffects::NO_GC,
    },
    IntrinsicSpec {
        name: "char_compare_to",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::Char(CharIntrinsic::CompareTo),
        target: IntrinsicTarget::Member,
        effects: IntrinsicEffects::NO_GC,
    },
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
        PrimitiveUnaryKind::BooleanNot.name(),
        IntrinsicFunctionKind::PrimitiveUnary(PrimitiveUnaryKind::BooleanNot),
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
        name: "ptr_to_ulong",
        stage: IntrinsicStage::Hir,
        kind: IntrinsicFunctionKind::Pointer(PointerIntrinsic::ToULong),
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
    intrinsic_method(
        "array_length",
        IntrinsicFunctionKind::Array(ArrayIntrinsic::ImmutableLength),
    ),
    intrinsic_method(
        "mutable_array_length",
        IntrinsicFunctionKind::Array(ArrayIntrinsic::MutableLength),
    ),
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
#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone, Copy)]
pub enum IntrinsicRegistryEntry {
    Standard(&'static IntrinsicSpec),
    Integer(IntegerIntrinsicKind),
}

impl IntrinsicRegistryEntry {
    pub fn name(self) -> String {
        match self {
            Self::Standard(spec) => spec.name.to_string(),
            Self::Integer(kind) => kind.name(),
        }
    }

    pub const fn stage(self) -> IntrinsicStage {
        match self {
            Self::Standard(spec) => spec.stage,
            Self::Integer(_) => IntrinsicStage::Hir,
        }
    }

    pub const fn kind(self) -> IntrinsicFunctionKind {
        match self {
            Self::Standard(spec) => spec.kind,
            Self::Integer(kind) => IntrinsicFunctionKind::Integer(kind),
        }
    }

    pub const fn target(self) -> IntrinsicTarget {
        match self {
            Self::Standard(spec) => spec.target,
            Self::Integer(_) => IntrinsicTarget::Member,
        }
    }

    pub const fn effects(self) -> IntrinsicEffects {
        match self {
            Self::Standard(spec) => spec.effects,
            Self::Integer(kind) => match kind.gc_effect() {
                GcEffect::NoGc => IntrinsicEffects::NO_GC,
                GcEffect::Managed => IntrinsicEffects::NONE,
            },
        }
    }
}

pub fn intrinsic_spec(name: &str) -> Option<IntrinsicRegistryEntry> {
    if let Some(kind) = integer_intrinsic_kind(name) {
        return Some(IntrinsicRegistryEntry::Integer(kind));
    }
    INTRINSIC_REGISTRY
        .iter()
        .find(|spec| spec.name == name)
        .map(IntrinsicRegistryEntry::Standard)
}

/// Complete compiler-recognized intrinsic function role set in canonical
/// semantic-key order. Published callables carry their own typed kind;
/// this enumeration is not a second persistent operation table.
pub fn intrinsic_function_kinds() -> Vec<IntrinsicFunctionKind> {
    let mut kinds = INTRINSIC_REGISTRY
        .iter()
        .map(|spec| spec.kind)
        .collect::<Vec<_>>();
    for kind in IntegerKind::ALL {
        kinds.extend(
            NoGcIntegerOperation::ALL
                .into_iter()
                .filter(|operation| operation.supports(kind))
                .map(|operation| {
                    IntrinsicFunctionKind::Integer(IntegerIntrinsicKind::NoGcOperation {
                        kind,
                        operation,
                    })
                }),
        );
        kinds.extend(IntegerDivRem::ALL.into_iter().map(|operation| {
            IntrinsicFunctionKind::Integer(IntegerIntrinsicKind::ManagedOperation {
                kind,
                operation,
            })
        }));
        kinds.extend(IntegerKind::ALL.into_iter().map(|target_kind| {
            IntrinsicFunctionKind::Integer(IntegerIntrinsicKind::Conversion {
                source: kind,
                target_kind,
            })
        }));
    }
    kinds.sort_unstable();
    assert!(
        kinds.windows(2).all(|pair| pair[0] != pair[1]),
        "compiler intrinsic registry contains a duplicate semantic role"
    );
    kinds
}

/// Resolve an integer intrinsic annotation name to its closed semantic key.
/// Raw strings stop at this registry boundary and never enter HIR output.
pub fn integer_intrinsic_kind(name: &str) -> Option<IntegerIntrinsicKind> {
    for kind in IntegerKind::ALL {
        for operation in NoGcIntegerOperation::ALL {
            if operation.supports(kind)
                && name == format!("{}_{}", kind.registry_key(), operation.registry_key())
            {
                return Some(IntegerIntrinsicKind::NoGcOperation { kind, operation });
            }
        }
        for operation in IntegerDivRem::ALL {
            if name == format!("{}_{}", kind.registry_key(), operation.registry_key()) {
                return Some(IntegerIntrinsicKind::ManagedOperation { kind, operation });
            }
        }
        for target_kind in IntegerKind::ALL {
            if name == format!("{}_to_{}", kind.registry_key(), target_kind.registry_key()) {
                return Some(IntegerIntrinsicKind::Conversion {
                    source: kind,
                    target_kind,
                });
            }
        }
    }
    None
}
