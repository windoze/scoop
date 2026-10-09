use super::*;

pub(super) fn validate_call_site(
    module: &Module,
    function: &Function,
    site: &scoop_lir::CallSite,
) -> Result<(), CodegenError> {
    let targets = &function.call_targets;
    match site {
        scoop_lir::CallSite::Managed(site) => {
            let call = checked_call_view(
                function,
                &site.call,
                &targets.managed_targets,
                scoop_lir::ManagedCallDestination::view,
            )?;
            validate_call(module, function, call, CallProtocol::Managed)
        }
        scoop_lir::CallSite::NoGc(site) | scoop_lir::CallSite::ReleaseScoop(site) => {
            let call = checked_call_view(
                function,
                &site.call,
                &targets.no_gc_targets,
                scoop_lir::NoGcCallDestination::view,
            )?;
            validate_call(module, function, call, CallProtocol::NoGc)
        }
        scoop_lir::CallSite::NativeSafe(site) => {
            let call = checked_call_view(
                function,
                &site.call,
                &targets.c_targets,
                scoop_lir::CCallDestination::view,
            )?;
            validate_call(module, function, call, CallProtocol::NativeSafe)
        }
        scoop_lir::CallSite::ReleaseNativeLeaf(site) => {
            if function.callable_body.release_owner().is_none() {
                return Err(call_error(
                    function,
                    "a release native leaf requires a release hook owner",
                ));
            }
            let call = checked_call_view(
                function,
                &site.call,
                &targets.c_targets,
                scoop_lir::CCallDestination::view,
            )?;
            validate_call(module, function, call, CallProtocol::ReleaseNativeLeaf)
        }
        scoop_lir::CallSite::NativeGcLeaf(site) => {
            let call = checked_call_view(
                function,
                &site.call,
                &targets.c_targets,
                scoop_lir::CCallDestination::view,
            )?;
            validate_call(module, function, call, CallProtocol::NativeGcLeaf)
        }
        scoop_lir::CallSite::NativeBorrowed(site) => {
            // Native-borrowed calls are sealed by `CallTargets`: construction
            // already checked the target/signature ids and result publication.
            let call = site.call.view(targets);
            validate_native_borrowed_publication(function, &call)?;
            validate_call(module, function, call.call, CallProtocol::NativeBorrowed)
        }
    }
}

pub(super) fn validate_invoke_site(
    module: &Module,
    function: &Function,
    site: &scoop_lir::InvokeSite,
) -> Result<(), CodegenError> {
    let targets = &function.call_targets;
    match site {
        scoop_lir::InvokeSite::Managed(site) => {
            let call = checked_call_view(
                function,
                &site.call,
                &targets.managed_targets,
                scoop_lir::ManagedCallDestination::view,
            )?;
            validate_call(module, function, call, CallProtocol::Managed)
        }
        scoop_lir::InvokeSite::NoGc(site) => {
            let call = checked_call_view(
                function,
                &site.call,
                &targets.no_gc_targets,
                scoop_lir::NoGcCallDestination::view,
            )?;
            validate_call(module, function, call, CallProtocol::NoGc)
        }
    }
}

pub(super) fn checked_call_view<'a, Destination: Copy>(
    function: &'a Function,
    call: &'a scoop_lir::TypedCall<Destination>,
    targets: &'a scoop_lir::ProtocolCallTargets<Destination>,
    destination_view: fn(Destination) -> scoop_lir::CallDestination,
) -> Result<scoop_lir::TypedCallView<'a>, CodegenError> {
    let all = &function.call_targets;
    let invalid_target = |convention: &str, index: usize| {
        CodegenError(format!(
            "typed call @{} references invalid {convention} target {index}",
            function.symbol()
        ))
    };
    let invalid_signature = |convention: &str, index: usize| {
        CodegenError(format!(
            "typed call @{} references invalid {convention} signature {index}",
            function.symbol()
        ))
    };

    match call {
        scoop_lir::TypedCall::Void { target, .. } => {
            let index = arena_index(*target);
            if index >= targets.void.len() {
                return Err(invalid_target("void", index));
            }
            let signature = targets.void[*target].signature;
            let index = arena_index(signature);
            if index >= all.void_signatures.len() {
                return Err(invalid_signature("void", index));
            }
        }
        scoop_lir::TypedCall::ElidedZst { target, .. } => {
            let index = arena_index(*target);
            if index >= targets.elided_zst.len() {
                return Err(invalid_target("elided-ZST", index));
            }
            let signature = targets.elided_zst[*target].signature;
            let index = arena_index(signature);
            if index >= all.elided_zst_signatures.len() {
                return Err(invalid_signature("elided-ZST", index));
            }
        }
        scoop_lir::TypedCall::Direct { target, .. } => {
            let index = arena_index(*target);
            if index >= targets.direct.len() {
                return Err(invalid_target("direct-result", index));
            }
            let signature = targets.direct[*target].signature;
            let index = arena_index(signature);
            if index >= all.direct_signatures.len() {
                return Err(invalid_signature("direct-result", index));
            }
        }
        scoop_lir::TypedCall::IndirectResult { target, .. } => {
            let index = arena_index(*target);
            if index >= targets.indirect_result.len() {
                return Err(invalid_target("indirect-result", index));
            }
            let signature = targets.indirect_result[*target].signature;
            let index = arena_index(signature);
            if index >= all.indirect_result_signatures.len() {
                return Err(invalid_signature("indirect-result", index));
            }
        }
    }

    Ok(all.typed_call_view(call, targets, destination_view))
}

pub(super) fn validate_call(
    module: &Module,
    function: &Function,
    call: scoop_lir::TypedCallView<'_>,
    protocol: CallProtocol,
) -> Result<(), CodegenError> {
    let expected_arguments = call.arguments();
    let actual_arguments = call.args();
    if actual_arguments.len() != expected_arguments.len() {
        return Err(call_error(
            function,
            format!(
                "signature has {} logical arguments but call has {}",
                expected_arguments.len(),
                actual_arguments.len()
            ),
        ));
    }

    for (index, (actual, expected)) in actual_arguments.iter().zip(expected_arguments).enumerate() {
        validate_argument(module, function, index, *actual, expected)?;
    }
    validate_result(function, &call)?;
    validate_destination(module, function, &call, protocol)?;

    let is_c_extern = matches!(
        call.destination(),
        scoop_lir::CallDestination::Extern(id)
            if extern_declaration(module, function, id).is_ok_and(|declaration| {
                matches!(declaration.kind, ExternFunctionKind::C { .. })
            })
    );
    for argument in actual_arguments {
        if matches!(
            argument,
            scoop_lir::AbiCallArgument::Direct(Value::CArgumentStorage(_))
        ) && !is_c_extern
        {
            return Err(call_error(
                function,
                "uses a C argument-storage address outside a C extern call",
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_argument(
    module: &Module,
    function: &Function,
    index: usize,
    actual: scoop_lir::AbiCallArgument,
    expected: &scoop_lir::AbiArgument,
) -> Result<(), CodegenError> {
    let (value, expected_type) = match (actual, expected) {
        (
            scoop_lir::AbiCallArgument::ElidedZst(value),
            scoop_lir::AbiArgument::ElidedZst(expected),
        ) => (value, expected.storage_type()),
        (scoop_lir::AbiCallArgument::Direct(value), scoop_lir::AbiArgument::Direct(expected)) => {
            (value, expected.storage_type())
        }
        (
            scoop_lir::AbiCallArgument::Indirect(storage),
            scoop_lir::AbiArgument::Indirect(expected),
        ) => {
            let local = storage.local();
            let local_index = arena_index(local);
            if local_index >= function.locals.len() {
                return Err(call_error(
                    function,
                    format!("indirect argument {index} references invalid local {local_index}"),
                ));
            }
            let actual_type = function.locals[local].ty();
            if actual_type != expected.storage_type() {
                return Err(call_error(
                    function,
                    format!(
                        "indirect argument {index} storage local{local_index} has type {}, expected exact {}",
                        actual_type.dump(),
                        expected.storage_type().dump()
                    ),
                ));
            }
            return Ok(());
        }
        (actual, expected) => {
            return Err(call_error(
                function,
                format!(
                    "argument {index} uses {} passing, expected {}",
                    call_argument_convention(actual),
                    argument_convention(expected)
                ),
            ));
        }
    };

    let actual_type = crate::validation::checked_value_type(
        module,
        function,
        value,
        &format!("typed call argument {index}"),
    )?;
    if &actual_type != expected_type {
        return Err(call_error(
            function,
            format!(
                "argument {index} has type {}, expected exact {}",
                actual_type.dump(),
                expected_type.dump()
            ),
        ));
    }
    Ok(())
}

pub(super) fn validate_result(
    function: &Function,
    call: &scoop_lir::TypedCallView<'_>,
) -> Result<(), CodegenError> {
    match call {
        scoop_lir::TypedCallView::Void { .. } => Ok(()),
        scoop_lir::TypedCallView::ElidedZst { signature, out, .. } => require_temp_type(
            function,
            *out,
            signature.result().storage_type(),
            "elided-ZST",
        ),
        scoop_lir::TypedCallView::Direct { signature, out, .. } => {
            require_temp_type(function, *out, signature.result().storage_type(), "direct")
        }
        scoop_lir::TypedCallView::IndirectResult {
            signature, storage, ..
        } => require_local_type(
            function,
            *storage,
            signature.result().storage_type(),
            "indirect result",
        ),
    }
}

pub(super) fn require_temp_type(
    function: &Function,
    temp: TempId,
    expected: &LirType,
    convention: &str,
) -> Result<(), CodegenError> {
    let actual = crate::validation::checked_temp_type(function, temp, "typed call result")?;
    if actual != expected {
        return Err(call_error(
            function,
            format!(
                "{convention} result temporary has type {}, expected exact {}",
                actual.dump(),
                expected.dump()
            ),
        ));
    }
    Ok(())
}

pub(super) fn require_local_type(
    function: &Function,
    local: scoop_lir::LocalId,
    expected: &LirType,
    owner: &str,
) -> Result<(), CodegenError> {
    let index = arena_index(local);
    if index >= function.locals.len() {
        return Err(call_error(
            function,
            format!("{owner} references invalid local {index}"),
        ));
    }
    let actual = function.locals[local].ty();
    if actual != expected {
        return Err(call_error(
            function,
            format!(
                "{owner} local{index} has type {}, expected exact {}",
                actual.dump(),
                expected.dump()
            ),
        ));
    }
    Ok(())
}
