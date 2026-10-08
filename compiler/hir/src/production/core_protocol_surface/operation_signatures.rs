use scoop_identity::{
    Effect, NonEmptyVec, PersistentGenericTypeId, SignatureCallableShape, SignatureTypeKey,
};

use super::*;

pub(super) fn expected_operation_signature(
    surface: &CoreCompilerProtocolSurfaceV1,
    kind: IntrinsicFunctionKind,
) -> SignatureCallableShape {
    let fundamental = surface.fundamental_types.entries();
    let unit = signature_concrete(fundamental, 0);
    let long = signature_concrete(fundamental, 4);
    let ulong = signature_concrete(fundamental, 8);
    let boolean = signature_concrete(fundamental, 9);
    let string = signature_concrete(fundamental, 10);
    let binder = SignatureTypeKey::Binder { depth: 0, index: 0 };

    let (effect, parameters, result) = match kind {
        IntrinsicFunctionKind::Atomic(_) => {
            unreachable!("atomic signatures are provided by their ordinary nominal declaration")
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
            let character = signature_concrete(fundamental, 15);
            match kind {
                crate::CharIntrinsic::Code => (
                    Effect::Ordinary,
                    Vec::new(),
                    signature_concrete(fundamental, 3),
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
        IntrinsicFunctionKind::CoroutineStart => {
            let coroutine = surface.coroutine_protocol.entries();
            (
                Effect::Ordinary,
                vec![
                    signature_application(coroutine, 5, binder.clone()),
                    signature_application(coroutine, 0, binder.clone()),
                ],
                unit.clone(),
            )
        }
        IntrinsicFunctionKind::CoroutineSuspend => {
            let coroutine = surface.coroutine_protocol.entries();
            (
                Effect::Suspend,
                vec![signature_application(coroutine, 8, binder.clone())],
                binder.clone(),
            )
        }
        IntrinsicFunctionKind::CurrentSourceLocation => (
            Effect::Ordinary,
            Vec::new(),
            signature_concrete(surface.source_location_protocol.entries(), 0),
        ),
        IntrinsicFunctionKind::ForeignCallbackRegister => {
            let callback = surface.foreign_callback_protocol.entries();
            (
                Effect::Ordinary,
                vec![
                    signature_concrete(fundamental, 18),
                    long.clone(),
                    signature_concrete(callback, 1),
                ],
                signature_application(callback, 0, binder.clone()),
            )
        }
        IntrinsicFunctionKind::ForeignCallbackRetain => {
            let callback = signature_application(
                surface.foreign_callback_protocol.entries(),
                0,
                binder.clone(),
            );
            (Effect::Ordinary, vec![callback.clone()], callback)
        }
        IntrinsicFunctionKind::ForeignCallbackRelease => (
            Effect::Ordinary,
            vec![signature_application(
                surface.foreign_callback_protocol.entries(),
                0,
                binder.clone(),
            )],
            unit.clone(),
        ),
        IntrinsicFunctionKind::ForeignCallbackState => (
            Effect::Ordinary,
            vec![signature_application(
                surface.foreign_callback_protocol.entries(),
                0,
                binder.clone(),
            )],
            signature_concrete(surface.foreign_callback_protocol.entries(), 4),
        ),
        IntrinsicFunctionKind::ForeignCallbackFailure => (
            Effect::Ordinary,
            vec![signature_application(
                surface.foreign_callback_protocol.entries(),
                0,
                binder,
            )],
            signature_application(
                surface.option_protocol.entries(),
                0,
                signature_concrete(surface.exception_protocol.entries(), 0),
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
                crate::ArrayIntrinsic::ToImmutable => 11,
                crate::ArrayIntrinsic::ToMutable => 12,
                crate::ArrayIntrinsic::ImmutableLength | crate::ArrayIntrinsic::MutableLength => {
                    unreachable!("length is handled above")
                }
            };
            (
                Effect::Ordinary,
                Vec::new(),
                signature_application(fundamental, result, binder),
            )
        }
        IntrinsicFunctionKind::DataBorrow(kind) => {
            let (source, element) = match kind {
                crate::DataBorrowIntrinsic::Array => (
                    signature_application(fundamental, 11, binder.clone()),
                    binder,
                ),
                crate::DataBorrowIntrinsic::MutableArray => (
                    signature_application(fundamental, 12, binder.clone()),
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
    fundamental: &[CoreProtocolEntryV1; FUNDAMENTAL_TYPE_COUNT],
    intrinsic: crate::IntegerIntrinsicKind,
) -> (Effect, Vec<SignatureTypeKey>, SignatureTypeKey) {
    let long = signature_concrete(fundamental, 4);
    let boolean = signature_concrete(fundamental, 9);
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
    fundamental: &[CoreProtocolEntryV1; FUNDAMENTAL_TYPE_COUNT],
    kind: crate::PointerIntrinsic,
) -> (Effect, Vec<SignatureTypeKey>, SignatureTypeKey) {
    let unit = signature_concrete(fundamental, 0);
    let long = signature_concrete(fundamental, 4);
    let ulong = signature_concrete(fundamental, 8);
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
        | IntrinsicFunctionKind::Pointer(_) => 0,
    }
}

fn signature_integer(
    fundamental: &[CoreProtocolEntryV1; FUNDAMENTAL_TYPE_COUNT],
    kind: crate::IntegerKind,
) -> SignatureTypeKey {
    let index = crate::IntegerKind::ALL
        .iter()
        .position(|candidate| *candidate == kind)
        .expect("every integer operation uses a canonical integer kind");
    signature_concrete(fundamental, index + 1)
}

fn signature_concrete<const N: usize>(
    entries: &[CoreProtocolEntryV1; N],
    index: usize,
) -> SignatureTypeKey {
    SignatureTypeKey::Nominal(concrete_entry(entries, index))
}

fn signature_application<const N: usize>(
    entries: &[CoreProtocolEntryV1; N],
    index: usize,
    argument: SignatureTypeKey,
) -> SignatureTypeKey {
    SignatureTypeKey::NominalApplication {
        origin: generic_entry(entries, index),
        arguments: NonEmptyVec::from_first(argument, []),
    }
}

fn generic_entry<const N: usize>(
    entries: &[CoreProtocolEntryV1; N],
    index: usize,
) -> PersistentGenericTypeId {
    match entries[index] {
        CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(id)) => id,
        _ => unreachable!("validated core protocol role has its fixed subject kind"),
    }
}
