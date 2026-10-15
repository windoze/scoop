use scoop_identity::DefinitionOwnerAtom;

use super::*;

pub(super) fn expected_operation_owner(
    surface: &CoreCompilerProtocolSurfaceV1,
    kind: IntrinsicFunctionKind,
) -> Option<DefinitionOwnerAtom> {
    let fundamental = &surface.fundamental_types;
    match kind {
        IntrinsicFunctionKind::Atomic(_)
        | IntrinsicFunctionKind::MaybeUninit(_)
        | IntrinsicFunctionKind::TypeReference
        | IntrinsicFunctionKind::TypeDescription => {
            unreachable!("these intrinsics use their ordinary nominal owner")
        }
        IntrinsicFunctionKind::Float(kind) => {
            let owner = match kind.owner() {
                crate::IntrinsicTypeKind::Float(crate::FloatKind::F32) => fundamental.float,
                crate::IntrinsicTypeKind::Float(crate::FloatKind::F64) => fundamental.double,
                crate::IntrinsicTypeKind::Integer(integer) => fundamental.integer(integer),
                _ => unreachable!("floating operations have numeric owners"),
            };
            Some(DefinitionOwnerAtom::Type(owner))
        }
        IntrinsicFunctionKind::Char(kind) => Some(DefinitionOwnerAtom::Type(
            if kind == crate::CharIntrinsic::FromCodeUnchecked {
                fundamental.int32
            } else {
                fundamental.character
            },
        )),
        IntrinsicFunctionKind::Integer(kind) => {
            let source = match kind {
                crate::IntegerIntrinsicKind::NoGcOperation { kind, .. }
                | crate::IntegerIntrinsicKind::ManagedOperation { kind, .. } => kind,
                crate::IntegerIntrinsicKind::Conversion { source, .. } => source,
            };
            Some(DefinitionOwnerAtom::Type(fundamental.integer(source)))
        }
        IntrinsicFunctionKind::PrimitiveUnary(_) => {
            Some(DefinitionOwnerAtom::Type(fundamental.boolean))
        }
        IntrinsicFunctionKind::PrimitiveBinary(_) => {
            Some(DefinitionOwnerAtom::Type(fundamental.string))
        }
        IntrinsicFunctionKind::ArrayAccess(kind) => Some(DefinitionOwnerAtom::GenericType(
            if kind == crate::ArrayAccessKind::ImmutableGet {
                fundamental.array
            } else {
                fundamental.mutable_array
            },
        )),
        IntrinsicFunctionKind::Array(kind) => Some(DefinitionOwnerAtom::GenericType(
            if matches!(
                kind,
                crate::ArrayIntrinsic::ToImmutable | crate::ArrayIntrinsic::MutableLength
            ) {
                fundamental.mutable_array
            } else {
                fundamental.array
            },
        )),
        IntrinsicFunctionKind::Pointer(
            crate::PointerIntrinsic::ToULong
            | crate::PointerIntrinsic::Cast
            | crate::PointerIntrinsic::Load
            | crate::PointerIntrinsic::LoadOffset
            | crate::PointerIntrinsic::Store
            | crate::PointerIntrinsic::StoreOffset
            | crate::PointerIntrinsic::Plus
            | crate::PointerIntrinsic::Minus,
        ) => Some(DefinitionOwnerAtom::GenericType(fundamental.ptr)),
        IntrinsicFunctionKind::GcPinRaw
        | IntrinsicFunctionKind::GcUnpinRaw
        | IntrinsicFunctionKind::GcGetHandleRaw
        | IntrinsicFunctionKind::GcReleaseHandleRaw
        | IntrinsicFunctionKind::GcCollect
        | IntrinsicFunctionKind::GcStats
        | IntrinsicFunctionKind::CoroutineStart
        | IntrinsicFunctionKind::CoroutineSuspend
        | IntrinsicFunctionKind::CurrentSourceLocation
        | IntrinsicFunctionKind::ForeignCallbackRegister
        | IntrinsicFunctionKind::ForeignCallbackRetain
        | IntrinsicFunctionKind::ForeignCallbackRelease
        | IntrinsicFunctionKind::ForeignCallbackState
        | IntrinsicFunctionKind::ForeignCallbackFailure
        | IntrinsicFunctionKind::DataBorrow(_)
        | IntrinsicFunctionKind::Pointer(_) => None,
    }
}
