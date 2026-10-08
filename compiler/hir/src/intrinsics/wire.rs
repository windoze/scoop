//! Canonical encodings for intrinsic operations.

use super::*;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

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
            Self::Float(kind) => encode_value_sum(encoder, 22, kind),
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
            Self::Char(kind) => encode_unsigned_value_sum(encoder, 21, kind.wire_tag()),
            Self::Pointer(kind) => encode_unsigned_value_sum(encoder, 20, pointer_tag(*kind)),
            Self::DataBorrow(kind) => encode_unsigned_value_sum(encoder, 23, kind.wire_tag()),
            Self::Atomic(kind) => encode_value_sum(encoder, 24, kind),
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
            24 => {
                require_intrinsic_sum_length(decoder, fields, 2)?;
                decoder.field(1, AtomicIntrinsic::decode).map(Self::Atomic)
            }
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
            22 => {
                require_intrinsic_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, FloatIntrinsicKind::decode)
                    .map(Self::Float)
            }
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
            23 => decode_unsigned_intrinsic(decoder, fields, |decoder, tag| {
                DataBorrowIntrinsic::ALL
                    .into_iter()
                    .find(|kind| kind.wire_tag() == tag)
                    .ok_or_else(|| intrinsic_wire_error(decoder, WireErrorKind::UnknownTag { tag }))
            })
            .map(Self::DataBorrow),
            21 => decode_unsigned_intrinsic(decoder, fields, |decoder, tag| {
                CharIntrinsic::ALL
                    .into_iter()
                    .find(|kind| kind.wire_tag() == tag)
                    .ok_or_else(|| intrinsic_wire_error(decoder, WireErrorKind::UnknownTag { tag }))
            })
            .map(Self::Char),
            tag => Err(intrinsic_wire_error(
                decoder,
                WireErrorKind::UnknownTag { tag },
            )),
        }
    }
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
        ArrayIntrinsic::ImmutableLength => 3,
        ArrayIntrinsic::MutableLength => 4,
    }
}

fn decode_array(decoder: &Decoder<'_>, tag: u64) -> Result<ArrayIntrinsic, WireError> {
    match tag {
        1 => Ok(ArrayIntrinsic::ToImmutable),
        2 => Ok(ArrayIntrinsic::ToMutable),
        3 => Ok(ArrayIntrinsic::ImmutableLength),
        4 => Ok(ArrayIntrinsic::MutableLength),
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
