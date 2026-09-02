use super::*;

/// Indented text dump for golden tests (`scoopc build --emit=lir`).
pub fn dump(module: &Module) -> String {
    let mut out = String::from("Module\n");
    for (_, global) in module.globals.iter() {
        match &global.init {
            GlobalInit::StringConst(value) => {
                out.push_str(&format!("  global @{} = {:?}\n", global.symbol, value));
            }
            GlobalInit::CString(value) => {
                out.push_str(&format!("  global @{} = c{:?}\n", global.symbol, value));
            }
            GlobalInit::Storage {
                ty, thread_local, ..
            } => out.push_str(&format!(
                "  {} @{} : {} scan={}\n",
                if *thread_local {
                    "thread_local"
                } else {
                    "global"
                },
                global.symbol,
                ty.dump(),
                global.scan.dump()
            )),
        }
    }
    for (id, global) in module.native_globals.iter() {
        let access = match global.access {
            NativeGlobalAccess::ReadOnly { get, address } => format!(
                "readonly(get=ng-get{},address=ng-address{})",
                get.into_raw(),
                address.into_raw()
            ),
            NativeGlobalAccess::Mutable { get, set, address } => format!(
                "mutable(get=ng-get{},set=ng-set{},address=ng-address{})",
                get.into_raw(),
                set.into_raw(),
                address.into_raw()
            ),
        };
        out.push_str(&format!(
            "  native_global{} {} @{} : {} {}{}\n",
            id.into_raw(),
            global.source_name,
            global.native_symbol,
            global.ty.dump(),
            access,
            if global.thread_local {
                " thread_local"
            } else {
                ""
            }
        ));
    }
    for (_, def) in module.enums.iter() {
        match &def.repr {
            EnumRepr::Niche { payload_variant } => out.push_str(&format!(
                "  enum {} niche(payload_variant={})\n",
                def.name, payload_variant
            )),
            EnumRepr::Tagged {
                variants,
                size,
                align,
            } => {
                let variants: Vec<String> = variants
                    .iter()
                    .map(|variant| {
                        let inner: Vec<String> =
                            variant.fields.iter().map(|field| field.ty.dump()).collect();
                        format!(
                            "({})@{}+{}{}",
                            inner.join(", "),
                            variant.slot_offset,
                            variant.slot_size,
                            if variant.gc_free { "" } else { ":refs" }
                        )
                    })
                    .collect();
                out.push_str(&format!(
                    "  enum {} tagged size={} align={} variants={}\n",
                    def.name,
                    size,
                    align,
                    variants.join(" ")
                ));
            }
        }
    }
    for (id, extern_) in module.extern_functions.iter() {
        let params = extern_
            .params
            .iter()
            .map(LirType::dump)
            .collect::<Vec<_>>()
            .join(", ");
        let kind = match &extern_.kind {
            ExternFunctionKind::C { bridge_symbol, .. } => {
                format!("c bridge=@{bridge_symbol} gc-leaf nounwind")
            }
            ExternFunctionKind::Scoop { gc_effect } => format!(
                "scoop {} nounwind",
                if *gc_effect == GcEffect::NoGc {
                    "gc-leaf"
                } else {
                    "managed"
                }
            ),
        };
        let library = if extern_.library.is_empty() {
            String::new()
        } else {
            format!(" lib={}", extern_.library)
        };
        out.push_str(&format!(
            "  extern ef{} {} @{}({}) -> {} <{}{}>\n",
            id.into_raw(),
            extern_.source_name,
            extern_.native_symbol,
            params,
            extern_.return_type.dump(),
            kind,
            library
        ));
    }
    for (id, callback) in module.callback_bridges.iter() {
        out.push_str(&format!(
            "  callback cb{} {} @{} -> @{}\n",
            id.into_raw(),
            callback.source_name,
            callback.bridge_symbol,
            callback.trampoline_symbol
        ));
    }
    for function in &module.functions {
        let params: Vec<String> = function.params.iter().map(LirType::dump).collect();
        out.push_str(&format!(
            "  fun @{}({}) -> {}{}\n",
            function.symbol,
            params.join(", "),
            function.return_ty.dump(),
            if function.gc_effect == GcEffect::NoGc {
                " <no-gc>"
            } else {
                ""
            }
        ));
        for (id, local) in function.locals.iter() {
            out.push_str(&format!(
                "    local %{} {}: {}\n",
                id.into_raw(),
                local.name,
                local.ty.dump()
            ));
        }
        for (block_id, block) in function.blocks.iter() {
            let _ = block_id;
            out.push_str(&format!("  block {}\n", block.name));
            for instruction in &block.instructions {
                dump_instruction(function, instruction, &mut out);
            }
            match &block.terminator {
                Terminator::Br(target) => {
                    out.push_str(&format!("    br @{}\n", block_name(function, *target)))
                }
                Terminator::CondBr {
                    cond,
                    then_block,
                    else_block,
                } => out.push_str(&format!(
                    "    cbr {} then @{} else @{}\n",
                    value_name(*cond),
                    block_name(function, *then_block),
                    block_name(function, *else_block)
                )),
                Terminator::Return { value } => match value {
                    Some(value) => out.push_str(&format!("    ret {}\n", value_name(*value))),
                    None => out.push_str("    ret\n"),
                },
                Terminator::Resume { exception } => {
                    out.push_str(&format!("    resume {}\n", value_name(*exception)))
                }
                Terminator::Unreachable => out.push_str("    unreachable\n"),
            }
        }
    }
    for (id, td) in module.meta.type_descriptors.iter() {
        let reference = TypeDescriptorRef::Local(id);
        if reference == module.meta.well_known_type_descriptors.string
            || module
                .meta
                .arrays
                .iter()
                .any(|(_, array)| array.type_descriptor == reference)
        {
            continue;
        }
        let parent = td
            .parent
            .map(type_descriptor_ref_name)
            .unwrap_or_else(|| "none".to_string());
        let vtable = td
            .vtable
            .iter()
            .map(|entry| callable_ref_name(entry.callable))
            .collect::<Vec<_>>()
            .join(", ");
        let itables = td
            .itables
            .iter()
            .map(|record| {
                let slots = record
                    .slots
                    .iter()
                    .map(|entry| callable_ref_name(entry.callable))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}:[{slots}]", type_descriptor_ref_name(record.interface))
            })
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "  td td{} {} @{} type-id={} size={} parent={} vtable=[{}] itables=[{}]\n",
            id.into_raw(),
            td.name,
            td.symbol,
            td.runtime_type_id,
            td.size,
            parent,
            vtable,
            itables,
        ));
    }
    for (id, array) in module.meta.arrays.iter() {
        let TypeDescriptorRef::Local(descriptor_id) = array.type_descriptor else {
            unreachable!("a local array application owns a local descriptor")
        };
        let descriptor = &module.meta.type_descriptors[descriptor_id];
        let TypeDescriptorScan::ArrayElement { scan, .. } = &descriptor.scan else {
            unreachable!("an array descriptor owns an element scan")
        };
        out.push_str(&format!(
            "  array-type array{} {} kind={} element={} size={} align={} scan={} td={}\n",
            id.into_raw(),
            descriptor.name,
            match array.kind {
                ArrayKind::Immutable => "immutable",
                ArrayKind::Mutable => "mutable",
            },
            array.element.dump(),
            array.element_size,
            array.element_align,
            scan.dump(),
            type_descriptor_ref_name(array.type_descriptor),
        ));
    }
    // Keep the textual dump stable while the typed intrinsic metadata remains
    // available directly on `LirMeta`: String historically appeared first,
    // and the unused UInt scalar was omitted. Tests that validate the intrinsic
    // contract inspect the typed fields instead of reconstructing it from text.
    let string_layout = module.meta.well_known_layouts.string;
    for (_layout_id, layout) in
        std::iter::once((string_layout, &module.meta.layouts[string_layout]))
            .chain(module.meta.layouts.iter().filter(|(_, layout)| {
                matches!(
                    layout.kind,
                    LayoutKind::Intrinsic(IntrinsicTypeRepresentation::Int)
                )
            }))
            .chain(module.meta.layouts.iter().filter(|(_, layout)| {
                matches!(
                    layout.kind,
                    LayoutKind::Intrinsic(IntrinsicTypeRepresentation::Boolean)
                )
            }))
            .chain(module.meta.layouts.iter().filter(|(_, layout)| {
                !matches!(
                    layout.kind,
                    LayoutKind::Intrinsic(
                        IntrinsicTypeRepresentation::Int
                            | IntrinsicTypeRepresentation::UInt
                            | IntrinsicTypeRepresentation::Boolean
                            | IntrinsicTypeRepresentation::String
                    )
                )
            }))
    {
        match &layout.kind {
            LayoutKind::Plain { scan } => match scan {
                RefScan::None => out.push_str(&format!(
                    "  layout {} size={} align={} refs=[]\n",
                    layout.name, layout.size, layout.align
                )),
                RefScan::References(offsets) => out.push_str(&format!(
                    "  layout {} size={} align={} refs={offsets:?}\n",
                    layout.name, layout.size, layout.align
                )),
                _ => out.push_str(&format!(
                    "  layout {} size={} align={} scan={}\n",
                    layout.name,
                    layout.size,
                    layout.align,
                    scan.dump()
                )),
            },
            LayoutKind::Enum { scan } => out.push_str(&format!(
                "  layout {} size={} align={} enum-scan={}\n",
                layout.name,
                layout.size,
                layout.align,
                scan.dump()
            )),
            LayoutKind::Intrinsic(
                IntrinsicTypeRepresentation::Int
                | IntrinsicTypeRepresentation::UInt
                | IntrinsicTypeRepresentation::Boolean
                | IntrinsicTypeRepresentation::String,
            ) => out.push_str(&format!(
                "  layout {} size={} align={} refs=[]\n",
                layout.name, layout.size, layout.align
            )),
        }
        if let Some(c_layout) = layout.c_layout {
            let fields = layout
                .fields
                .iter()
                .map(|field| format!("{}@{}", field.offset, field.access_align))
                .collect::<Vec<_>>()
                .join(",");
            out.push_str(&format!(
                "  layout-meta {} c-layout(aligned={},packed={}) fields=[{}] interior-mutable={}\n",
                layout.name, c_layout.aligned, c_layout.packed, fields, layout.interior_mutable
            ));
        } else if layout.interior_mutable {
            out.push_str(&format!(
                "  layout-meta {} interior-mutable=true\n",
                layout.name
            ));
        }
    }
    out.push_str(&format!("  entry @{}\n", module.entry_symbol));
    out
}

fn block_name(function: &Function, id: BlockId) -> String {
    function.blocks[id].name.clone()
}

fn value_name(value: Value) -> String {
    match value {
        Value::Local(id) => format!("local{}", id.into_raw()),
        Value::Param(index) => format!("param{index}"),
        Value::Temp(id) => format!("t{}", id.into_raw()),
        Value::IntConst(value) => format!("{value}"),
        Value::BoolConst(value) => format!("{value}"),
        Value::NullPointer(kind) => format!("null<{}>", kind.dump()),
        Value::TypeDescriptor(reference) => type_descriptor_ref_name(reference),
        Value::Global(id) => format!("global{}", id.into_raw()),
    }
}

fn type_descriptor_ref_name(reference: TypeDescriptorRef) -> String {
    match reference {
        TypeDescriptorRef::Local(id) => format!("td{}", id.into_raw()),
        TypeDescriptorRef::External(id) => format!("external-td{}", id.into_raw()),
    }
}

fn callable_ref_name(reference: CallableRef) -> String {
    match reference {
        CallableRef::Local(id) => format!("local-fn{}", id.into_u32()),
        CallableRef::Runtime(function) => format!("runtime@{}", function.symbol()),
        CallableRef::External(id) => format!("external-fn{}", id.into_raw()),
    }
}

fn call_destination_name(function: &Function, destination: CallDestination) -> String {
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

fn typed_call_name(function: &Function, call: &TypedCall, destination: CallDestination) -> String {
    let targets = &function.call_targets;
    let args = call
        .args()
        .iter()
        .map(|arg| value_name(*arg))
        .collect::<Vec<_>>()
        .join(", ");
    match *call {
        TypedCall::Void { signature, .. } => {
            let value = &targets.void_signatures[signature];
            let params = value
                .params
                .iter()
                .map(LirType::dump)
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "sig=void{} ({params}) {}({args})",
                signature.into_raw(),
                call_destination_name(function, destination),
            )
        }
        TypedCall::Direct { signature, out, .. } => {
            let value = &targets.direct_signatures[signature];
            let params = value
                .params
                .iter()
                .map(LirType::dump)
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "t{} = sig=direct{} ({params}) -> {} {}({args})",
                out.into_raw(),
                signature.into_raw(),
                value.result.dump(),
                call_destination_name(function, destination),
            )
        }
        TypedCall::IndirectResult {
            signature, storage, ..
        } => {
            let value = &targets.indirect_result_signatures[signature];
            let params = value
                .params
                .iter()
                .map(LirType::dump)
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "local{} = sig=indirect{} (sret {}, {params}) {}({args})",
                storage.into_raw(),
                signature.into_raw(),
                value.result.ty.dump(),
                call_destination_name(function, destination),
            )
        }
    }
}

fn caller_roots_name(roots: &[CallerRoot]) -> String {
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

fn exceptional_roots_name(roots: &ExceptionalRootSet) -> String {
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

fn live_set_name(live: &StatepointLiveSet) -> String {
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

fn call_site_name(function: &Function, site: &CallSite) -> String {
    let targets = &function.call_targets;
    match site {
        CallSite::Managed(site) => format!(
            "managed-target{} sp{} live=[{}] {}",
            site.target.into_raw(),
            site.safepoint.get(),
            live_set_name(&site.live),
            typed_call_name(
                function,
                &site.call,
                targets.managed_targets[site.target].destination
            )
        ),
        CallSite::NoGc(site) => format!(
            "no-gc-target{} {}",
            site.target.into_raw(),
            typed_call_name(
                function,
                &site.call,
                targets.no_gc_targets[site.target].destination
            )
        ),
        CallSite::NativeSafe(site) => format!(
            "native-safe-target{} sp{} roots=[{}] {}",
            site.target.into_raw(),
            site.safepoint.get(),
            caller_roots_name(site.roots.as_slice()),
            typed_call_name(
                function,
                &site.call,
                targets.native_safe_targets[site.target].destination
            )
        ),
        CallSite::NativeBorrowed(site) => {
            let result = match &site.roots.result {
                NativeBorrowedResultRoot::GcFree => String::new(),
                NativeBorrowedResultRoot::Rooted { storage, scan } => {
                    format!(" result-root=local{}:{}", storage.into_raw(), scan.dump())
                }
            };
            format!(
                "native-borrowed-target{} sp{} roots=[{}]{result} {}",
                site.target.into_raw(),
                site.safepoint.get(),
                caller_roots_name(site.roots.as_slice()),
                typed_call_name(
                    function,
                    &site.call,
                    targets.native_borrowed_targets[site.target].destination
                )
            )
        }
    }
}

fn invoke_site_name(function: &Function, site: &InvokeSite) -> String {
    let targets = &function.call_targets;
    match site {
        InvokeSite::Managed(site) => format!(
            "managed-target{} sp{} roots=[{}] {} normal @{} unwind @{}",
            site.target.into_raw(),
            site.safepoint.get(),
            exceptional_roots_name(&site.roots),
            typed_call_name(
                function,
                &site.call,
                targets.managed_targets[site.target].destination
            ),
            block_name(function, site.normal),
            block_name(function, site.unwind)
        ),
        InvokeSite::NoGc(site) => format!(
            "no-gc-target{} {} normal @{} unwind @{}",
            site.target.into_raw(),
            typed_call_name(
                function,
                &site.call,
                targets.no_gc_targets[site.target].destination
            ),
            block_name(function, site.normal),
            block_name(function, site.unwind)
        ),
    }
}

fn dump_instruction(function: &Function, instruction: &Instruction, buf: &mut String) {
    match instruction {
        Instruction::BinOp { out, op, lhs, rhs } => buf.push_str(&format!(
            "    t{} = {:?} {}, {} : {}\n",
            out.into_raw(),
            op,
            value_name(*lhs),
            value_name(*rhs),
            function.temps[*out].ty.dump()
        )),
        Instruction::UnaryOp { out, op, operand } => buf.push_str(&format!(
            "    t{} = {:?} {} : {}\n",
            out.into_raw(),
            op,
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
        Instruction::MakeAggregate { out, elements } => {
            let elements: Vec<String> = elements.iter().map(|e| value_name(*e)).collect();
            buf.push_str(&format!(
                "    t{} = aggregate ({}) : {}\n",
                out.into_raw(),
                elements.join(", "),
                function.temps[*out].ty.dump()
            ))
        }
        Instruction::ExtractValue {
            out,
            aggregate,
            index,
        } => buf.push_str(&format!(
            "    t{} = extract {}, {} : {}\n",
            out.into_raw(),
            value_name(*aggregate),
            index,
            function.temps[*out].ty.dump()
        )),
        Instruction::HeapLoad {
            out,
            object,
            offset,
        } => buf.push_str(&format!(
            "    t{} = heap_load {} +{} : {}\n",
            out.into_raw(),
            value_name(*object),
            offset,
            function.temps[*out].ty.dump()
        )),
        Instruction::AtomicLoad {
            out,
            object,
            offset,
        } => buf.push_str(&format!(
            "    t{} = atomic_load acquire {} +{} : {}\n",
            out.into_raw(),
            value_name(*object),
            offset,
            function.temps[*out].ty.dump()
        )),
        Instruction::GlobalLoad { out, global } => buf.push_str(&format!(
            "    t{} = global_load global{} : {}\n",
            out.into_raw(),
            global.into_raw(),
            function.temps[*out].ty.dump()
        )),
        Instruction::GlobalStore { global, value } => buf.push_str(&format!(
            "    global_store global{}, {}\n",
            global.into_raw(),
            value_name(*value)
        )),
        Instruction::GlobalAddress { out, global } => buf.push_str(&format!(
            "    t{} = global_address global{}\n",
            out.into_raw(),
            global.into_raw()
        )),
        Instruction::NativeGlobalLoad {
            out,
            global,
            safepoint,
            roots,
        } => buf.push_str(&format!(
            "    t{} = native_global_load ng{} sp{} roots=[{}] : {}\n",
            out.into_raw(),
            global.into_raw(),
            safepoint.get(),
            caller_roots_name(roots.as_slice()),
            function.temps[*out].ty.dump()
        )),
        Instruction::NativeGlobalStore {
            global,
            value,
            safepoint,
            roots,
        } => buf.push_str(&format!(
            "    native_global_store ng{}, {} sp{} roots=[{}]\n",
            global.into_raw(),
            value_name(*value),
            safepoint.get(),
            caller_roots_name(roots.as_slice())
        )),
        Instruction::NativeGlobalAddress {
            out,
            global,
            safepoint,
            roots,
        } => buf.push_str(&format!(
            "    t{} = native_global_address ng{} sp{} roots=[{}]\n",
            out.into_raw(),
            global.into_raw(),
            safepoint.get(),
            caller_roots_name(roots.as_slice())
        )),
        Instruction::HeapStore {
            object,
            offset,
            value,
        } => buf.push_str(&format!(
            "    heap_store {} +{} {}\n",
            value_name(*object),
            offset,
            value_name(*value)
        )),
        Instruction::AtomicStore {
            object,
            offset,
            value,
        } => buf.push_str(&format!(
            "    atomic_store release {} +{} {}\n",
            value_name(*object),
            offset,
            value_name(*value)
        )),
        Instruction::AtomicCompareExchange {
            out,
            object,
            offset,
            expected,
            replacement,
        } => buf.push_str(&format!(
            "    t{} = atomic_cmpxchg acq_rel/acquire {} +{} expected={} replacement={} : {}\n",
            out.into_raw(),
            value_name(*object),
            offset,
            value_name(*expected),
            value_name(*replacement),
            function.temps[*out].ty.dump()
        )),
        Instruction::FunctionAddress { out, symbol } => buf.push_str(&format!(
            "    t{} = function_address @{} : ptr\n",
            out.into_raw(),
            symbol
        )),
        Instruction::ForeignCallbackRegister {
            out,
            bridge,
            closure,
        } => buf.push_str(&format!(
            "    t{} = foreign_callback_register fcb{} {} : {}\n",
            out.into_raw(),
            bridge.into_raw(),
            value_name(*closure),
            function.temps[*out].ty.dump()
        )),
        Instruction::ForeignCallbackOperation(operation) => match *operation {
            ForeignCallbackOperation::Retain { out, callback } => buf.push_str(&format!(
                "    t{} = foreign_callback_{} {} : {}\n",
                out.into_raw(),
                "Retain",
                value_name(callback),
                function.temps[out].ty.dump()
            )),
            ForeignCallbackOperation::State { out, callback } => buf.push_str(&format!(
                "    t{} = foreign_callback_{} {} : {}\n",
                out.into_raw(),
                "State",
                value_name(callback),
                function.temps[out].ty.dump()
            )),
            ForeignCallbackOperation::Failure { out, callback } => buf.push_str(&format!(
                "    t{} = foreign_callback_{} {} : {}\n",
                out.into_raw(),
                "Failure",
                value_name(callback),
                function.temps[out].ty.dump()
            )),
            ForeignCallbackOperation::Release { callback } => buf.push_str(&format!(
                "    foreign_callback_Release {}\n",
                value_name(callback)
            )),
        },
        Instruction::IntToPtr { out, value } => buf.push_str(&format!(
            "    t{} = int_to_ptr {} : ptr\n",
            out.into_raw(),
            value_name(*value)
        )),
        Instruction::PtrToInt { out, value } => buf.push_str(&format!(
            "    t{} = ptr_to_int {} : i64\n",
            out.into_raw(),
            value_name(*value)
        )),
        Instruction::RawLoad {
            out,
            pointer,
            align,
        } => buf.push_str(&format!(
            "    t{} = raw_load {} align {} : {}\n",
            out.into_raw(),
            value_name(*pointer),
            align,
            function.temps[*out].ty.dump()
        )),
        Instruction::RawStore {
            pointer,
            value,
            align,
        } => buf.push_str(&format!(
            "    raw_store {} {} align {}\n",
            value_name(*pointer),
            value_name(*value),
            align
        )),
        Instruction::PtrOffset {
            out,
            pointer,
            bytes,
        } => buf.push_str(&format!(
            "    t{} = ptr_offset {} {} : ptr\n",
            out.into_raw(),
            value_name(*pointer),
            value_name(*bytes)
        )),
        Instruction::LocalAddress { out, local } => buf.push_str(&format!(
            "    t{} = local_address local{} : ptr\n",
            out.into_raw(),
            local.into_raw()
        )),
        Instruction::Store { local, value } => buf.push_str(&format!(
            "    store {} -> local{}\n",
            value_name(*value),
            local.into_raw()
        )),
        Instruction::Call { site } => {
            buf.push_str(&format!("    call {}\n", call_site_name(function, site)));
        }
        Instruction::ManagedPoll { site } => buf.push_str(&format!(
            "    poll managed-target{} sp{} live=[{}]\n",
            site.target.into_raw(),
            site.safepoint.get(),
            live_set_name(&site.live)
        )),
        Instruction::Invoke { site } => {
            buf.push_str(&format!(
                "    invoke {}\n",
                invoke_site_name(function, site)
            ));
        }
        Instruction::LandingPad { record, raw } => buf.push_str(&format!(
            "    (t{}, t{}) = landingpad : ({}, {})\n",
            record.into_raw(),
            raw.into_raw(),
            function.temps[*record].ty.dump(),
            function.temps[*raw].ty.dump()
        )),
        Instruction::CleanupPad { record, raw } => buf.push_str(&format!(
            "    (t{}, t{}) = cleanup_pad : ({}, {})\n",
            record.into_raw(),
            raw.into_raw(),
            function.temps[*record].ty.dump(),
            function.temps[*raw].ty.dump()
        )),
        Instruction::BeginCatch { out, raw } => buf.push_str(&format!(
            "    t{} = begin_catch {} : {}\n",
            out.into_raw(),
            value_name(*raw),
            function.temps[*out].ty.dump()
        )),
        Instruction::EndCatch => buf.push_str("    end_catch\n"),
        Instruction::Throw { exception } => {
            buf.push_str(&format!("    throw {}\n", value_name(*exception)))
        }
        Instruction::ArrayAlloc {
            out,
            elements,
            array_type,
            safepoint,
            live,
        } => {
            let elements: Vec<String> = elements.iter().map(|e| value_name(*e)).collect();
            buf.push_str(&format!(
                "    t{} = array_alloc array{} ({}) sp{} live {} : {}\n",
                out.into_raw(),
                array_type.into_raw(),
                elements.join(", "),
                safepoint.get(),
                live_set_name(live),
                function.temps[*out].ty.dump()
            ))
        }
        Instruction::ArrayLen {
            out,
            operand,
            array_type,
        } => buf.push_str(&format!(
            "    t{} = array_len array{} {} : {}\n",
            out.into_raw(),
            array_type.into_raw(),
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
        Instruction::ArrayGet {
            out,
            array,
            index,
            array_type,
        } => buf.push_str(&format!(
            "    t{} = array_get array{} {} {} : {}\n",
            out.into_raw(),
            array_type.into_raw(),
            value_name(*array),
            value_name(*index),
            function.temps[*out].ty.dump()
        )),
        Instruction::ArraySet {
            array,
            index,
            value,
            array_type,
        } => buf.push_str(&format!(
            "    array_set array{} {} {} {}\n",
            array_type.into_raw(),
            value_name(*array),
            value_name(*index),
            value_name(*value)
        )),
        Instruction::ArrayClone {
            out,
            operand,
            array_type,
            safepoint,
            live,
        } => buf.push_str(&format!(
            "    t{} = array_clone array{} {} sp{} live {} : {}\n",
            out.into_raw(),
            array_type.into_raw(),
            value_name(*operand),
            safepoint.get(),
            live_set_name(live),
            function.temps[*out].ty.dump()
        )),
        Instruction::EnumWrap {
            out,
            enum_id,
            variant,
            fields,
        } => {
            let fields: Vec<String> = fields.iter().map(|f| value_name(*f)).collect();
            buf.push_str(&format!(
                "    t{} = enum_wrap e{} v{} ({}) : {}\n",
                out.into_raw(),
                enum_id.into_raw(),
                variant,
                fields.join(", "),
                function.temps[*out].ty.dump()
            ))
        }
        Instruction::EnumTag {
            out,
            enum_id,
            operand,
        } => buf.push_str(&format!(
            "    t{} = enum_tag e{} {} : {}\n",
            out.into_raw(),
            enum_id.into_raw(),
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
        Instruction::EnumField {
            out,
            enum_id,
            variant,
            index,
            operand,
        } => buf.push_str(&format!(
            "    t{} = enum_field e{} v{} f{} {} : {}\n",
            out.into_raw(),
            enum_id.into_raw(),
            variant,
            index,
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
    }
}
