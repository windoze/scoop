use super::super::*;
use super::{block_number, dump_statements, dump_terminator, function_ref, string_ref, type_name};

pub fn dump(module: &Module) -> String {
    let mut out = String::from("Module\n");
    for (id, global) in module.globals.iter() {
        let storage = match &global.storage {
            GlobalStorage::Managed { initial_state } => format!(
                "managed initial={}",
                static_initial_state_name(module, initial_state)
            ),
            GlobalStorage::Local {
                thread_local: false,
                initial_state,
            } => format!(
                "global initial={}",
                static_initial_state_name(module, initial_state)
            ),
            GlobalStorage::Local {
                thread_local: true,
                initial_state,
            } => format!(
                "thread_local initial={}",
                static_initial_state_name(module, initial_state)
            ),
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
            "  global{} {}: {} {storage}\n",
            id.into_raw().into_u32(),
            global.name,
            type_name(module, &global.ty)
        ));
    }
    for (id, unit) in module.initialization_units.iter() {
        let storage = match unit.kind {
            InitializationUnitKind::EagerTopLevel { storage } => storage,
            InitializationUnitKind::LazySingleton { published_root, .. } => {
                module.singleton_published_roots[published_root].global
            }
        };
        let kind = match unit.kind {
            InitializationUnitKind::EagerTopLevel { .. } => "",
            InitializationUnitKind::LazySingleton { .. } => "singleton ",
        };
        let schedule = match unit.schedule {
            InitializationSchedule::EagerStartup => "eager",
            InitializationSchedule::LazyAccess => "lazy",
        };
        out.push_str(&format!(
            "  init{} {} {kind}{schedule} global{} initializer={} ensure={} failure={} deps=[{}]\n",
            id.into_raw().into_u32(),
            unit.display_name,
            storage.into_raw().into_u32(),
            function_ref(unit.initializer),
            function_ref(unit.ensure),
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
            "  callback cb{} {} -> {} function_type{} id={}\n",
            id.into_raw().into_u32(),
            function_ref(callback.source),
            function_ref(callback.bridge_function),
            callback.signature.into_raw().into_u32(),
            callback.identity().callable_record().id(),
        ));
    }
    for (id, family) in module.foreign_callback_families.iter() {
        out.push_str(&format!(
            "  foreign_callback_family fcf{} callback={} state={} failure={}\n",
            id.into_raw().into_u32(),
            type_name(module, &Type::Struct(family.callback)),
            type_name(
                module,
                &Type::Enum(
                    family.states.enum_id(),
                    module.enums[family.states.enum_id()].type_arguments.clone(),
                ),
            ),
            type_name(
                module,
                &Type::Enum(
                    family.failure_result.enum_id(),
                    module.enums[family.failure_result.enum_id()]
                        .type_arguments
                        .clone(),
                ),
            ),
        ));
    }
    for (id, bridge) in module.foreign_callback_bridges.iter() {
        let adapter = &module.foreign_callback_adapters[bridge.adapter];
        let mode_enum = &module.enums[bridge.mode.enum_id()];
        let mode = &mode_enum.variants[bridge.mode.variant_index() as usize].name;
        out.push_str(&format!(
            "  foreign_callback_bridge fcb{} family=fcf{} native=function_type{} managed=function_type{} context={} mode={}.{} adapter={}\n",
            id.into_raw().into_u32(),
            bridge.family.into_raw().into_u32(),
            bridge.native_signature.into_raw().into_u32(),
            adapter.managed_signature.into_raw().into_u32(),
            bridge.context_index,
            mode_enum.name,
            mode,
            function_ref(adapter.function),
        ));
    }
    for (id, def) in module.structs.iter() {
        match &def.representation {
            StructRepresentation::Declared {
                c_layout,
                c_abi,
                interior_mutable,
                fields,
            } => {
                let fields: Vec<String> = fields
                    .iter()
                    .map(|f| format!("{}: {}", f.name, type_name(module, &f.ty)))
                    .collect();
                let mut attributes = Vec::new();
                if let StructCAbi::UInt64Field { .. } = c_abi {
                    attributes.push("c-abi uint64-field".to_owned());
                }
                if let Some(layout) = c_layout {
                    attributes.push(format!(
                        "c-layout aligned={} packed={}",
                        layout.aligned.name(),
                        layout.packed.name()
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
                    type_name(module, &Type::Struct(id)),
                    fields.join(", "),
                    attributes
                ));
            }
            StructRepresentation::Intrinsic(_) => {}
        }
    }
    for (id, def) in module.enums.iter() {
        out.push_str(&format!(
            "  enum {}\n",
            type_name(module, &Type::Enum(id, def.type_arguments.clone()))
        ));
        for variant in &def.variants {
            let fields: Vec<String> = variant
                .fields
                .iter()
                .map(|f| format!("{}: {}", f.name, type_name(module, &f.ty)))
                .collect();
            out.push_str(&format!("    {}({})\n", variant.name, fields.join(", ")));
        }
    }
    for (id, def) in module.classes.iter() {
        if matches!(def.representation, ClassRepresentation::Intrinsic(_)) {
            continue;
        }
        out.push_str(&format!(
            "  class {} vtable={} itables={}\n",
            type_name(module, &Type::Class(id)),
            def.vtable.len(),
            def.itables.len()
        ));
    }
    for (id, _) in module.interfaces.iter() {
        out.push_str(&format!(
            "  interface {}\n",
            type_name(module, &Type::Interface(id))
        ));
    }
    for (id, def) in module.closure_classes.iter() {
        let invoke = module.closure_invoke_functions[def.invoke].function;
        out.push_str(&format!(
            "  closure cc{} {} type=function_type{} invoke={} captures={}\n",
            id.into_raw().into_u32(),
            def.name,
            def.function_type.into_raw().into_u32(),
            function_ref(invoke),
            def.captures.len()
        ));
        for bridge in &def.bridges {
            let generated = module
                .meta
                .function_bridges
                .iter()
                .find(|generated| {
                    generated.class() == id
                        && generated.target() == bridge.target
                        && generated.function() == bridge.function
                })
                .map(|generated| format!(" id={}", generated.identity().callable_record().id()))
                .unwrap_or_default();
            out.push_str(&format!(
                "    bridge function_type{} -> {}{}\n",
                bridge.target.into_raw().into_u32(),
                function_ref(bridge.function),
                generated,
            ));
        }
    }
    for (index, identity) in module.meta.generated_exact_types.iter().enumerate() {
        let location = match identity.location() {
            GeneratedExactTypeLocation::Closure(class) => {
                format!("closure{}", class.into_raw().into_u32())
            }
            GeneratedExactTypeLocation::Class(class) => {
                format!("class{}", class.into_raw().into_u32())
            }
            GeneratedExactTypeLocation::Enum(enumeration) => {
                format!("enum{}", enumeration.into_raw().into_u32())
            }
        };
        out.push_str(&format!(
            "  generated_exact_type get{index} location={location} nominal_id={} exact_id={}\n",
            identity.nominal_record().id(),
            identity.exact_record().id(),
        ));
    }
    for (index, identity) in module.meta.generated_callables.iter().enumerate() {
        out.push_str(&format!(
            "  generated_callable gc{index} function={} id={}\n",
            function_ref(identity.function()),
            identity.identity_record().id(),
        ));
    }
    for (id, adapter) in module.meta.closure_adapters.iter() {
        out.push_str(&format!(
            "  adapter ca{} class=cc{} source=function_type{} target=function_type{}\n",
            id.into_raw().into_u32(),
            adapter.class().into_raw().into_u32(),
            adapter.source().into_raw().into_u32(),
            adapter.target().into_raw().into_u32()
        ));
    }
    for (id, adapter) in module.meta.dynamic_closure_adapters.iter() {
        out.push_str(&format!(
            "  dynamic_adapter da{} class=cc{} target=function_type{}\n",
            id.into_raw().into_u32(),
            adapter.class().into_raw().into_u32(),
            adapter.target().into_raw().into_u32()
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
            "  fun {} {}({}) -> {}{}\n",
            function.name,
            function_ref(id),
            params.join(", "),
            type_name(module, &function.return_ty),
            if function.gc_effect == GcEffect::NoGc {
                " <no-gc>"
            } else {
                ""
            }
        ));
        for (block_id, block) in function.body.blocks.iter() {
            let loop_header_poll = if function
                .body
                .loop_header_polls
                .iter()
                .any(|target| target.header() == block_id)
            {
                " <loop-header-poll>"
            } else {
                ""
            };
            let unwind = block
                .unwind
                .map(|target| format!(" unwind bb{}", block_number(target)))
                .unwrap_or_default();
            out.push_str(&format!(
                "    bb{} {}{}{}\n",
                block_number(block_id),
                block.name,
                loop_header_poll,
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
            type_name(
                module,
                &Type::Enum(
                    step.enum_id(),
                    module.enums[step.enum_id()].type_arguments.clone(),
                ),
            ),
            type_name(module, step.result())
        ));
    }
    for (id, slot) in module.meta.coroutine_slots.iter() {
        out.push_str(&format!(
            "  coroutine_slot cl{} {} value={}\n",
            id.into_raw().into_u32(),
            type_name(
                module,
                &Type::Enum(
                    slot.enum_id(),
                    module.enums[slot.enum_id()].type_arguments.clone(),
                ),
            ),
            type_name(module, slot.value())
        ));
    }
    for (index, shell) in module.meta.continuation_shells.iter().enumerate() {
        out.push_str(&format!(
            "  continuation_shell ch{index} result={} success=fn{} failure=fn{}\n",
            type_name(module, shell.result()),
            shell.success().into_raw().into_u32(),
            shell.failure().into_raw().into_u32(),
        ));
    }
    for (index, start) in module.meta.coroutine_starts.iter().enumerate() {
        out.push_str(&format!(
            "  coroutine_start ct{index} result={} function=fn{}\n",
            type_name(module, start.result()),
            start.function().into_raw().into_u32(),
        ));
    }
    for (id, value) in module.meta.coroutine_saved_values.iter() {
        out.push_str(&format!(
            "  coroutine_saved cv{} class={} field={} slot=cl{} value={}\n",
            id.into_raw().into_u32(),
            type_name(module, &Type::Class(value.field().class())),
            value.field().field_index(),
            value.slot().into_raw().into_u32(),
            type_name(module, value.value())
        ));
    }
    for (id, value) in module.meta.coroutine_failure_values.iter() {
        out.push_str(&format!(
            "  coroutine_failure cx{} class={} field={} slot=cl{} throwable={}\n",
            id.into_raw().into_u32(),
            type_name(module, &Type::Class(value.field().class())),
            value.field().field_index(),
            value.slot().into_raw().into_u32(),
            type_name(module, &Type::Class(value.throwable()))
        ));
    }
    for (id, frame) in module.meta.coroutine_frames.iter() {
        out.push_str(&format!(
            "  coroutine_frame cr{} {} id={} owner=cf{} state=field{} completion=field{} saved=[{}] failure=cx{}\n",
            id.into_raw().into_u32(),
            type_name(module, &Type::Class(frame.class())),
            frame.identity().generated_type_record().id(),
            frame.owner().into_raw().into_u32(),
            frame.state().field_index(),
            frame.completion().field_index(),
            frame
                .saved_values()
                .iter()
                .map(|value| format!("cv{}", value.into_raw().into_u32()))
                .collect::<Vec<_>>()
                .join(","),
            frame.failure().into_raw().into_u32()
        ));
    }
    for (id, point) in module.meta.coroutine_resume_points.iter() {
        let parents = point
            .parents()
            .iter()
            .map(coroutine_transfer_name)
            .collect::<Vec<_>>()
            .join(" -> ");
        let parents = if parents.is_empty() {
            String::new()
        } else {
            format!("{parents} -> ")
        };
        let identity = point.identity();
        out.push_str(&format!(
            "  coroutine_resume cp{} site={} result={} frame=cr{} adapter={} environment_id={} resume={} success_id={} failure={} failure_id={}\n",
            id.into_raw().into_u32(),
            point.site().get(),
            type_name(module, point.result()),
            point.frame().into_raw().into_u32(),
            type_name(module, &Type::Class(point.adapter())),
            identity.generated_type_record().id(),
            function_ref(point.resume()),
            identity.success().callable_record().id(),
            function_ref(point.resume_with_exception()),
            identity.failure().callable_record().id()
        ));
        out.push_str(&format!(
            "    success {} -> {}Fallthrough(bb{})\n",
            point.success_state(),
            parents,
            block_number(point.success().post().block())
        ));
        out.push_str(&format!(
            "      entry=bb{}\n",
            block_number(point.success().entry().block())
        ));
        let failure = point.failure();
        let unwind = failure
            .unwind()
            .map(|target| format!("bb{}", block_number(target.block())))
            .unwrap_or_else(|| "propagate".to_string());
        out.push_str(&format!(
            "    failure {} -> {}ManagedThrow(cx{}, unwind={})\n",
            point.failure_state(),
            parents,
            failure.exception().into_raw().into_u32(),
            unwind
        ));
        out.push_str(&format!(
            "      entry=bb{}\n",
            block_number(failure.entry().block())
        ));
    }
    for (id, coroutine) in module.meta.coroutine_functions.iter() {
        let lowering = match &coroutine.lowering {
            CoroutineLowering::Immediate => " immediate".to_string(),
            CoroutineLowering::StateMachine {
                frame,
                driver,
                driver_identity,
                resume_points,
            } => format!(
                " frame=cr{} driver={} id={} resumes=[{}]",
                frame.into_raw().into_u32(),
                function_ref(*driver),
                driver_identity.callable_record().id(),
                resume_points
                    .iter()
                    .map(|id| format!("cp{}", id.into_raw().into_u32()))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        };
        out.push_str(&format!(
            "  coroutine_fn cf{} {} source_return={} step=cs{}{}\n",
            id.into_raw().into_u32(),
            function_ref(coroutine.function),
            type_name(module, &coroutine.source_return),
            coroutine.step.into_raw().into_u32(),
            lowering
        ));
    }
    for (id, instance) in module.meta.instances.iter() {
        out.push_str(&format!(
            "  instance mi{} function={} <- {}\n",
            id.into_raw().into_u32(),
            function_ref(instance.function),
            instance.display_name
        ));
    }
    for (id, string) in module.strings.iter() {
        out.push_str(&format!("  str {} {:?}\n", string_ref(id), string.value));
    }
    match module.output {
        MirOutput::Library => out.push_str("  output library\n"),
        MirOutput::Executable { entry } => {
            out.push_str(&format!("  output executable {}\n", function_ref(entry)));
        }
    }
    out
}

fn coroutine_transfer_name(transfer: &CoroutinePendingTransfer) -> String {
    match transfer {
        CoroutinePendingTransfer::Fallthrough(target) => {
            format!("Fallthrough(bb{})", block_number(target.block()))
        }
        CoroutinePendingTransfer::Return(CoroutineReturnTransfer::Unit) => {
            "Return(Unit)".to_string()
        }
        CoroutinePendingTransfer::Return(CoroutineReturnTransfer::Saved(value)) => {
            format!("Return(cv{})", value.into_raw().into_u32())
        }
        CoroutinePendingTransfer::Break(target) => {
            format!("Break(bb{})", block_number(target.block()))
        }
        CoroutinePendingTransfer::Continue(target) => {
            format!("Continue(bb{})", block_number(target.block()))
        }
        CoroutinePendingTransfer::ManagedThrow(throw_) => {
            let unwind = throw_
                .unwind()
                .map(|target| format!("bb{}", block_number(target.block())))
                .unwrap_or_else(|| "propagate".to_string());
            format!(
                "ManagedThrow(cv{}, unwind={unwind})",
                throw_.exception().into_raw().into_u32()
            )
        }
    }
}

fn static_initial_state_name(module: &Module, state: &MirStaticInitialState) -> String {
    match state {
        MirStaticInitialState::ZeroedForRuntimeUnit => "zeroed-for-runtime-unit".to_string(),
        MirStaticInitialState::EncodedStaticValue { payload } => {
            format!("encoded({})", constant_image_name(module, payload))
        }
    }
}

fn constant_image_name(module: &Module, image: &MirConstantImage) -> String {
    match image {
        MirConstantImage::Integer(value) => {
            format!("{}:0x{:x}", value.kind().canonical_name(), value.raw_bits())
        }
        MirConstantImage::Boolean(value) => value.to_string(),
        MirConstantImage::String(id) => string_ref(*id),
        MirConstantImage::PointerNull(MirPointerNull::Data) => "null<data>".to_string(),
        MirConstantImage::PointerNull(MirPointerNull::Code) => "null<code>".to_string(),
        MirConstantImage::EnumUnit { variant } => {
            format!(
                "{}::v{}",
                type_name(
                    module,
                    &Type::Enum(
                        variant.enum_id(),
                        module.enums[variant.enum_id()].type_arguments.clone(),
                    ),
                ),
                variant.variant_index()
            )
        }
        MirConstantImage::Struct { struct_id, fields } => format!(
            "{}{{{}}}",
            type_name(module, &Type::Struct(*struct_id)),
            fields
                .iter()
                .map(|field| constant_image_name(module, field))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}
