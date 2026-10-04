use scoop_identity::DefinitionOwnerAtom;

use super::*;

pub(super) fn expected_operation_owner(
    surface: &CoreCompilerProtocolSurfaceV1,
    kind: IntrinsicFunctionKind,
) -> Option<DefinitionOwnerAtom> {
    let fundamental = surface.fundamental_types.entries();
    match kind {
        IntrinsicFunctionKind::Integer(kind) => {
            let source = match kind {
                crate::IntegerIntrinsicKind::NoGcOperation { kind, .. }
                | crate::IntegerIntrinsicKind::ManagedOperation { kind, .. } => kind,
                crate::IntegerIntrinsicKind::Conversion { source, .. } => source,
            };
            let index = crate::IntegerKind::ALL
                .iter()
                .position(|candidate| *candidate == source)
                .expect("every integer intrinsic uses one canonical integer owner");
            Some(DefinitionOwnerAtom::Type(concrete_entry(
                fundamental,
                index + 1,
            )))
        }
        IntrinsicFunctionKind::PrimitiveUnary(_) => {
            Some(DefinitionOwnerAtom::Type(concrete_entry(fundamental, 9)))
        }
        IntrinsicFunctionKind::PrimitiveBinary(_) => {
            Some(DefinitionOwnerAtom::Type(concrete_entry(fundamental, 10)))
        }
        IntrinsicFunctionKind::ArrayAccess(kind) => {
            Some(DefinitionOwnerAtom::GenericType(generic_entry(
                fundamental,
                if kind == crate::ArrayAccessKind::ImmutableGet {
                    11
                } else {
                    12
                },
            )))
        }
        IntrinsicFunctionKind::Array(kind) => {
            Some(DefinitionOwnerAtom::GenericType(generic_entry(
                fundamental,
                if matches!(
                    kind,
                    crate::ArrayIntrinsic::ToImmutable | crate::ArrayIntrinsic::MutableLength
                ) {
                    12
                } else {
                    11
                },
            )))
        }
        IntrinsicFunctionKind::Pointer(
            crate::PointerIntrinsic::ToULong
            | crate::PointerIntrinsic::Cast
            | crate::PointerIntrinsic::Load
            | crate::PointerIntrinsic::LoadOffset
            | crate::PointerIntrinsic::Store
            | crate::PointerIntrinsic::StoreOffset
            | crate::PointerIntrinsic::Plus
            | crate::PointerIntrinsic::Minus,
        ) => Some(DefinitionOwnerAtom::GenericType(generic_entry(
            fundamental,
            13,
        ))),
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
        | IntrinsicFunctionKind::Pointer(_) => None,
    }
}
