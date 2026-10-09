use super::*;

pub(super) fn validate(
    function: &Function,
    call: &scoop_lir::TypedCallView<'_>,
    protocol: CallProtocol,
    declaration: &scoop_lir::ExternFunction,
    c_signature: &scoop_lir::CFunctionType,
    call_mode: scoop_lir::CAbiCallMode,
    call_plan: &scoop_lir::CAbiCallPlan,
) -> Result<(), CodegenError> {
    if !matches!(
        (protocol, call_mode),
        (
            CallProtocol::NativeSafe,
            scoop_lir::CAbiCallMode::NativeSafe
        ) | (CallProtocol::NativeGcLeaf, scoop_lir::CAbiCallMode::GcLeaf)
            | (CallProtocol::ReleaseNativeLeaf, _)
    ) {
        return Err(call_error(
            function,
            format!(
                "C extern `{}` requires its declared C caller protocol",
                declaration.source_name
            ),
        ));
    }
    if call_calling_convention(call) != declaration.calling_convention {
        return Err(call_error(
            function,
            format!(
                "C extern `{}` calling convention disagrees with its declaration",
                declaration.source_name
            ),
        ));
    }
    if let scoop_lir::CAbiCallPlan::Direct(plan) = call_plan {
        if call.args().iter().any(|argument| {
            matches!(
                argument,
                scoop_lir::AbiCallArgument::Direct(Value::CArgumentStorage(_))
            )
        }) {
            return Err(call_error(
                function,
                format!(
                    "C extern `{}` DirectC call cannot use bridge argument storage",
                    declaration.source_name
                ),
            ));
        }
        return require_abi_signature(
            function,
            call,
            &plan.abi_signature(),
            &format!("C extern `{}`", declaration.source_name),
        );
    }

    let capture = matches!(
        call_plan,
        scoop_lir::CAbiCallPlan::StorageBridge {
            result: scoop_lir::CResultAdaptation::CaptureErrno,
            ..
        }
    );
    let result_pointer = usize::from(capture && !c_signature.return_type.is_void());
    if call.arguments().len() != c_signature.params.len() + result_pointer
        || call.arguments().iter().any(|argument| {
            !matches!(argument, scoop_lir::AbiArgument::Direct(value)
                if value.storage_type() == &scoop_lir::RAW_PTR
                    && value.scan() == &RefScan::None)
        })
    {
        return Err(call_error(
            function,
            format!(
                "C extern `{}` bridge parameters must be direct raw storage pointers",
                declaration.source_name
            ),
        ));
    }

    if result_pointer == 1 {
        let scoop_lir::AbiCallArgument::Direct(Value::CArgumentStorage(storage)) = call.args()[0]
        else {
            return Err(call_error(
                function,
                "errno bridge requires exact native result storage",
            ));
        };
        require_local_type(
            function,
            storage.local(),
            &c_signature.storage_return_type(),
            "errno bridge native result storage",
        )?;
    }
    for (index, (argument, parameter)) in call
        .args()
        .iter()
        .skip(result_pointer)
        .zip(&c_signature.params)
        .enumerate()
    {
        let scoop_lir::AbiCallArgument::Direct(Value::CArgumentStorage(storage)) = argument else {
            return Err(call_error(
                function,
                format!(
                    "C extern `{}` argument {index} is not an exact C argument-storage address",
                    declaration.source_name
                ),
            ));
        };
        require_local_type(
            function,
            storage.local(),
            &parameter.storage_type(),
            &format!(
                "C extern `{}` argument {index} storage",
                declaration.source_name
            ),
        )?;
    }

    let result_matches = if capture {
        matches!(call, scoop_lir::TypedCallView::Direct { signature, .. }
            if signature.result().storage_type() == &scoop_lir::LirType::I32
                && signature.result().scan() == &RefScan::None)
    } else {
        match (&c_signature.return_type, call) {
            (scoop_lir::CReturnType::Void, scoop_lir::TypedCallView::Void { .. }) => true,
            (
                scoop_lir::CReturnType::Value(_),
                scoop_lir::TypedCallView::IndirectResult { signature, .. },
            ) => {
                signature.convention() == scoop_lir::IndirectResultConvention::CStoragePointer
                    && signature.result().storage_type() == &c_signature.storage_return_type()
                    && signature.result().scan() == &RefScan::None
            }
            _ => false,
        }
    };
    if !result_matches {
        return Err(call_error(
            function,
            format!(
                "C extern `{}` result does not use its declared storage bridge convention",
                declaration.source_name
            ),
        ));
    }
    Ok(())
}
