use super::*;

pub(super) fn validate_runtime_call(
    function: &Function,
    call: &scoop_lir::TypedCallView<'_>,
    protocol: CallProtocol,
    runtime: scoop_lir::RuntimeFunction,
) -> Result<(), CodegenError> {
    if runtime.requires_dedicated_operation() {
        return Err(call_error(
            function,
            "boxing runtime calls require a descriptor-refined operation",
        ));
    }
    let (expected_arguments, expected_result, expected_protocol) = runtime_signature(runtime);
    let arguments_match = call.arguments().len() == expected_arguments.len()
        && call
            .arguments()
            .iter()
            .zip(&expected_arguments)
            .all(|(argument, expected)| {
                matches!(argument, scoop_lir::AbiArgument::Direct(value)
                    if value.storage_type() == expected)
            });
    let result_matches = match (expected_result.as_ref(), call) {
        (None, scoop_lir::TypedCallView::Void { .. }) => true,
        (Some(expected), scoop_lir::TypedCallView::Direct { signature, .. }) => {
            signature.result().storage_type() == expected
        }
        _ => false,
    };
    if !arguments_match || !result_matches || protocol != expected_protocol {
        return Err(call_error(
            function,
            format!(
                "has a signature or protocol outside the closed runtime ABI for `{}`",
                runtime.symbol()
            ),
        ));
    }
    Ok(())
}

pub(super) fn runtime_signature(
    runtime: scoop_lir::RuntimeFunction,
) -> (Vec<LirType>, Option<LirType>, CallProtocol) {
    use scoop_lir::{ManagedRuntimeFunction as Managed, NoGcRuntimeFunction as NoGc};

    match runtime {
        scoop_lir::RuntimeFunction::Managed(function) => {
            let (arguments, result) = match function {
                Managed::Safepoint | Managed::GcCollect => (Vec::new(), None),
                Managed::Alloc => (
                    vec![
                        scoop_lir::METADATA_PTR,
                        LirType::MachineScalar(MachineScalarKind::ByteSize),
                    ],
                    Some(scoop_lir::MANAGED_PTR),
                ),
                Managed::BoxZst | Managed::BoxValue => unreachable!(
                    "dedicated operations are rejected before runtime signature lookup"
                ),
                Managed::MaterializeException => {
                    (vec![scoop_lir::MANAGED_PTR], Some(scoop_lir::MANAGED_PTR))
                }
                Managed::ContextPush => (
                    vec![
                        scoop_lir::RAW_PTR,
                        scoop_lir::MANAGED_PTR,
                        scoop_lir::METADATA_PTR,
                    ],
                    Some(scoop_lir::MANAGED_PTR),
                ),
                Managed::ContextFork => (
                    vec![scoop_lir::MANAGED_PTR, scoop_lir::METADATA_PTR],
                    Some(scoop_lir::MANAGED_PTR),
                ),
                Managed::ContextEnsureRoot => {
                    (vec![scoop_lir::METADATA_PTR], Some(scoop_lir::MANAGED_PTR))
                }
                Managed::StringConcat => (
                    vec![scoop_lir::MANAGED_PTR, scoop_lir::MANAGED_PTR],
                    Some(scoop_lir::MANAGED_PTR),
                ),
                Managed::InitializationEnter => (
                    vec![scoop_lir::METADATA_PTR],
                    Some(LirType::MachineScalar(
                        MachineScalarKind::InitializationOutcome,
                    )),
                ),
                Managed::InitializationSucceed => (vec![scoop_lir::METADATA_PTR], None),
                Managed::InitializationFail => {
                    (vec![scoop_lir::METADATA_PTR, scoop_lir::MANAGED_PTR], None)
                }
                Managed::InitializationFailure | Managed::InitializationCycleMessage => {
                    (vec![scoop_lir::METADATA_PTR], Some(scoop_lir::MANAGED_PTR))
                }
            };
            (arguments, result, CallProtocol::Managed)
        }
        scoop_lir::RuntimeFunction::NoGc(function) => {
            let (arguments, result) = match function {
                NoGc::IsInstance => (
                    vec![scoop_lir::MANAGED_PTR, scoop_lir::METADATA_PTR],
                    Some(LirType::I1),
                ),
                NoGc::ITableLookup => (
                    vec![scoop_lir::METADATA_PTR, scoop_lir::METADATA_PTR],
                    Some(scoop_lir::METADATA_PTR),
                ),
                NoGc::Pin | NoGc::GetHandle => (vec![scoop_lir::MANAGED_PTR], Some(LirType::I64)),
                NoGc::Unpin | NoGc::ReleaseHandle => {
                    (vec![LirType::I64], Some(scoop_lir::MANAGED_PTR))
                }
                NoGc::ContextTryGet => (vec![scoop_lir::RAW_PTR], Some(scoop_lir::MANAGED_PTR)),
                NoGc::ContextRestore => {
                    (vec![scoop_lir::MANAGED_PTR, scoop_lir::MANAGED_PTR], None)
                }
                NoGc::ContextSnapshot | NoGc::ContextCurrent => {
                    (Vec::new(), Some(scoop_lir::MANAGED_PTR))
                }
                NoGc::ContextEnter => (vec![scoop_lir::MANAGED_PTR], Some(scoop_lir::MANAGED_PTR)),
                NoGc::ContextLeave => (vec![scoop_lir::MANAGED_PTR], None),
                NoGc::GcStats => (Vec::new(), Some(LirType::I64)),
                NoGc::StringCompare => (
                    vec![scoop_lir::MANAGED_PTR, scoop_lir::MANAGED_PTR],
                    Some(LirType::I64),
                ),
                NoGc::Trap => (vec![scoop_lir::RAW_PTR], None),
                NoGc::Throw => (vec![scoop_lir::MANAGED_PTR], None),
                NoGc::Rethrow => (Vec::new(), None),
                NoGc::UnboxZst
                | NoGc::UnboxValue
                | NoGc::PushRecursiveRegion
                | NoGc::PopRecursiveRegion => unreachable!(
                    "dedicated operations are rejected before runtime signature lookup"
                ),
            };
            (arguments, result, CallProtocol::NoGc)
        }
    }
}

pub(super) fn validate_native_borrowed_publication(
    function: &Function,
    view: &scoop_lir::NativeBorrowedTypedCallView<'_>,
) -> Result<(), CodegenError> {
    match (view.result, &view.call) {
        (
            scoop_lir::NativeBorrowedResultPublication::Void,
            scoop_lir::TypedCallView::Void { .. },
        )
        | (
            scoop_lir::NativeBorrowedResultPublication::ElidedZst,
            scoop_lir::TypedCallView::ElidedZst { .. },
        ) => Ok(()),
        (
            scoop_lir::NativeBorrowedResultPublication::DirectGcFree,
            scoop_lir::TypedCallView::Direct { signature, .. },
        ) if signature.result().scan() == &RefScan::None => Ok(()),
        (
            scoop_lir::NativeBorrowedResultPublication::IndirectResultGcFree,
            scoop_lir::TypedCallView::IndirectResult { signature, .. },
        ) if signature.result().scan() == &RefScan::None => Ok(()),
        (
            scoop_lir::NativeBorrowedResultPublication::DirectRooted { storage, scan },
            scoop_lir::TypedCallView::Direct { signature, .. },
        ) if scan.as_ref_scan() == signature.result().scan() => require_local_type(
            function,
            storage,
            signature.result().storage_type(),
            "native-borrowed direct result root",
        ),
        (
            scoop_lir::NativeBorrowedResultPublication::IndirectResultRooted { storage, scan },
            scoop_lir::TypedCallView::IndirectResult {
                signature,
                storage: result_storage,
                ..
            },
        ) if storage == *result_storage && scan.as_ref_scan() == signature.result().scan() => {
            require_local_type(
                function,
                storage,
                signature.result().storage_type(),
                "native-borrowed indirect result root",
            )
        }
        _ => Err(call_error(
            function,
            "native-borrowed result publication disagrees with its typed result convention",
        )),
    }
}
