use super::*;

mod boxing;
mod initialization;
mod instruction;
mod metadata;
mod names;

use instruction::dump_instruction;
use names::*;

pub use initialization::dump_initialization_dependencies;

/// Indented text dump for golden tests (`scoopc build --emit=lir`).
pub fn dump(module: &Module) -> String {
    let mut out = String::from("Module\n");
    for (_, global) in module.globals.iter() {
        match &global.init {
            GlobalInit::ImportedStorage { definition, ty } => out.push_str(&format!(
                "  imported_global @{} : {} provider={} scan={}\n",
                global.symbol(),
                ty.dump(),
                definition.provider(),
                global.scan.dump()
            )),
            GlobalInit::StringConst { value, .. } => {
                out.push_str(&format!("  global @{} = {:?}\n", global.symbol(), value));
            }
            GlobalInit::CString { value, .. } => {
                out.push_str(&format!("  global @{} = c{:?}\n", global.symbol(), value));
            }
            GlobalInit::RawStorage {
                ty, thread_local, ..
            } => out.push_str(&format!(
                "  raw_{} @{} : {}\n",
                if *thread_local {
                    "thread_local"
                } else {
                    "global"
                },
                global.symbol(),
                ty.dump()
            )),
            GlobalInit::Storage { ty, .. } => out.push_str(&format!(
                "  {} @{} : {} scan={}\n",
                "global",
                global.symbol(),
                ty.dump(),
                global.scan.dump()
            )),
        }
    }
    for (id, unit) in module.initialization_units.iter() {
        let initializer =
            module.functions[unit.initializer.declaration().into_u32() as usize].symbol();
        let ensure = module.functions[unit.ensure.declaration().into_u32() as usize].symbol();
        let schedule = match unit.schedule {
            InitializationSchedule::EagerStartup => "",
            InitializationSchedule::LazyAccess => " lazy",
        };
        out.push_str(&format!(
            "  init{} {}{schedule} storage=@{} failure=@{} initializer=@{} ensure=@{} deps=[{}]\n",
            id.into_raw().into_u32(),
            unit.display_name,
            module.globals[unit.kind.storage()].symbol(),
            module.globals[unit.failure_root].symbol(),
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
        let (params, return_type, kind) = match &extern_.kind {
            ExternFunctionKind::C {
                bridge,
                signature,
                call_mode,
            } => (
                signature
                    .storage_params()
                    .iter()
                    .map(LirType::dump)
                    .collect::<Vec<_>>()
                    .join(", "),
                signature.storage_return_type().dump(),
                format!(
                    "c exact={} bridge=@{} gc-leaf nounwind{}",
                    signature.dump(),
                    bridge.symbol(),
                    if *call_mode == CAbiCallMode::GcLeaf {
                        " mode=gc-leaf"
                    } else {
                        ""
                    },
                ),
            ),
            ExternFunctionKind::Scoop {
                gc_effect,
                signature,
            } => (
                signature
                    .arguments()
                    .iter()
                    .map(abi_argument_name)
                    .collect::<Vec<_>>()
                    .join(", "),
                abi_return_name(signature.result()),
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
            return_type,
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
            match callback.bridge {
                StaticCallbackTarget::Local(bridge) => module.functions
                    [bridge.declaration().into_u32() as usize]
                    .symbol()
                    .to_string(),
                StaticCallbackTarget::External(bridge) => module.meta.external_callables[bridge]
                    .expected_symbol()
                    .symbol()
                    .to_string(),
            },
            callback.trampoline.entry().symbol(),
            params,
            callback.return_type.dump(),
        ));
    }
    for (id, family) in module.foreign_callback_families.iter() {
        out.push_str(&format!(
            "  foreign_callback_family fcf{} callback=struct{} state=enum{} failure=enum{}\n",
            id.into_raw(),
            family.callback.into_raw(),
            family.states.definition().into_raw(),
            family.failure_result.definition().into_raw(),
        ));
    }
    for (id, bridge) in module.foreign_callback_bridges.iter() {
        let family = &module.foreign_callback_families[bridge.family];
        let mode = if bridge.mode == family.modes.reusable() {
            "ForeignCallbackMode.Reusable".to_string()
        } else if bridge.mode == family.modes.one_shot() {
            "ForeignCallbackMode.OneShot".to_string()
        } else {
            format!(
                "enum{}.v{}",
                bridge.mode.definition().into_raw(),
                bridge.mode.index()
            )
        };
        let params = bridge
            .params
            .iter()
            .map(CType::dump)
            .collect::<Vec<_>>()
            .join(",");
        out.push_str(&format!(
            "  foreign_callback_bridge fcb{} family=fcf{} @{} -> @{} signature=@{} context={} mode={} c=({})->{}\n",
            id.into_raw(),
            bridge.family.into_raw(),
            module.functions[bridge.adapter.declaration().into_u32() as usize].symbol(),
            bridge.trampoline.entry().symbol(),
            bridge.trampoline.signature_descriptor_symbol(),
            bridge.context_index,
            mode,
            params,
            bridge.return_type.dump(),
        ));
    }
    for function in module.callable_bodies() {
        let params = function
            .signature
            .arguments()
            .iter()
            .map(abi_argument_name)
            .collect::<Vec<_>>();
        out.push_str(&format!(
            "  fun @{}({}) -> {}{}\n",
            function.symbol(),
            params.join(", "),
            abi_return_name(function.signature.result()),
            if function.gc_effect == GcEffect::NoGc {
                " <no-gc>"
            } else {
                ""
            }
        ));
        for (id, local) in function.locals.iter() {
            let storage = match local.storage() {
                LocalStorage::LogicalZst(value) => format!(" <logical-zst {}>", value.exact()),
                LocalStorage::AddressableZst(place) => format!(
                    " <zst-token {} align={} lifetime=function>",
                    place.value().exact(),
                    place.value().representation().layout().alignment(),
                ),
                LocalStorage::NonZero(_) => String::new(),
            };
            out.push_str(&format!(
                "    local %{} {}: {}{}\n",
                id.into_raw(),
                local.name,
                local.ty().dump(),
                storage,
            ));
        }
        for (block_id, block) in function.blocks.iter() {
            let _ = block_id;
            out.push_str(&format!("  block {}\n", block.name));
            for instruction in &block.instructions {
                dump_instruction(module, function, instruction, &mut out);
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
    metadata::dump_metadata(module, &mut out);
    match module.output {
        LirOutput::Library => out.push_str("  output library\n"),
        LirOutput::Executable { entry } => {
            let entry = &module.functions[entry.declaration().into_u32() as usize];
            out.push_str(&format!("  output executable @{}\n", entry.symbol()));
        }
    }
    out
}
