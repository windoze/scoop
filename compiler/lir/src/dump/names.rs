use super::*;

pub(super) fn block_name(function: &Function, id: BlockId) -> String {
    function.blocks[id].name.clone()
}

pub(super) fn value_name(value: Value) -> String {
    match value {
        Value::Local(id) => format!("local{}", id.into_raw()),
        Value::Param(index) => format!("param{index}"),
        Value::Temp(id) => format!("t{}", id.into_raw()),
        Value::IntConst(value) => format!("{value}"),
        Value::MachineScalar(value) => {
            format!("machine<{}>({value:?})", value.kind().name())
        }
        Value::BoolConst(value) => format!("{value}"),
        Value::NullPointer(kind) => format!("null<{}>", kind.dump()),
        Value::TypeDescriptor(reference) => type_descriptor_ref_name(reference),
        Value::RootScan(id) => format!("root-scan{}", id.into_raw()),
        Value::Global(id) => format!("global{}", id.into_raw()),
        Value::InitializationUnit(id) => format!("init{}", id.into_raw()),
    }
}

pub(super) fn type_descriptor_ref_name(reference: TypeDescriptorRef) -> String {
    match reference {
        TypeDescriptorRef::Local(id) => format!("td{}", id.into_raw()),
        TypeDescriptorRef::External(id) => format!("external-td{}", id.into_raw()),
    }
}

pub(super) fn callable_ref_name(reference: CallableRef) -> String {
    match reference {
        CallableRef::Local(id) => format!("local-fn{}", id.into_u32()),
        CallableRef::Runtime(function) => format!("runtime@{}", function.symbol()),
        CallableRef::External(id) => format!("external-fn{}", id.into_raw()),
    }
}

pub(super) fn call_destination_name(function: &Function, destination: CallDestination) -> String {
    match destination {
        CallDestination::Local(id) => format!("local-fn{}", id.into_u32()),
        CallDestination::Runtime(runtime) => format!("runtime @{}", runtime.symbol()),
        CallDestination::Extern(id) => format!("extern{}", id.into_raw()),
        CallDestination::Dispatch { table, slot } => {
            let slot = function.call_targets.dispatch_slots[slot];
            format!(
                "dispatch[{:?}:{}] {}",
                slot.kind,
                slot.index,
                value_name(table)
            )
        }
    }
}

pub(super) fn typed_call_name(function: &Function, call: &TypedCallView<'_>) -> String {
    let args = call
        .args()
        .iter()
        .map(|arg| value_name(*arg))
        .collect::<Vec<_>>()
        .join(", ");
    match call {
        TypedCallView::Void {
            signature_id,
            destination,
            signature,
            ..
        } => {
            let params = signature
                .params
                .iter()
                .map(LirType::dump)
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "sig=void{} ({params}) {}({args})",
                signature_id,
                call_destination_name(function, *destination),
            )
        }
        TypedCallView::Direct {
            signature_id,
            destination,
            signature,
            out,
            ..
        } => {
            let params = signature
                .params
                .iter()
                .map(LirType::dump)
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "t{} = sig=direct{} ({params}) -> {} {}({args})",
                out.into_raw(),
                signature_id,
                signature.result.dump(),
                call_destination_name(function, *destination),
            )
        }
        TypedCallView::IndirectResult {
            signature_id,
            destination,
            signature,
            storage,
            ..
        } => {
            let params = signature
                .params
                .iter()
                .map(LirType::dump)
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "local{} = sig=indirect{} (sret {}, {params}) {}({args})",
                storage.into_raw(),
                signature_id,
                signature.result.ty.dump(),
                call_destination_name(function, *destination),
            )
        }
    }
}

pub(super) fn typed_target_name(protocol: &str, call: &TypedCallView<'_>) -> String {
    format!(
        "{protocol}-{}-target{}",
        call.return_convention_name(),
        call.target_raw()
    )
}

pub(super) fn caller_roots_name(roots: &[CallerRoot]) -> String {
    roots
        .iter()
        .map(|root| {
            let source = root.source.dump();
            match root.scan.as_ref_scan() {
                RefScan::References(offsets) if offsets == &[0] => source,
                scan => format!("{source}:{}", scan.dump()),
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn exceptional_roots_name(roots: &ExceptionalRootSet) -> String {
    roots
        .as_slice()
        .iter()
        .map(|root| {
            let edge = match (root.normal_live, root.unwind_live) {
                (true, true) => "normal+unwind",
                (true, false) => "normal",
                (false, true) => "unwind",
                (false, false) => "argument",
            };
            format!(
                "{}:{edge}",
                caller_roots_name(std::slice::from_ref(&root.root))
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn live_set_name(live: &StatepointLiveSet) -> String {
    live.as_slice()
        .iter()
        .map(|value| {
            let offsets = value
                .leaves
                .as_slice()
                .iter()
                .map(|leaf| leaf.byte_offset.to_string())
                .collect::<Vec<_>>()
                .join("+");
            format!("{}:{}@{offsets}", value.source.dump(), value.ty.dump())
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn call_site_name(function: &Function, site: &CallSite) -> String {
    let targets = &function.call_targets;
    match site {
        CallSite::Managed(site) => {
            let call = targets.typed_call_view(
                &site.call,
                &targets.managed_targets,
                ManagedCallDestination::view,
            );
            format!(
                "{} sp{} live=[{}] {}",
                typed_target_name("managed", &call),
                site.safepoint.get(),
                live_set_name(&site.live),
                typed_call_name(function, &call)
            )
        }
        CallSite::NoGc(site) => {
            let call = targets.typed_call_view(
                &site.call,
                &targets.no_gc_targets,
                NoGcCallDestination::view,
            );
            format!(
                "{} {}",
                typed_target_name("no-gc", &call),
                typed_call_name(function, &call)
            )
        }
        CallSite::NativeSafe(site) => {
            let call = targets.typed_call_view(
                &site.call,
                &targets.native_safe_targets,
                NativeSafeCallDestination::view,
            );
            format!(
                "{} sp{} roots=[{}] {}",
                typed_target_name("native-safe", &call),
                site.safepoint.get(),
                caller_roots_name(site.roots.as_slice()),
                typed_call_name(function, &call)
            )
        }
        CallSite::NativeBorrowed(site) => {
            let native = site.call.view(targets);
            let call = native.call;
            let result = match native.result {
                NativeBorrowedResultPublication::DirectRooted { storage, scan }
                | NativeBorrowedResultPublication::IndirectResultRooted { storage, scan } => {
                    format!(" result-root=local{}:{}", storage.into_raw(), scan.dump())
                }
                NativeBorrowedResultPublication::Void
                | NativeBorrowedResultPublication::DirectGcFree
                | NativeBorrowedResultPublication::IndirectResultGcFree => String::new(),
            };
            format!(
                "{} sp{} roots=[{}]{result} {}",
                typed_target_name("native-borrowed", &call),
                site.safepoint.get(),
                caller_roots_name(site.roots.as_slice()),
                typed_call_name(function, &call)
            )
        }
    }
}

pub(super) fn invoke_site_name(function: &Function, site: &InvokeSite) -> String {
    let targets = &function.call_targets;
    match site {
        InvokeSite::Managed(site) => {
            let call = targets.typed_call_view(
                &site.call,
                &targets.managed_targets,
                ManagedCallDestination::view,
            );
            format!(
                "{} sp{} roots=[{}] {} normal @{} unwind @{}",
                typed_target_name("managed", &call),
                site.safepoint.get(),
                exceptional_roots_name(&site.roots),
                typed_call_name(function, &call),
                block_name(function, site.normal),
                block_name(function, site.unwind)
            )
        }
        InvokeSite::NoGc(site) => {
            let call = targets.typed_call_view(
                &site.call,
                &targets.no_gc_targets,
                NoGcCallDestination::view,
            );
            format!(
                "{} {} normal @{} unwind @{}",
                typed_target_name("no-gc", &call),
                typed_call_name(function, &call),
                block_name(function, site.normal),
                block_name(function, site.unwind)
            )
        }
    }
}
