use super::*;

/// Registry-approved intrinsic type declaration shapes. This table owns both
/// the raw annotation spelling accepted at the AST boundary and the complete
/// nominal declaration contract emitted as typed HIR.
pub const INTRINSIC_TYPE_REGISTRY: &[IntrinsicTypeSpec] = &[
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
    Integer(IntegerIntrinsicKind),
    PrimitiveUnary(PrimitiveUnaryKind),
    PrimitiveBinary(PrimitiveBinaryKind),
    ArrayAccess(ArrayAccessKind),
    Array(ArrayIntrinsic),
    Pointer(PointerIntrinsic),
}

impl IntrinsicFunctionKind {
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
            Self::Pointer(kind) => kind.name().to_string(),
        }
    }

    pub const fn integer_gc_effect(self) -> Option<GcEffect> {
        match self {
            Self::Integer(kind) => Some(kind.gc_effect()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intrinsic_type_registry_contains_each_integer_kind_once() {
        for kind in IntegerKind::ALL {
            let matches = INTRINSIC_TYPE_REGISTRY
                .iter()
                .filter(|spec| spec.kind == IntrinsicTypeKind::Integer(kind))
                .collect::<Vec<_>>();
            assert_eq!(matches.len(), 1);
            assert_eq!(matches[0].name, kind.intrinsic_name());
        }
    }

    #[test]
    fn pointer_intrinsic_families_are_registered_with_distinct_contracts() {
        let pointer = intrinsic_type_spec("core_ptr").expect("Ptr family is registered");
        assert_eq!(pointer.kind, IntrinsicTypeKind::Ptr);
        assert_eq!(pointer.kind.target(), IntrinsicTypeTarget::Struct);
        assert_eq!(
            pointer.kind.parameters(),
            IntrinsicTypeParameters::OneInvariantValue
        );

        let function_pointer =
            intrinsic_type_spec("core_fun_ptr").expect("FunPtr family is registered");
        assert_eq!(function_pointer.kind, IntrinsicTypeKind::FunPtr);
        assert_eq!(function_pointer.kind.target(), IntrinsicTypeTarget::Struct);
        assert_eq!(
            function_pointer.kind.parameters(),
            IntrinsicTypeParameters::OneInvariantUnconstrained
        );
    }

    #[test]
    fn integer_intrinsic_registry_is_total_and_effect_typed() {
        let mut count = 0;
        for kind in IntegerKind::ALL {
            for operation in NoGcIntegerOperation::ALL {
                let name = format!("{}_{}", kind.registry_key(), operation.registry_key());
                if operation.supports(kind) {
                    let entry = intrinsic_spec(&name).expect("supported operation is registered");
                    assert_eq!(
                        entry.kind(),
                        IntrinsicFunctionKind::Integer(IntegerIntrinsicKind::NoGcOperation {
                            kind,
                            operation,
                        })
                    );
                    assert!(entry.effects().no_gc);
                    count += 1;
                } else {
                    assert!(intrinsic_spec(&name).is_none());
                }
            }
            for operation in IntegerDivRem::ALL {
                let name = format!("{}_{}", kind.registry_key(), operation.registry_key());
                let entry = intrinsic_spec(&name).expect("div/rem is registered");
                assert_eq!(
                    entry.kind(),
                    IntrinsicFunctionKind::Integer(IntegerIntrinsicKind::ManagedOperation {
                        kind,
                        operation,
                    })
                );
                assert!(!entry.effects().no_gc);
                count += 1;
            }
            for target_kind in IntegerKind::ALL {
                let name = format!("{}_to_{}", kind.registry_key(), target_kind.registry_key());
                let entry = intrinsic_spec(&name).expect("conversion is registered");
                assert_eq!(
                    entry.kind(),
                    IntrinsicFunctionKind::Integer(IntegerIntrinsicKind::Conversion {
                        source: kind,
                        target_kind,
                    })
                );
                assert!(entry.effects().no_gc);
                count += 1;
            }
        }
        assert_eq!(count, 204);
    }
}
