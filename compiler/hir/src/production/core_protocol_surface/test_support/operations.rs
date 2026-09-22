use super::*;

pub(super) fn operation_entry(
    operations: &[CoreCompilerOperationV1],
    kind: IntrinsicFunctionKind,
) -> CoreProtocolEntryV1 {
    CoreProtocolEntryV1::Callable(
        operations
            .iter()
            .find(|operation| operation.kind == kind)
            .expect("the test fixture constructs every intrinsic operation")
            .callable
            .clone(),
    )
}

pub(super) fn fixture_operation_owner(
    kind: IntrinsicFunctionKind,
    fundamental: &CoreFundamentalTypeProtocolV1,
) -> Option<DefinitionOwnerAtom> {
    match kind {
        IntrinsicFunctionKind::Integer(kind) => {
            let index = crate::IntegerKind::ALL
                .iter()
                .position(|candidate| *candidate == integer_source_kind(kind))
                .expect("every integer intrinsic uses a canonical integer kind");
            Some(DefinitionOwnerAtom::Type(concrete_entry_ref(
                fundamental.entries(),
                index + 1,
            )))
        }
        IntrinsicFunctionKind::PrimitiveUnary(_) => Some(DefinitionOwnerAtom::Type(
            concrete_entry_ref(fundamental.entries(), 9),
        )),
        IntrinsicFunctionKind::PrimitiveBinary(_) => Some(DefinitionOwnerAtom::Type(
            concrete_entry_ref(fundamental.entries(), 10),
        )),
        IntrinsicFunctionKind::ArrayAccess(kind) => {
            Some(DefinitionOwnerAtom::GenericType(generic_entry_ref(
                fundamental.entries(),
                if kind == crate::ArrayAccessKind::ImmutableGet {
                    11
                } else {
                    12
                },
            )))
        }
        IntrinsicFunctionKind::Array(kind) => {
            Some(DefinitionOwnerAtom::GenericType(generic_entry_ref(
                fundamental.entries(),
                if kind == crate::ArrayIntrinsic::ToImmutable {
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
        ) => Some(DefinitionOwnerAtom::GenericType(generic_entry_ref(
            fundamental.entries(),
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

fn integer_source_kind(kind: crate::IntegerIntrinsicKind) -> crate::IntegerKind {
    match kind {
        crate::IntegerIntrinsicKind::NoGcOperation { kind, .. }
        | crate::IntegerIntrinsicKind::ManagedOperation { kind, .. } => kind,
        crate::IntegerIntrinsicKind::Conversion { source, .. } => source,
    }
}
