use super::super::*;
use super::{block_number, dump_statements, dump_terminator, type_name};

pub fn dump(module: &Module) -> String {
    let mut out = String::from("Module\n");
    for (id, global) in module.globals.iter() {
        let storage = match &global.storage {
            GlobalStorage::Managed { .. } => "managed".to_string(),
            GlobalStorage::Local {
                thread_local: false,
                ..
            } => "global".to_string(),
            GlobalStorage::Local {
                thread_local: true, ..
            } => "thread_local".to_string(),
            GlobalStorage::Extern {
                native_symbol,
                thread_local,
                ..
            } => format!(
                "extern {native_symbol}{}",
                if *thread_local { " thread_local" } else { "" }
            ),
        };
        out.push_str(&format!(
            "  global{} @{} {}: {} {storage}\n",
            id.into_raw().into_u32(),
            global.symbol,
            global.name,
            type_name(module, &global.ty)
        ));
    }
    for (id, unit) in module.initialization_units.iter() {
        let InitializationUnitKind::EagerTopLevel { storage } = unit.kind;
        out.push_str(&format!(
            "  init{} {} eager global{} initializer={} ensure={} failure={} deps=[{}]\n",
            id.into_raw().into_u32(),
            unit.stable_key,
            storage.into_raw().into_u32(),
            module.functions[unit.initializer].symbol,
            module.functions[unit.ensure].symbol,
            module.initialization_failure_roots[unit.failure_root]
                .global
                .into_raw()
                .into_u32(),
            unit.dependencies
                .iter()
                .map(|dependency| dependency.into_raw().into_u32().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    for (id, extern_) in module.extern_functions.iter() {
        let abi = match extern_.abi {
            ExternAbi::C => "c",
            ExternAbi::Scoop => "scoop",
        };
        let params = extern_
            .params
            .iter()
            .map(|ty| type_name(module, ty))
            .collect::<Vec<_>>()
            .join(", ");
        let library = if extern_.library.is_empty() {
            String::new()
        } else {
            format!(" lib={}", extern_.library)
        };
        out.push_str(&format!(
            "  extern ef{} {} @{}({}) -> {} <abi={abi}{}{}>\n",
            id.into_raw().into_u32(),
            extern_.source_name,
            extern_.native_symbol,
            params,
            type_name(module, &extern_.return_type),
            if extern_.gc_effect == GcEffect::NoGc {
                " no-gc"
            } else {
                " managed"
            },
            library
        ));
    }
    for (id, callback) in module.callback_bridges.iter() {
        out.push_str(&format!(
            "  callback cb{} @{} -> @{} function_type{}\n",
            id.into_raw().into_u32(),
            module.functions[callback.source].symbol,
            module.functions[callback.bridge_function].symbol,
            callback.signature.into_raw().into_u32()
        ));
    }
    for (_, def) in module.structs.iter() {
        match &def.representation {
            StructRepresentation::Declared {
                c_layout,
                interior_mutable,
                fields,
            } => {
                let fields: Vec<String> = fields
                    .iter()
                    .map(|f| format!("{}: {}", f.name, type_name(module, &f.ty)))
                    .collect();
                let mut attributes = Vec::new();
                if let Some(layout) = c_layout {
                    attributes.push(format!(
                        "c-layout aligned={} packed={}",
                        layout.aligned, layout.packed
                    ));
                }
                if *interior_mutable {
                    attributes.push("interior-mutable".to_string());
                }
                let attributes = if attributes.is_empty() {
                    String::new()
                } else {
                    format!(" <{}>", attributes.join(" "))
                };
                out.push_str(&format!(
                    "  struct {} ({}){}\n",
                    def.name,
                    fields.join(", "),
                    attributes
                ));
            }
            StructRepresentation::Intrinsic(_) => {}
        }
    }
    for (_, def) in module.enums.iter() {
        out.push_str(&format!("  enum {}\n", def.name));
        for variant in &def.variants {
            let fields: Vec<String> = variant
                .fields
                .iter()
                .map(|f| format!("{}: {}", f.name, type_name(module, &f.ty)))
                .collect();
            out.push_str(&format!("    {}({})\n", variant.name, fields.join(", ")));
        }
    }
    for (_, def) in module.classes.iter() {
        if matches!(def.representation, ClassRepresentation::Intrinsic(_)) {
            continue;
        }
        out.push_str(&format!(
            "  class {} vtable={} itables={}\n",
            def.name,
            def.vtable.len(),
            def.itables.len()
        ));
    }
    for (_, def) in module.interfaces.iter() {
        out.push_str(&format!("  interface {}\n", def.name));
    }
    for (id, def) in module.closure_classes.iter() {
        let invoke = module.closure_invoke_functions[def.invoke].function;
        out.push_str(&format!(
            "  closure cc{} {} type=function_type{} invoke=@{} captures={}\n",
            id.into_raw().into_u32(),
            def.name,
            def.function_type.into_raw().into_u32(),
            module.functions[invoke].symbol,
            def.captures.len()
        ));
        for bridge in &def.bridges {
            out.push_str(&format!(
                "    bridge function_type{} -> @{}\n",
                bridge.target.into_raw().into_u32(),
                module.functions[bridge.function].symbol
            ));
        }
    }
    for (id, adapter) in module.meta.closure_adapters.iter() {
        out.push_str(&format!(
            "  adapter ca{} class=cc{} source=function_type{} target=function_type{}\n",
            id.into_raw().into_u32(),
            adapter.class.into_raw().into_u32(),
            adapter.source.into_raw().into_u32(),
            adapter.target.into_raw().into_u32()
        ));
    }
    for (id, adapter) in module.meta.dynamic_closure_adapters.iter() {
        out.push_str(&format!(
            "  dynamic_adapter da{} class=cc{} target=function_type{}\n",
            id.into_raw().into_u32(),
            adapter.class.into_raw().into_u32(),
            adapter.target.into_raw().into_u32()
        ));
    }
    for &id in &module.top_level {
        let function = &module.functions[id];
        let params: Vec<String> = function
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, type_name(module, &p.ty)))
            .collect();
        out.push_str(&format!(
            "  fun {} @{}({}) -> {}{}\n",
            function.name,
            function.symbol,
            params.join(", "),
            type_name(module, &function.return_ty),
            if function.gc_effect == GcEffect::NoGc {
                " <no-gc>"
            } else {
                ""
            }
        ));
        for (block_id, block) in function.body.blocks.iter() {
            let unwind = block
                .unwind
                .map(|target| format!(" unwind bb{}", block_number(target)))
                .unwrap_or_default();
            out.push_str(&format!(
                "    bb{} {}{}\n",
                block_number(block_id),
                block.name,
                unwind
            ));
            dump_statements(
                module,
                &function.body.locals,
                &block.statements,
                3,
                &mut out,
            );
            dump_terminator(
                module,
                &function.body.locals,
                &block.terminator,
                3,
                &mut out,
            );
        }
    }
    for (id, step) in module.meta.coroutine_steps.iter() {
        out.push_str(&format!(
            "  coroutine_step cs{} {} result={}\n",
            id.into_raw().into_u32(),
            module.enums[step.enum_id].name,
            type_name(module, &step.result)
        ));
    }
    for (id, slot) in module.meta.coroutine_slots.iter() {
        out.push_str(&format!(
            "  coroutine_slot cl{} {} value={}\n",
            id.into_raw().into_u32(),
            module.enums[slot.enum_id].name,
            type_name(module, &slot.value)
        ));
    }
    for (id, frame) in module.meta.coroutine_frames.iter() {
        out.push_str(&format!(
            "  coroutine_frame cr{} {} owner=cf{}\n",
            id.into_raw().into_u32(),
            module.classes[frame.class].name,
            frame.owner.into_raw().into_u32()
        ));
    }
    for (id, point) in module.meta.coroutine_resume_points.iter() {
        out.push_str(&format!(
            "  coroutine_resume cp{} state={} result={} frame=cr{} adapter={} resume=@{} failure=@{}\n",
            id.into_raw().into_u32(),
            point.state,
            type_name(module, &point.result),
            point.frame.into_raw().into_u32(),
            module.classes[point.adapter].name,
            module.functions[point.resume].symbol,
            module.functions[point.resume_with_exception].symbol
        ));
    }
    for (id, coroutine) in module.meta.coroutine_functions.iter() {
        let lowering = match &coroutine.lowering {
            CoroutineLowering::Immediate => " immediate".to_string(),
            CoroutineLowering::StateMachine {
                frame,
                driver,
                resume_points,
            } => format!(
                " frame=cr{} driver=@{} resumes=[{}]",
                frame.into_raw().into_u32(),
                module.functions[*driver].symbol,
                resume_points
                    .iter()
                    .map(|id| format!("cp{}", id.into_raw().into_u32()))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        };
        out.push_str(&format!(
            "  coroutine_fn cf{} @{} source_return={} step=cs{}{}\n",
            id.into_raw().into_u32(),
            module.functions[coroutine.function].symbol,
            type_name(module, &coroutine.source_return),
            coroutine.step.into_raw().into_u32(),
            lowering
        ));
    }
    for (_, instance) in module.meta.instances.iter() {
        out.push_str(&format!(
            "  instance @{} <- {}\n",
            instance.symbol,
            module
                .meta
                .monomorphized_source_display_name(&instance.source)
        ));
    }
    for (_, string) in module.strings.iter() {
        out.push_str(&format!("  str @{} {:?}\n", string.symbol, string.value));
    }
    out.push_str(&format!("  entry @{ENTRY_SYMBOL}\n"));
    out
}
