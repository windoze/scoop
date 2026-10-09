use super::*;

pub(super) fn validate_destination(
    module: &Module,
    function: &Function,
    call: &scoop_lir::TypedCallView<'_>,
    protocol: CallProtocol,
) -> Result<(), CodegenError> {
    let convention = indirect_result_convention(call);
    if convention == Some(scoop_lir::IndirectResultConvention::CStoragePointer)
        && !matches!(
            protocol,
            CallProtocol::NativeSafe | CallProtocol::NativeGcLeaf | CallProtocol::ReleaseNativeLeaf
        )
    {
        return Err(call_error(
            function,
            "C storage result pointer is only valid for a native-safe C bridge call",
        ));
    }

    match call.destination() {
        scoop_lir::CallDestination::Local(id) => {
            let Some(declaration) = module.functions.get(id.into_u32() as usize) else {
                return Err(call_error(
                    function,
                    format!("references invalid local function id {}", id.into_u32()),
                ));
            };
            let expected_protocol = match declaration.gc_effect {
                scoop_lir::GcEffect::Managed => CallProtocol::Managed,
                scoop_lir::GcEffect::NoGc => CallProtocol::NoGc,
            };
            if protocol != expected_protocol {
                return Err(call_error(
                    function,
                    format!(
                        "{} protocol does not match @{}'s {:?} effect",
                        protocol.name(),
                        declaration.symbol(),
                        declaration.gc_effect
                    ),
                ));
            }
            require_abi_signature(
                function,
                call,
                &declaration.signature,
                &format!("typed local call to @{}", declaration.symbol()),
            )
        }
        scoop_lir::CallDestination::External(id) => {
            let index = arena_index(id);
            if index >= module.meta.external_callables.len() {
                return Err(call_error(
                    function,
                    format!("references invalid external callable {index}"),
                ));
            }
            let declaration = &module.meta.external_callables[id];
            let expected_protocol = match declaration.gc_effect() {
                scoop_lir::GcEffect::Managed => CallProtocol::Managed,
                scoop_lir::GcEffect::NoGc => CallProtocol::NoGc,
            };
            if protocol != expected_protocol {
                return Err(call_error(
                    function,
                    format!(
                        "{} protocol does not match external `{}`'s {:?} effect",
                        protocol.name(),
                        declaration.expected_symbol().symbol(),
                        declaration.gc_effect()
                    ),
                ));
            }
            require_abi_signature(
                function,
                call,
                declaration.signature(),
                &format!(
                    "typed external call to `{}`",
                    declaration.expected_symbol().symbol()
                ),
            )
        }
        scoop_lir::CallDestination::Extern(id) => {
            let declaration = extern_declaration(module, function, id)?;
            match &declaration.kind {
                ExternFunctionKind::C {
                    signature,
                    call_mode,
                    call_plan,
                } => c_calls::validate(
                    function,
                    call,
                    protocol,
                    declaration,
                    signature,
                    *call_mode,
                    call_plan,
                ),
                ExternFunctionKind::Scoop { signature, .. } => {
                    if protocol != CallProtocol::NativeBorrowed {
                        return Err(call_error(
                            function,
                            format!(
                                "Scoop extern `{}` requires the native-borrowed protocol",
                                declaration.source_name
                            ),
                        ));
                    }
                    require_abi_signature(
                        function,
                        call,
                        signature,
                        &format!("Scoop extern `{}`", declaration.source_name),
                    )
                }
            }
        }
        scoop_lir::CallDestination::Runtime(runtime) => {
            validate_runtime_call(function, call, protocol, runtime)
        }
        scoop_lir::CallDestination::Dispatch { .. } => {
            if matches!(
                protocol,
                CallProtocol::NativeSafe
                    | CallProtocol::NativeBorrowed
                    | CallProtocol::NativeGcLeaf
                    | CallProtocol::ReleaseNativeLeaf
            ) {
                return Err(call_error(
                    function,
                    "dynamic dispatch cannot use a native transition protocol",
                ));
            }
            if convention == Some(scoop_lir::IndirectResultConvention::CStoragePointer) {
                return Err(call_error(
                    function,
                    "dynamic dispatch cannot use a C storage result pointer",
                ));
            }
            Ok(())
        }
    }
}

pub(super) fn extern_declaration<'a>(
    module: &'a Module,
    function: &Function,
    id: scoop_lir::ExternFunctionId,
) -> Result<&'a scoop_lir::ExternFunction, CodegenError> {
    let index = arena_index(id);
    if index >= module.extern_functions.iter().count() {
        return Err(call_error(
            function,
            format!("references invalid extern function id {index}"),
        ));
    }
    Ok(&module.extern_functions[id])
}

pub(super) fn require_abi_signature(
    function: &Function,
    call: &scoop_lir::TypedCallView<'_>,
    expected: &scoop_lir::ScoopAbiSignature,
    callee: &str,
) -> Result<(), CodegenError> {
    let matches = call.arguments() == expected.arguments()
        && call_calling_convention(call) == expected.calling_convention()
        && match (call, expected.result()) {
            (scoop_lir::TypedCallView::Void { .. }, scoop_lir::AbiReturn::UnitVoid) => true,
            (
                scoop_lir::TypedCallView::ElidedZst { signature, .. },
                scoop_lir::AbiReturn::ElidedZst(result),
            ) => signature.result() == result,
            (
                scoop_lir::TypedCallView::Direct { signature, .. },
                scoop_lir::AbiReturn::Direct(result),
            ) => signature.result() == result,
            (
                scoop_lir::TypedCallView::IndirectResult { signature, .. },
                scoop_lir::AbiReturn::Indirect(result),
            ) => {
                signature.convention() == scoop_lir::IndirectResultConvention::Sret
                    && signature.result() == result
            }
            _ => false,
        };
    if !matches {
        let result = expected
            .result()
            .logical_storage_type()
            .map(LirType::dump)
            .unwrap_or_else(|| "void".to_string());
        return Err(call_error(
            function,
            format!(
                "{callee} signature or physical convention does not match its authoritative ABI declaration returning {result}"
            ),
        ));
    }
    Ok(())
}
