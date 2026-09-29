use super::*;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

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
    Pointer(PointerIntrinsic),
}

impl WireEncode for IntrinsicFunctionKind {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::GcPinRaw => encode_empty_sum(encoder, 1),
            Self::GcUnpinRaw => encode_empty_sum(encoder, 2),
            Self::GcGetHandleRaw => encode_empty_sum(encoder, 3),
            Self::GcReleaseHandleRaw => encode_empty_sum(encoder, 4),
            Self::GcCollect => encode_empty_sum(encoder, 5),
            Self::GcStats => encode_empty_sum(encoder, 6),
            Self::CoroutineStart => encode_empty_sum(encoder, 7),
            Self::CoroutineSuspend => encode_empty_sum(encoder, 8),
            Self::CurrentSourceLocation => encode_empty_sum(encoder, 9),
            Self::ForeignCallbackRegister => encode_empty_sum(encoder, 10),
            Self::ForeignCallbackRetain => encode_empty_sum(encoder, 11),
            Self::ForeignCallbackRelease => encode_empty_sum(encoder, 12),
            Self::ForeignCallbackState => encode_empty_sum(encoder, 13),
            Self::ForeignCallbackFailure => encode_empty_sum(encoder, 14),
            Self::Integer(kind) => encode_value_sum(encoder, 15, kind),
            Self::PrimitiveUnary(kind) => {
                encode_unsigned_value_sum(encoder, 16, primitive_unary_tag(*kind))
            }
            Self::PrimitiveBinary(kind) => {
                encode_unsigned_value_sum(encoder, 17, primitive_binary_tag(*kind))
            }
            Self::ArrayAccess(kind) => {
                encode_unsigned_value_sum(encoder, 18, array_access_tag(*kind))
            }
            Self::Array(kind) => encode_unsigned_value_sum(encoder, 19, array_tag(*kind)),
            Self::Pointer(kind) => encode_unsigned_value_sum(encoder, 20, pointer_tag(*kind)),
        }
    }
}

impl WireDecode for IntrinsicFunctionKind {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        if fields == 0 {
            return Err(intrinsic_wire_error(
                decoder,
                WireErrorKind::MissingField { field: 0 },
            ));
        }
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => decode_empty_intrinsic(decoder, fields, Self::GcPinRaw),
            2 => decode_empty_intrinsic(decoder, fields, Self::GcUnpinRaw),
            3 => decode_empty_intrinsic(decoder, fields, Self::GcGetHandleRaw),
            4 => decode_empty_intrinsic(decoder, fields, Self::GcReleaseHandleRaw),
            5 => decode_empty_intrinsic(decoder, fields, Self::GcCollect),
            6 => decode_empty_intrinsic(decoder, fields, Self::GcStats),
            7 => decode_empty_intrinsic(decoder, fields, Self::CoroutineStart),
            8 => decode_empty_intrinsic(decoder, fields, Self::CoroutineSuspend),
            9 => decode_empty_intrinsic(decoder, fields, Self::CurrentSourceLocation),
            10 => decode_empty_intrinsic(decoder, fields, Self::ForeignCallbackRegister),
            11 => decode_empty_intrinsic(decoder, fields, Self::ForeignCallbackRetain),
            12 => decode_empty_intrinsic(decoder, fields, Self::ForeignCallbackRelease),
            13 => decode_empty_intrinsic(decoder, fields, Self::ForeignCallbackState),
            14 => decode_empty_intrinsic(decoder, fields, Self::ForeignCallbackFailure),
            15 => {
                require_intrinsic_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, IntegerIntrinsicKind::decode)
                    .map(Self::Integer)
            }
            16 => decode_unsigned_intrinsic(decoder, fields, decode_primitive_unary)
                .map(Self::PrimitiveUnary),
            17 => decode_unsigned_intrinsic(decoder, fields, decode_primitive_binary)
                .map(Self::PrimitiveBinary),
            18 => decode_unsigned_intrinsic(decoder, fields, decode_array_access)
                .map(Self::ArrayAccess),
            19 => decode_unsigned_intrinsic(decoder, fields, decode_array).map(Self::Array),
            20 => decode_unsigned_intrinsic(decoder, fields, decode_pointer).map(Self::Pointer),
            tag => Err(intrinsic_wire_error(
                decoder,
                WireErrorKind::UnknownTag { tag },
            )),
        }
    }
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

/// Primitive member operations shared by calls and bound reference invokes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveMemberIntrinsic {
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

impl WireEncode for IntegerIntrinsicKind {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(0)?;
        match self {
            Self::NoGcOperation { kind, operation } => {
                encoder.unsigned(1)?;
                encoder.field(1)?;
                encoder.unsigned(integer_kind_tag(*kind))?;
                encoder.field(2)?;
                encoder.unsigned(no_gc_integer_operation_tag(*operation))
            }
            Self::ManagedOperation { kind, operation } => {
                encoder.unsigned(2)?;
                encoder.field(1)?;
                encoder.unsigned(integer_kind_tag(*kind))?;
                encoder.field(2)?;
                encoder.unsigned(integer_div_rem_tag(*operation))
            }
            Self::Conversion {
                source,
                target_kind,
            } => {
                encoder.unsigned(3)?;
                encoder.field(1)?;
                encoder.unsigned(integer_kind_tag(*source))?;
                encoder.field(2)?;
                encoder.unsigned(integer_kind_tag(*target_kind))
            }
        }
    }
}

impl WireDecode for IntegerIntrinsicKind {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let source = decoder.field(1, |decoder| {
            let tag = decoder.unsigned()?;
            decode_integer_kind(decoder, tag)
        })?;
        decoder.field(2, |decoder| {
            let value = decoder.unsigned()?;
            match tag {
                1 => decode_no_gc_integer_operation(decoder, value)
                    .map(|operation| (source, operation))
                    .and_then(|(kind, operation)| {
                        if operation.supports(kind) {
                            Ok(Self::NoGcOperation { kind, operation })
                        } else {
                            Err(intrinsic_wire_error(
                                decoder,
                                WireErrorKind::UnknownTag { tag: value },
                            ))
                        }
                    }),
                2 => {
                    decode_integer_div_rem(decoder, value).map(|operation| Self::ManagedOperation {
                        kind: source,
                        operation,
                    })
                }
                3 => decode_integer_kind(decoder, value).map(|target_kind| Self::Conversion {
                    source,
                    target_kind,
                }),
                tag => Err(intrinsic_wire_error(
                    decoder,
                    WireErrorKind::UnknownTag { tag },
                )),
            }
        })
    }
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
}

impl ArrayIntrinsic {
    pub const ALL: [Self; 2] = [Self::ToImmutable, Self::ToMutable];

    pub const fn name(self) -> &'static str {
        match self {
            Self::ToImmutable => "array_to_immutable",
            Self::ToMutable => "array_to_mutable",
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

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn encode_unsigned_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: u64,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    encoder.unsigned(value)
}

fn decode_empty_intrinsic(
    decoder: &Decoder<'_>,
    fields: u64,
    value: IntrinsicFunctionKind,
) -> Result<IntrinsicFunctionKind, WireError> {
    require_intrinsic_sum_length(decoder, fields, 1)?;
    Ok(value)
}

fn decode_unsigned_intrinsic<T>(
    decoder: &mut Decoder<'_>,
    fields: u64,
    decode: impl FnOnce(&Decoder<'_>, u64) -> Result<T, WireError>,
) -> Result<T, WireError> {
    require_intrinsic_sum_length(decoder, fields, 2)?;
    decoder.field(1, |decoder| {
        let value = decoder.unsigned()?;
        decode(decoder, value)
    })
}

fn require_intrinsic_sum_length(
    decoder: &Decoder<'_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(intrinsic_wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn intrinsic_wire_error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

const fn integer_kind_tag(kind: IntegerKind) -> u64 {
    match kind {
        IntegerKind::SIGNED_8 => 1,
        IntegerKind::SIGNED_16 => 2,
        IntegerKind::SIGNED_32 => 3,
        IntegerKind::SIGNED_64 => 4,
        IntegerKind::UNSIGNED_8 => 5,
        IntegerKind::UNSIGNED_16 => 6,
        IntegerKind::UNSIGNED_32 => 7,
        IntegerKind::UNSIGNED_64 => 8,
    }
}

fn decode_integer_kind(decoder: &Decoder<'_>, tag: u64) -> Result<IntegerKind, WireError> {
    match tag {
        1 => Ok(IntegerKind::SIGNED_8),
        2 => Ok(IntegerKind::SIGNED_16),
        3 => Ok(IntegerKind::SIGNED_32),
        4 => Ok(IntegerKind::SIGNED_64),
        5 => Ok(IntegerKind::UNSIGNED_8),
        6 => Ok(IntegerKind::UNSIGNED_16),
        7 => Ok(IntegerKind::UNSIGNED_32),
        8 => Ok(IntegerKind::UNSIGNED_64),
        tag => Err(intrinsic_wire_error(
            decoder,
            WireErrorKind::UnknownTag { tag },
        )),
    }
}

const fn no_gc_integer_operation_tag(operation: NoGcIntegerOperation) -> u64 {
    match operation {
        NoGcIntegerOperation::UnaryPlus => 1,
        NoGcIntegerOperation::UnaryMinus => 2,
        NoGcIntegerOperation::Inc => 3,
        NoGcIntegerOperation::Dec => 4,
        NoGcIntegerOperation::Add => 5,
        NoGcIntegerOperation::Sub => 6,
        NoGcIntegerOperation::Mul => 7,
        NoGcIntegerOperation::CompareTo => 8,
        NoGcIntegerOperation::Equals => 9,
        NoGcIntegerOperation::And => 10,
        NoGcIntegerOperation::Or => 11,
        NoGcIntegerOperation::Xor => 12,
        NoGcIntegerOperation::Inv => 13,
        NoGcIntegerOperation::Shl => 14,
        NoGcIntegerOperation::Shr => 15,
        NoGcIntegerOperation::Ushr => 16,
    }
}

fn decode_no_gc_integer_operation(
    decoder: &Decoder<'_>,
    tag: u64,
) -> Result<NoGcIntegerOperation, WireError> {
    match tag {
        1 => Ok(NoGcIntegerOperation::UnaryPlus),
        2 => Ok(NoGcIntegerOperation::UnaryMinus),
        3 => Ok(NoGcIntegerOperation::Inc),
        4 => Ok(NoGcIntegerOperation::Dec),
        5 => Ok(NoGcIntegerOperation::Add),
        6 => Ok(NoGcIntegerOperation::Sub),
        7 => Ok(NoGcIntegerOperation::Mul),
        8 => Ok(NoGcIntegerOperation::CompareTo),
        9 => Ok(NoGcIntegerOperation::Equals),
        10 => Ok(NoGcIntegerOperation::And),
        11 => Ok(NoGcIntegerOperation::Or),
        12 => Ok(NoGcIntegerOperation::Xor),
        13 => Ok(NoGcIntegerOperation::Inv),
        14 => Ok(NoGcIntegerOperation::Shl),
        15 => Ok(NoGcIntegerOperation::Shr),
        16 => Ok(NoGcIntegerOperation::Ushr),
        tag => Err(intrinsic_wire_error(
            decoder,
            WireErrorKind::UnknownTag { tag },
        )),
    }
}

const fn integer_div_rem_tag(operation: IntegerDivRem) -> u64 {
    match operation {
        IntegerDivRem::Div => 1,
        IntegerDivRem::Rem => 2,
    }
}

fn decode_integer_div_rem(decoder: &Decoder<'_>, tag: u64) -> Result<IntegerDivRem, WireError> {
    match tag {
        1 => Ok(IntegerDivRem::Div),
        2 => Ok(IntegerDivRem::Rem),
        tag => Err(intrinsic_wire_error(
            decoder,
            WireErrorKind::UnknownTag { tag },
        )),
    }
}

const fn primitive_unary_tag(kind: PrimitiveUnaryKind) -> u64 {
    match kind {
        PrimitiveUnaryKind::BooleanNot => 1,
    }
}

fn decode_primitive_unary(
    decoder: &Decoder<'_>,
    tag: u64,
) -> Result<PrimitiveUnaryKind, WireError> {
    match tag {
        1 => Ok(PrimitiveUnaryKind::BooleanNot),
        tag => Err(intrinsic_wire_error(
            decoder,
            WireErrorKind::UnknownTag { tag },
        )),
    }
}

const fn primitive_binary_tag(kind: PrimitiveBinaryKind) -> u64 {
    match kind {
        PrimitiveBinaryKind::StringConcat => 1,
        PrimitiveBinaryKind::StringCompareTo => 2,
    }
}

fn decode_primitive_binary(
    decoder: &Decoder<'_>,
    tag: u64,
) -> Result<PrimitiveBinaryKind, WireError> {
    match tag {
        1 => Ok(PrimitiveBinaryKind::StringConcat),
        2 => Ok(PrimitiveBinaryKind::StringCompareTo),
        tag => Err(intrinsic_wire_error(
            decoder,
            WireErrorKind::UnknownTag { tag },
        )),
    }
}

const fn array_access_tag(kind: ArrayAccessKind) -> u64 {
    match kind {
        ArrayAccessKind::ImmutableGet => 1,
        ArrayAccessKind::MutableGet => 2,
        ArrayAccessKind::MutableSet => 3,
    }
}

fn decode_array_access(decoder: &Decoder<'_>, tag: u64) -> Result<ArrayAccessKind, WireError> {
    match tag {
        1 => Ok(ArrayAccessKind::ImmutableGet),
        2 => Ok(ArrayAccessKind::MutableGet),
        3 => Ok(ArrayAccessKind::MutableSet),
        tag => Err(intrinsic_wire_error(
            decoder,
            WireErrorKind::UnknownTag { tag },
        )),
    }
}

const fn array_tag(kind: ArrayIntrinsic) -> u64 {
    match kind {
        ArrayIntrinsic::ToImmutable => 1,
        ArrayIntrinsic::ToMutable => 2,
    }
}

fn decode_array(decoder: &Decoder<'_>, tag: u64) -> Result<ArrayIntrinsic, WireError> {
    match tag {
        1 => Ok(ArrayIntrinsic::ToImmutable),
        2 => Ok(ArrayIntrinsic::ToMutable),
        tag => Err(intrinsic_wire_error(
            decoder,
            WireErrorKind::UnknownTag { tag },
        )),
    }
}

const fn pointer_tag(kind: PointerIntrinsic) -> u64 {
    match kind {
        PointerIntrinsic::ToULong => 1,
        PointerIntrinsic::Cast => 2,
        PointerIntrinsic::Load => 3,
        PointerIntrinsic::LoadOffset => 4,
        PointerIntrinsic::Store => 5,
        PointerIntrinsic::StoreOffset => 6,
        PointerIntrinsic::Plus => 7,
        PointerIntrinsic::Minus => 8,
        PointerIntrinsic::AddressOf => 9,
        PointerIntrinsic::SizeOf => 10,
        PointerIntrinsic::AlignOf => 11,
    }
}

fn decode_pointer(decoder: &Decoder<'_>, tag: u64) -> Result<PointerIntrinsic, WireError> {
    match tag {
        1 => Ok(PointerIntrinsic::ToULong),
        2 => Ok(PointerIntrinsic::Cast),
        3 => Ok(PointerIntrinsic::Load),
        4 => Ok(PointerIntrinsic::LoadOffset),
        5 => Ok(PointerIntrinsic::Store),
        6 => Ok(PointerIntrinsic::StoreOffset),
        7 => Ok(PointerIntrinsic::Plus),
        8 => Ok(PointerIntrinsic::Minus),
        9 => Ok(PointerIntrinsic::AddressOf),
        10 => Ok(PointerIntrinsic::SizeOf),
        11 => Ok(PointerIntrinsic::AlignOf),
        tag => Err(intrinsic_wire_error(
            decoder,
            WireErrorKind::UnknownTag { tag },
        )),
    }
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
    use scoop_wire::{decode_canonical, encode};

    #[test]
    fn intrinsic_function_roles_have_a_closed_canonical_wire_identity() {
        let kinds = intrinsic_function_kinds();
        assert_eq!(kinds.len(), INTRINSIC_REGISTRY.len() + 204);
        assert!(kinds.windows(2).all(|pair| pair[0] < pair[1]));

        for kind in kinds {
            let bytes = encode(&kind).unwrap();
            assert_eq!(
                decode_canonical::<IntrinsicFunctionKind>(&bytes).unwrap(),
                kind
            );
        }

        assert_eq!(
            encode(&IntrinsicFunctionKind::GcPinRaw).unwrap(),
            vec![0xa1, 0x00, 0x01]
        );
        assert_eq!(
            encode(&IntrinsicFunctionKind::Integer(
                IntegerIntrinsicKind::Conversion {
                    source: IntegerKind::SIGNED_32,
                    target_kind: IntegerKind::UNSIGNED_64,
                }
            ))
            .unwrap(),
            vec![
                0xa2, 0x00, 0x0f, 0x01, 0xa3, 0x00, 0x03, 0x01, 0x03, 0x02, 0x08
            ]
        );
        assert_eq!(
            encode(&IntrinsicFunctionKind::Pointer(PointerIntrinsic::AlignOf)).unwrap(),
            vec![0xa2, 0x00, 0x14, 0x01, 0x0b]
        );
    }

    #[test]
    fn intrinsic_function_role_reader_rejects_open_or_invalid_values() {
        for bytes in [
            vec![0xa1, 0x00, 0x15],
            vec![0xa2, 0x00, 0x10, 0x01, 0x02],
            vec![0xa2, 0x00, 0x14, 0x01, 0x0c],
            vec![
                0xa2, 0x00, 0x0f, 0x01, 0xa3, 0x00, 0x01, 0x01, 0x05, 0x02, 0x10,
            ],
        ] {
            assert!(decode_canonical::<IntrinsicFunctionKind>(&bytes).is_err());
        }
    }

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
