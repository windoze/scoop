use super::*;

mod instruction;
mod names;

use instruction::dump_instruction;
use names::*;

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
    for (id, unit) in module.initialization_units.iter() {
        let initializer =
            &module.functions[unit.initializer.declaration().into_u32() as usize].symbol;
        let ensure = &module.functions[unit.ensure.declaration().into_u32() as usize].symbol;
        let schedule = match unit.schedule {
            InitializationSchedule::EagerStartup => "",
            InitializationSchedule::LazyAccess => " lazy",
        };
        out.push_str(&format!(
            "  init{} {}{schedule} storage=@{} failure=@{} initializer=@{} ensure=@{} deps=[{}]\n",
            id.into_raw().into_u32(),
            unit.stable_key,
            module.globals[unit.kind.storage()].symbol,
            module.globals[unit.failure_root].symbol,
            initializer,
            ensure,
            unit.dependencies
                .iter()
                .map(|dependency| dependency.into_raw().into_u32().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
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
            "  native_global{} {} @{} : {} c={} {}{}\n",
            id.into_raw(),
            global.source_name,
            global.native_symbol,
            global.storage_type().dump(),
            global.c_type.dump(),
            access,
            if global.thread_local {
                " thread_local"
            } else {
                ""
            }
        ));
    }
    for (id, definition) in module.structs.iter() {
        let Some(fields) = definition.c_fields() else {
            continue;
        };
        let fields = fields
            .iter()
            .map(|field| {
                format!(
                    "{}@{}/{}",
                    field.ty.dump(),
                    field.layout.offset,
                    field.layout.access_align
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        out.push_str(&format!(
            "  c-struct struct{} {} fields=[{}]\n",
            id.into_raw(),
            definition.name,
            fields
        ));
    }
    for (_, def) in module.enums.iter() {
        match &def.repr {
            EnumRepr::Niche {
                kind,
                payload_variant,
            } => out.push_str(&format!(
                "  enum {} niche(kind={},payload_variant={})\n",
                def.name,
                kind.pointer_kind().dump(),
                payload_variant
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
        let (parameter_types, return_type, kind) = match &extern_.kind {
            ExternFunctionKind::C {
                bridge_symbol,
                signature,
            } => (
                signature.storage_params(),
                signature.storage_return_type(),
                format!(
                    "c exact={} bridge=@{bridge_symbol} gc-leaf nounwind",
                    signature.dump()
                ),
            ),
            ExternFunctionKind::Scoop {
                gc_effect,
                signature,
            } => (
                signature.params.clone(),
                signature.storage_return_type(),
                format!(
                    "scoop {} nounwind",
                    if *gc_effect == GcEffect::NoGc {
                        "gc-leaf"
                    } else {
                        "managed"
                    }
                ),
            ),
        };
        let params = parameter_types
            .iter()
            .map(LirType::dump)
            .collect::<Vec<_>>()
            .join(", ");
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
            return_type.dump(),
            kind,
            library
        ));
    }
    for (id, callback) in module.callback_bridges.iter() {
        let params = callback
            .params
            .iter()
            .map(CType::dump)
            .collect::<Vec<_>>()
            .join(",");
        out.push_str(&format!(
            "  callback cb{} {} @{} -> @{} c=({})->{}\n",
            id.into_raw(),
            callback.source_name,
            callback.bridge_symbol,
            callback.trampoline_symbol,
            params,
            callback.return_type.dump(),
        ));
    }
    for (id, family) in module.foreign_callback_families.iter() {
        out.push_str(&format!(
            "  foreign_callback_family fcf{} callback=struct{} state=enum{} failure=enum{}\n",
            id.into_raw(),
            family.callback.into_raw(),
            family.state.into_raw(),
            family.failure.into_raw(),
        ));
    }
    for (id, bridge) in module.foreign_callback_bridges.iter() {
        let params = bridge
            .params
            .iter()
            .map(CType::dump)
            .collect::<Vec<_>>()
            .join(",");
        out.push_str(&format!(
            "  foreign_callback_bridge fcb{} family=fcf{} @{} -> @{} signature=@{} context={} mode={:?} c=({})->{}\n",
            id.into_raw(),
            bridge.family.into_raw(),
            bridge.adapter_symbol,
            bridge.trampoline_symbol,
            bridge.signature_symbol,
            bridge.context_index,
            bridge.mode,
            params,
            bridge.return_type.dump(),
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
    // String remains first, followed by every exact source integer layout in
    // arena order, Boolean, and ordinary layouts.
    let string_layout = module.meta.well_known_layouts.string;
    for (_layout_id, layout) in
        std::iter::once((string_layout, &module.meta.layouts[string_layout]))
            .chain(module.meta.layouts.iter().filter(|(_, layout)| {
                matches!(
                    layout.kind,
                    LayoutKind::Intrinsic(IntrinsicTypeRepresentation::Integer(_))
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
                        IntrinsicTypeRepresentation::Integer(_)
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
                IntrinsicTypeRepresentation::Integer(_)
                | IntrinsicTypeRepresentation::Boolean
                | IntrinsicTypeRepresentation::String,
            ) => out.push_str(&format!(
                "  layout {} size={} align={} refs=[]\n",
                layout.name, layout.size, layout.align
            )),
            LayoutKind::Intrinsic(IntrinsicTypeRepresentation::Ptr { pointee }) => {
                let pointee = match pointee {
                    LirDataPointee::OpaqueVoid => "void".to_string(),
                    LirDataPointee::Value(ty) => ty.dump(),
                };
                out.push_str(&format!(
                    "  layout {} size={} align={} intrinsic=ptr<{}> refs=[]\n",
                    layout.name, layout.size, layout.align, pointee
                ));
            }
            LayoutKind::Intrinsic(IntrinsicTypeRepresentation::FunPtr { signature }) => {
                let params = signature
                    .params
                    .iter()
                    .map(LirType::dump)
                    .collect::<Vec<_>>()
                    .join(",");
                let result = match &signature.return_type {
                    LirReturnType::Void => "void".to_string(),
                    LirReturnType::Value(ty) => ty.dump(),
                };
                out.push_str(&format!(
                    "  layout {} size={} align={} intrinsic=funptr<({})->{}> refs=[]\n",
                    layout.name, layout.size, layout.align, params, result
                ));
            }
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
                layout.name,
                c_layout.aligned.bytes().unwrap_or(0),
                c_layout.packed.bytes().unwrap_or(0),
                fields,
                layout.interior_mutable
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
