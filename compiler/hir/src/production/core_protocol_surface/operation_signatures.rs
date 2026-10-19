use scoop_identity::{
    Effect, NonEmptyVec, PersistentGenericTypeId, SignatureCallableShape, SignatureTypeKey,
};

use super::*;

pub(super) fn expected_operation_signature(
    surface: &CoreCompilerProtocolSurfaceV1,
    kind: IntrinsicFunctionKind,
) -> SignatureCallableShape {
    let fundamental = &surface.fundamental_types;
    let unit = SignatureTypeKey::Nominal(fundamental.unit);
    let long = SignatureTypeKey::Nominal(fundamental.int64);
    let ulong = SignatureTypeKey::Nominal(fundamental.uint64);
    let boolean = SignatureTypeKey::Nominal(fundamental.boolean);
    let string = SignatureTypeKey::Nominal(fundamental.string);
    let binder = SignatureTypeKey::Binder { depth: 0, index: 0 };

    let (effect, parameters, result) = match kind {
        IntrinsicFunctionKind::Atomic(_)
        | IntrinsicFunctionKind::MaybeUninit(_)
        | IntrinsicFunctionKind::TypeReference
        | IntrinsicFunctionKind::TypeDescription
        | IntrinsicFunctionKind::AnnotationKey => {
            unreachable!("these signatures come from their ordinary nominal declaration")
        }
        IntrinsicFunctionKind::Float(kind) => {
            let floating = |kind| SignatureTypeKey::Nominal(surface.float_source_type(kind));
            match kind {
                crate::FloatIntrinsicKind::Unary { kind, operation } => (
                    Effect::Ordinary,
                    Vec::new(),
                    if operation.is_predicate() {
                        boolean.clone()
                    } else {
                        floating(kind)
                    },
                ),
                crate::FloatIntrinsicKind::Binary { kind, operation } => (
                    Effect::Ordinary,
                    vec![floating(kind)],
                    if operation.is_predicate() {
                        boolean.clone()
                    } else if operation == crate::FloatBinaryOperator::CompareTo {
                        long.clone()
                    } else {
                        floating(kind)
                    },
                ),
                crate::FloatIntrinsicKind::Conversion(conversion) => {
                    let result = match conversion {
                        scoop_identity::FloatConversion::FromInteger { target, .. }
                        | scoop_identity::FloatConversion::BetweenFloats { target, .. } => {
                            floating(target)
                        }
                        scoop_identity::FloatConversion::ToInteger { target, .. } => {
                            signature_integer(fundamental, target)
                        }
                    };
                    (Effect::Ordinary, Vec::new(), result)
                }
            }
        }
        IntrinsicFunctionKind::Char(kind) => {
            let character = SignatureTypeKey::Nominal(fundamental.character);
            match kind {
                crate::CharIntrinsic::Code => (
                    Effect::Ordinary,
                    Vec::new(),
                    SignatureTypeKey::Nominal(fundamental.int32),
                ),
                crate::CharIntrinsic::FromCodeUnchecked => {
                    (Effect::Ordinary, Vec::new(), character)
                }
                crate::CharIntrinsic::Equals => (Effect::Ordinary, vec![character], boolean),
                crate::CharIntrinsic::CompareTo => (Effect::Ordinary, vec![character], long),
            }
        }
        IntrinsicFunctionKind::GcPinRaw | IntrinsicFunctionKind::GcGetHandleRaw => {
            (Effect::Ordinary, vec![binder.clone()], ulong.clone())
        }
        IntrinsicFunctionKind::GcUnpinRaw | IntrinsicFunctionKind::GcReleaseHandleRaw => {
            (Effect::Ordinary, vec![ulong.clone()], binder.clone())
        }
        IntrinsicFunctionKind::GcCollect => (Effect::Ordinary, Vec::new(), unit.clone()),
        IntrinsicFunctionKind::GcStats => (Effect::Ordinary, Vec::new(), ulong.clone()),
        IntrinsicFunctionKind::String(kind) => match kind {
            crate::StringIntrinsic::ByteLength => (Effect::Ordinary, vec![string], long),
            crate::StringIntrinsic::ByteAt => (
                Effect::Ordinary,
                vec![string, long],
                signature_integer(fundamental, crate::IntegerKind::SIGNED_8),
            ),
            crate::StringIntrinsic::CharacterAtByte => (
                Effect::Ordinary,
                vec![string, long],
                SignatureTypeKey::Nominal(fundamental.character),
            ),
        },
        IntrinsicFunctionKind::CoroutineStart => {
            let coroutine = &surface.coroutine_protocol;
            (
                Effect::Ordinary,
                vec![
                    signature_application(coroutine.suspend_task, binder.clone()),
                    signature_application(coroutine.continuation, binder.clone()),
                ],
                unit.clone(),
            )
        }
        IntrinsicFunctionKind::CoroutineSuspend => {
            let coroutine = &surface.coroutine_protocol;
            (
                Effect::Suspend,
                vec![signature_application(
                    coroutine.suspend_registration,
                    binder.clone(),
                )],
                binder.clone(),
            )
        }
        IntrinsicFunctionKind::CurrentSourceLocation => (
            Effect::Ordinary,
            Vec::new(),
            SignatureTypeKey::Nominal(surface.source_location_protocol.location),
        ),
        IntrinsicFunctionKind::ForeignCallbackRegister => {
            let callback = &surface.foreign_callback_protocol;
            (
                Effect::Ordinary,
                vec![
                    SignatureTypeKey::Nominal(fundamental.any),
                    long.clone(),
                    SignatureTypeKey::Nominal(callback.mode),
                ],
                signature_application(callback.callback, binder.clone()),
            )
        }
        IntrinsicFunctionKind::ForeignCallbackRetain => {
            let callback =
                signature_application(surface.foreign_callback_protocol.callback, binder.clone());
            (Effect::Ordinary, vec![callback.clone()], callback)
        }
        IntrinsicFunctionKind::ForeignCallbackRelease => (
            Effect::Ordinary,
            vec![signature_application(
                surface.foreign_callback_protocol.callback,
                binder.clone(),
            )],
            unit.clone(),
        ),
        IntrinsicFunctionKind::ForeignCallbackState => (
            Effect::Ordinary,
            vec![signature_application(
                surface.foreign_callback_protocol.callback,
                binder.clone(),
            )],
            SignatureTypeKey::Nominal(surface.foreign_callback_protocol.state),
        ),
        IntrinsicFunctionKind::ForeignCallbackFailure => (
            Effect::Ordinary,
            vec![signature_application(
                surface.foreign_callback_protocol.callback,
                binder,
            )],
            signature_application(
                surface.option_protocol.option,
                SignatureTypeKey::Nominal(surface.exception_protocol.throwable),
            ),
        ),
        IntrinsicFunctionKind::Integer(kind) => expected_integer_signature(fundamental, kind),
        IntrinsicFunctionKind::PrimitiveUnary(crate::PrimitiveUnaryKind::BooleanNot) => {
            (Effect::Ordinary, Vec::new(), boolean)
        }
        IntrinsicFunctionKind::PrimitiveBinary(kind) => match kind {
            crate::PrimitiveBinaryKind::StringConcat => {
                (Effect::Ordinary, vec![string.clone()], string)
            }
            crate::PrimitiveBinaryKind::StringCompareTo => (Effect::Ordinary, vec![string], long),
        },
        IntrinsicFunctionKind::ArrayAccess(kind) => match kind {
            crate::ArrayAccessKind::ImmutableGet | crate::ArrayAccessKind::MutableGet => {
                (Effect::Ordinary, vec![long], binder)
            }
            crate::ArrayAccessKind::MutableSet => (Effect::Ordinary, vec![long, binder], unit),
        },
        IntrinsicFunctionKind::Array(
            crate::ArrayIntrinsic::ImmutableLength | crate::ArrayIntrinsic::MutableLength,
        ) => (Effect::Ordinary, Vec::new(), long),
        IntrinsicFunctionKind::Array(kind) => {
            let result = match kind {
                crate::ArrayIntrinsic::ToImmutable => fundamental.array,
                crate::ArrayIntrinsic::ToMutable => fundamental.mutable_array,
                crate::ArrayIntrinsic::ImmutableLength | crate::ArrayIntrinsic::MutableLength => {
                    unreachable!("length is handled above")
                }
            };
            (
                Effect::Ordinary,
                Vec::new(),
                signature_application(result, binder),
            )
        }
        IntrinsicFunctionKind::DataBorrow(kind) => {
            let (source, element) = match kind {
                crate::DataBorrowIntrinsic::Array => (
                    signature_application(fundamental.array, binder.clone()),
                    binder,
                ),
                crate::DataBorrowIntrinsic::MutableArray => (
                    signature_application(fundamental.mutable_array, binder.clone()),
                    binder,
                ),
                crate::DataBorrowIntrinsic::String => (
                    string,
                    signature_integer(fundamental, crate::IntegerKind::UNSIGNED_8),
                ),
            };
            let result = SignatureTypeKey::Binder {
                depth: 0,
                index: kind.type_parameter_count() - 1,
            };
            let callback = SignatureTypeKey::Function {
                receiver: None,
                effect: Effect::Ordinary,
                parameters: vec![SignatureTypeKey::RawPointer(Box::new(element)), long],
                result: Box::new(result.clone()),
            };
            (Effect::Ordinary, vec![source, callback], result)
        }
        IntrinsicFunctionKind::Pointer(kind) => expected_pointer_signature(fundamental, kind),
    };
    SignatureCallableShape::new(effect, None, parameters, result)
}

fn expected_integer_signature(
    fundamental: &CoreFundamentalTypeProtocolV1,
    intrinsic: crate::IntegerIntrinsicKind,
) -> (Effect, Vec<SignatureTypeKey>, SignatureTypeKey) {
    let long = SignatureTypeKey::Nominal(fundamental.int64);
    let boolean = SignatureTypeKey::Nominal(fundamental.boolean);
    match intrinsic {
        crate::IntegerIntrinsicKind::NoGcOperation { kind, operation } => {
            let owner = signature_integer(fundamental, kind);
            let (parameters, result) = match operation {
                crate::NoGcIntegerOperation::UnaryPlus
                | crate::NoGcIntegerOperation::UnaryMinus
                | crate::NoGcIntegerOperation::Inc
                | crate::NoGcIntegerOperation::Dec
                | crate::NoGcIntegerOperation::Inv => (Vec::new(), owner),
                crate::NoGcIntegerOperation::Add
                | crate::NoGcIntegerOperation::Sub
                | crate::NoGcIntegerOperation::Mul
                | crate::NoGcIntegerOperation::And
                | crate::NoGcIntegerOperation::Or
                | crate::NoGcIntegerOperation::Xor => (vec![owner.clone()], owner),
                crate::NoGcIntegerOperation::CompareTo => (vec![owner], long),
                crate::NoGcIntegerOperation::Equals => (vec![owner], boolean),
                crate::NoGcIntegerOperation::Shl
                | crate::NoGcIntegerOperation::Shr
                | crate::NoGcIntegerOperation::Ushr => (vec![long], owner),
            };
            (Effect::Ordinary, parameters, result)
        }
        crate::IntegerIntrinsicKind::ManagedOperation { kind, .. } => {
            let owner = signature_integer(fundamental, kind);
            (Effect::Ordinary, vec![owner.clone()], owner)
        }
        crate::IntegerIntrinsicKind::Conversion { target_kind, .. } => (
            Effect::Ordinary,
            Vec::new(),
            signature_integer(fundamental, target_kind),
        ),
    }
}

fn expected_pointer_signature(
    fundamental: &CoreFundamentalTypeProtocolV1,
    kind: crate::PointerIntrinsic,
) -> (Effect, Vec<SignatureTypeKey>, SignatureTypeKey) {
    let unit = SignatureTypeKey::Nominal(fundamental.unit);
    let long = SignatureTypeKey::Nominal(fundamental.int64);
    let ulong = SignatureTypeKey::Nominal(fundamental.uint64);
    let binder = SignatureTypeKey::Binder { depth: 0, index: 0 };
    match kind {
        crate::PointerIntrinsic::ToULong
        | crate::PointerIntrinsic::SizeOf
        | crate::PointerIntrinsic::AlignOf => (Effect::Ordinary, Vec::new(), ulong),
        crate::PointerIntrinsic::Cast => (
            Effect::Ordinary,
            Vec::new(),
            SignatureTypeKey::RawPointer(Box::new(binder)),
        ),
        crate::PointerIntrinsic::Load => (Effect::Ordinary, Vec::new(), binder),
        crate::PointerIntrinsic::LoadOffset => (Effect::Ordinary, vec![long], binder),
        crate::PointerIntrinsic::Store => (Effect::Ordinary, vec![binder], unit),
        crate::PointerIntrinsic::StoreOffset => (Effect::Ordinary, vec![long, binder], unit),
        crate::PointerIntrinsic::Plus | crate::PointerIntrinsic::Minus => (
            Effect::Ordinary,
            vec![long],
            SignatureTypeKey::RawPointer(Box::new(binder)),
        ),
        crate::PointerIntrinsic::AddressOf => (
            Effect::Ordinary,
            vec![binder.clone()],
            SignatureTypeKey::RawPointer(Box::new(binder)),
        ),
    }
}

pub(super) fn operation_own_type_parameter_count(kind: IntrinsicFunctionKind) -> u32 {
    match kind {
        IntrinsicFunctionKind::DataBorrow(kind) => kind.type_parameter_count(),
        IntrinsicFunctionKind::TypeReference
        | IntrinsicFunctionKind::TypeDescription
        | IntrinsicFunctionKind::AnnotationKey => 1,
        IntrinsicFunctionKind::GcPinRaw
        | IntrinsicFunctionKind::GcUnpinRaw
        | IntrinsicFunctionKind::GcGetHandleRaw
        | IntrinsicFunctionKind::GcReleaseHandleRaw
        | IntrinsicFunctionKind::CoroutineStart
        | IntrinsicFunctionKind::CoroutineSuspend
        | IntrinsicFunctionKind::ForeignCallbackRegister
        | IntrinsicFunctionKind::ForeignCallbackRetain
        | IntrinsicFunctionKind::ForeignCallbackRelease
        | IntrinsicFunctionKind::ForeignCallbackState
        | IntrinsicFunctionKind::ForeignCallbackFailure
        | IntrinsicFunctionKind::Pointer(
            crate::PointerIntrinsic::Cast
            | crate::PointerIntrinsic::AddressOf
            | crate::PointerIntrinsic::SizeOf
            | crate::PointerIntrinsic::AlignOf,
        ) => 1,
        IntrinsicFunctionKind::GcCollect
        | IntrinsicFunctionKind::GcStats
        | IntrinsicFunctionKind::CurrentSourceLocation
        | IntrinsicFunctionKind::Integer(_)
        | IntrinsicFunctionKind::Float(_)
        | IntrinsicFunctionKind::Char(_)
        | IntrinsicFunctionKind::PrimitiveUnary(_)
        | IntrinsicFunctionKind::PrimitiveBinary(_)
        | IntrinsicFunctionKind::ArrayAccess(_)
        | IntrinsicFunctionKind::Array(_)
        | IntrinsicFunctionKind::Atomic(_)
        | IntrinsicFunctionKind::MaybeUninit(_)
        | IntrinsicFunctionKind::String(_)
        | IntrinsicFunctionKind::Pointer(_) => 0,
    }
}

fn signature_integer(
    fundamental: &CoreFundamentalTypeProtocolV1,
    kind: crate::IntegerKind,
) -> SignatureTypeKey {
    SignatureTypeKey::Nominal(fundamental.integer(kind))
}

fn signature_application(
    origin: PersistentGenericTypeId,
    argument: SignatureTypeKey,
) -> SignatureTypeKey {
    SignatureTypeKey::NominalApplication {
        origin,
        arguments: NonEmptyVec::from_first(argument, []),
    }
}
