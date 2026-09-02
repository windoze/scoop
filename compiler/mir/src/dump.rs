use super::*;

/// Indented text dump for golden tests (`scoopc build --emit=mir`).
pub fn dump(module: &Module) -> String {
    let mut out = String::from("Module\n");
    for (id, global) in module.globals.iter() {
        let storage = match &global.storage {
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

/// Render a type for dumps.
pub fn type_name(module: &Module, ty: &Type) -> String {
    match ty {
        Type::Unit => "Unit".to_string(),
        Type::Int => "Int".to_string(),
        Type::UInt => "UInt".to_string(),
        Type::Boolean => "Boolean".to_string(),
        Type::String => "String".to_string(),
        Type::Struct(id) => module.structs[*id].name.clone(),
        Type::Class(id) => match &module.classes[*id].representation {
            ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::Array { element }) => {
                format!("Array<{}>", type_name(module, element))
            }
            ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::MutableArray {
                element,
            }) => format!("MutableArray<{}>", type_name(module, element)),
            ClassRepresentation::Declared { .. }
            | ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::String) => {
                module.classes[*id].name.clone()
            }
            ClassRepresentation::Intrinsic(_) => {
                unreachable!("the intrinsic registry fixes declaration targets")
            }
        },
        Type::Interface(id) => module.interfaces[*id].name.clone(),
        Type::Any => "Any".to_string(),
        Type::Tuple(elements) => {
            let inner: Vec<String> = elements.iter().map(|t| type_name(module, t)).collect();
            format!("({})", inner.join(", "))
        }
        Type::Function(id) => {
            let function = &module.function_types[*id];
            let parameters: Vec<String> = function
                .parameter_types
                .iter()
                .map(|ty| type_name(module, ty))
                .collect();
            let suspend = if function.is_suspend { "suspend " } else { "" };
            format!(
                "{suspend}({}) -> {}",
                parameters.join(", "),
                type_name(module, &function.return_type)
            )
        }
        Type::Ptr(inner) => format!("Ptr<{}>", type_name(module, inner)),
        Type::FunPtr(id) => {
            let function = &module.function_types[*id];
            let parameters: Vec<_> = function
                .parameter_types
                .iter()
                .map(|ty| type_name(module, ty))
                .collect();
            format!(
                "FunPtr<({}) -> {}>",
                parameters.join(", "),
                type_name(module, &function.return_type)
            )
        }
        Type::Enum(id, args) => {
            let name = &module.enums[*id].name;
            if args.is_empty() {
                name.clone()
            } else {
                let inner: Vec<String> = args.iter().map(|t| type_name(module, t)).collect();
                format!("{}<{}>", name, inner.join(", "))
            }
        }
    }
}

fn dump_statements(
    module: &Module,
    locals: &Arena<Local>,
    statements: &[Statement],
    indent: usize,
    out: &mut String,
) {
    for statement in statements {
        let pad = "  ".repeat(indent);
        match &statement.kind {
            StatementKind::Expr(expr) => dump_expr(module, locals, expr, indent, out),
            StatementKind::Call(effect) => match effect {
                CallEffect::Unit(call) => dump_call(module, locals, call, None, indent, out),
                CallEffect::Value { destination, call } => {
                    dump_call(module, locals, call, Some(*destination), indent, out)
                }
            },
            StatementKind::ValDecl { local, init } => {
                let local = &locals[*local];
                let keyword = if local.mutable { "var" } else { "val" };
                out.push_str(&format!(
                    "{pad}{keyword} {}: {}\n",
                    local.name,
                    type_name(module, &local.ty)
                ));
                dump_expr(module, locals, init, indent + 1, out);
            }
            StatementKind::FieldSet {
                object,
                index,
                value,
            } => {
                out.push_str(&format!("{pad}field_set {index}\n"));
                dump_expr(module, locals, object, indent + 1, out);
                dump_expr(module, locals, value, indent + 1, out);
            }
            StatementKind::AtomicFieldStore {
                object,
                index,
                value,
            } => {
                out.push_str(&format!("{pad}atomic_store_release field={index}\n"));
                dump_expr(module, locals, object, indent + 1, out);
                dump_expr(module, locals, value, indent + 1, out);
            }
            StatementKind::ArraySet {
                array_type,
                array,
                index,
                value,
            } => {
                out.push_str(&format!(
                    "{pad}array_set {}\n",
                    module.classes[*array_type].name
                ));
                dump_expr(module, locals, array, indent + 1, out);
                dump_expr(module, locals, index, indent + 1, out);
                dump_expr(module, locals, value, indent + 1, out);
            }
            StatementKind::Assign { local, value } => {
                out.push_str(&format!("{pad}assign {}\n", locals[*local].name));
                dump_expr(module, locals, value, indent + 1, out);
            }
            StatementKind::GlobalAssign { global, value } => {
                out.push_str(&format!(
                    "{pad}global_assign {}\n",
                    module.globals[*global].name
                ));
                dump_expr(module, locals, value, indent + 1, out);
            }
            StatementKind::Eh(eh) => match eh {
                EhStatement::LandingPad { cleanup } => {
                    out.push_str(&format!("{pad}landing_pad cleanup={cleanup}\n"));
                }
                EhStatement::BeginCatch => out.push_str(&format!("{pad}begin_catch\n")),
                EhStatement::EndCatch => out.push_str(&format!("{pad}end_catch\n")),
            },
        }
    }
}

fn block_number(id: BlockId) -> u32 {
    id.into_raw().into_u32()
}

fn dump_terminator(
    module: &Module,
    locals: &Arena<Local>,
    terminator: &Terminator,
    indent: usize,
    out: &mut String,
) {
    let pad = "  ".repeat(indent);
    match terminator {
        Terminator::Goto(target) => {
            out.push_str(&format!("{pad}goto bb{}\n", block_number(*target)));
        }
        Terminator::Branch {
            cond,
            then_block,
            else_block,
        } => {
            out.push_str(&format!(
                "{pad}branch bb{} bb{}\n",
                block_number(*then_block),
                block_number(*else_block)
            ));
            dump_expr(module, locals, cond, indent + 1, out);
        }
        Terminator::Return { value } => {
            out.push_str(&format!("{pad}return\n"));
            if let Some(value) = value {
                dump_expr(module, locals, value, indent + 1, out);
            }
        }
        Terminator::Throw { exception, unwind } => {
            let edge = unwind
                .map(|target| format!(" unwind bb{}", block_number(target)))
                .unwrap_or_default();
            out.push_str(&format!("{pad}throw{edge}\n"));
            dump_expr(module, locals, exception, indent + 1, out);
        }
        Terminator::Rethrow { unwind } => {
            let edge = unwind
                .map(|target| format!(" unwind bb{}", block_number(target)))
                .unwrap_or_default();
            out.push_str(&format!("{pad}rethrow{edge}\n"));
        }
        Terminator::Resume => out.push_str(&format!("{pad}resume\n")),
        Terminator::Trap { message } => {
            out.push_str(&format!("{pad}trap @{}\n", module.strings[*message].symbol));
        }
        Terminator::Unreachable => out.push_str(&format!("{pad}unreachable\n")),
    }
}

fn dump_expr(module: &Module, locals: &Arena<Local>, expr: &Expr, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    out.push_str(&format!("{pad}Type {}\n", type_name(module, &expr.ty)));
    match &expr.kind {
        ExprKind::StringConst(id) => {
            out.push_str(&format!(
                "{pad}StringConst @{}\n",
                module.strings[*id].symbol
            ));
        }
        ExprKind::IntLiteral(value) => out.push_str(&format!("{pad}IntLiteral {value}\n")),
        ExprKind::BoolLiteral(value) => out.push_str(&format!("{pad}BoolLiteral {value}\n")),
        ExprKind::UnitLiteral => out.push_str(&format!("{pad}UnitLiteral\n")),
        ExprKind::TupleLiteral(elements) => {
            out.push_str(&format!("{pad}TupleLiteral\n"));
            for element in elements {
                dump_expr(module, locals, element, indent + 1, out);
            }
        }
        ExprKind::ClassInit { class_id, args } => {
            out.push_str(&format!(
                "{pad}ClassInit {}\n",
                module.classes[*class_id].name
            ));
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::ClosureAlloc { class, captures } => {
            out.push_str(&format!(
                "{pad}ClosureAlloc cc{} {}\n",
                class.into_raw().into_u32(),
                module.closure_classes[*class].name
            ));
            for capture in captures {
                dump_expr(module, locals, capture, indent + 1, out);
            }
        }
        ExprKind::ClosureCapture {
            closure,
            class,
            index,
        } => {
            out.push_str(&format!(
                "{pad}ClosureCapture cc{} {index}\n",
                class.into_raw().into_u32()
            ));
            dump_expr(module, locals, closure, indent + 1, out);
        }
        ExprKind::StructInit { struct_id, args } => {
            out.push_str(&format!(
                "{pad}StructInit {}\n",
                module.structs[*struct_id].name
            ));
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::Local(local) => out.push_str(&format!("{pad}Local {}\n", locals[*local].name)),
        ExprKind::GlobalRead(global) => out.push_str(&format!(
            "{pad}GlobalRead {}\n",
            module.globals[*global].name
        )),
        ExprKind::PtrFromUInt { operand, pointee } => {
            out.push_str(&format!(
                "{pad}PtrFromUInt {}\n",
                type_name(module, pointee)
            ));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::PtrToUInt(operand) => {
            out.push_str(&format!("{pad}PtrToUInt\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::PtrCast { operand, pointee } => {
            out.push_str(&format!("{pad}PtrCast {}\n", type_name(module, pointee)));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::PtrLoad {
            pointer,
            pointee,
            offset,
        } => {
            out.push_str(&format!("{pad}PtrLoad {}\n", type_name(module, pointee)));
            dump_expr(module, locals, pointer, indent + 1, out);
            if let Some(offset) = offset {
                dump_expr(module, locals, offset, indent + 1, out);
            }
        }
        ExprKind::PtrStore {
            pointer,
            pointee,
            offset,
            value,
        } => {
            out.push_str(&format!("{pad}PtrStore {}\n", type_name(module, pointee)));
            dump_expr(module, locals, pointer, indent + 1, out);
            if let Some(offset) = offset {
                dump_expr(module, locals, offset, indent + 1, out);
            }
            dump_expr(module, locals, value, indent + 1, out);
        }
        ExprKind::PtrOffset {
            pointer,
            pointee,
            offset,
            subtract,
        } => {
            out.push_str(&format!(
                "{pad}PtrOffset {} subtract={subtract}\n",
                type_name(module, pointee)
            ));
            dump_expr(module, locals, pointer, indent + 1, out);
            dump_expr(module, locals, offset, indent + 1, out);
        }
        ExprKind::AddressOf { local, pointee } => out.push_str(&format!(
            "{pad}AddressOf {} : Ptr<{}>\n",
            locals[*local].name,
            type_name(module, pointee)
        )),
        ExprKind::GlobalAddress { global, pointee } => out.push_str(&format!(
            "{pad}GlobalAddress {} {}\n",
            module.globals[*global].name,
            type_name(module, pointee)
        )),
        ExprKind::SizeOf(ty) => {
            out.push_str(&format!("{pad}SizeOf {}\n", type_name(module, ty)));
        }
        ExprKind::AlignOf(ty) => {
            out.push_str(&format!("{pad}AlignOf {}\n", type_name(module, ty)));
        }
        ExprKind::FunPtrNull(signature) => out.push_str(&format!(
            "{pad}FunPtrNull function_type{}\n",
            signature.into_raw().into_u32()
        )),
        ExprKind::FunctionAddress { callback } => out.push_str(&format!(
            "{pad}FunctionAddress cb{}\n",
            callback.into_raw().into_u32()
        )),
        ExprKind::ForeignCallbackRegister { bridge, closure } => {
            let bridge_id = *bridge;
            let bridge = &module.foreign_callback_bridges[bridge_id];
            let adapter = &module.foreign_callback_adapters[bridge.adapter];
            out.push_str(&format!(
                "{pad}ForeignCallbackRegister fcb{} native=function_type{} managed=function_type{} context={} mode={:?} adapter=@{}\n",
                bridge_id.into_raw().into_u32(),
                bridge.native_signature.into_raw().into_u32(),
                adapter.managed_signature.into_raw().into_u32(),
                bridge.context_index,
                bridge.mode,
                module.functions[adapter.function].symbol,
            ));
            dump_expr(module, locals, closure, indent + 1, out);
        }
        ExprKind::ForeignCallbackOperation {
            operation,
            callback,
            ..
        } => {
            out.push_str(&format!("{pad}ForeignCallback{operation:?}\n"));
            dump_expr(module, locals, callback, indent + 1, out);
        }
        ExprKind::CaughtException => out.push_str(&format!("{pad}CaughtException\n")),
        ExprKind::Retype { operand, ty } => {
            out.push_str(&format!("{pad}Retype {}\n", type_name(module, ty)));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::FieldAccess { receiver, index } => {
            out.push_str(&format!("{pad}FieldAccess {index}\n"));
            dump_expr(module, locals, receiver, indent + 1, out);
        }
        ExprKind::AtomicFieldLoad { object, index } => {
            out.push_str(&format!("{pad}AtomicLoadAcquire field={index}\n"));
            dump_expr(module, locals, object, indent + 1, out);
        }
        ExprKind::AtomicFieldCompareExchange {
            object,
            index,
            expected,
            replacement,
        } => {
            out.push_str(&format!(
                "{pad}AtomicCompareExchange field={index} success=acq_rel failure=acquire\n"
            ));
            dump_expr(module, locals, object, indent + 1, out);
            dump_expr(module, locals, expected, indent + 1, out);
            dump_expr(module, locals, replacement, indent + 1, out);
        }
        ExprKind::Box(operand) => {
            out.push_str(&format!("{pad}Box\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::Unbox(operand) => {
            out.push_str(&format!("{pad}Unbox\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::IsInstance { operand, check_ty } => {
            out.push_str(&format!(
                "{pad}IsInstance {}\n",
                type_name(module, check_ty)
            ));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::Cast { operand, optional } => {
            out.push_str(&format!("{pad}Cast optional={optional}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::ArrayLiteral {
            array_type,
            elements,
        } => {
            out.push_str(&format!(
                "{pad}ArrayLiteral {}\n",
                module.classes[*array_type].name
            ));
            for element in elements {
                dump_expr(module, locals, element, indent + 1, out);
            }
        }
        ExprKind::ArrayGet {
            array_type,
            array,
            index,
        } => {
            out.push_str(&format!(
                "{pad}ArrayGet {}\n",
                module.classes[*array_type].name
            ));
            dump_expr(module, locals, array, indent + 1, out);
            dump_expr(module, locals, index, indent + 1, out);
        }
        ExprKind::ArrayLen {
            array_type,
            operand,
        } => {
            out.push_str(&format!(
                "{pad}ArrayLen {}\n",
                module.classes[*array_type].name
            ));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::ArrayClone {
            source_type,
            target_type,
            operand,
        } => {
            out.push_str(&format!(
                "{pad}ArrayClone {} -> {}\n",
                module.classes[*source_type].name, module.classes[*target_type].name
            ));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::Binary { op, lhs, rhs } => {
            out.push_str(&format!("{pad}Binary {op:?}\n"));
            dump_expr(module, locals, lhs, indent + 1, out);
            dump_expr(module, locals, rhs, indent + 1, out);
        }
        ExprKind::Unary { op, operand } => {
            out.push_str(&format!("{pad}Unary {op:?}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::VariantConstruct { variant, fields } => {
            out.push_str(&format!(
                "{pad}VariantConstruct {} v{}\n",
                type_name(module, &expr.ty),
                variant
            ));
            for field in fields {
                dump_expr(module, locals, field, indent + 1, out);
            }
        }
        ExprKind::EnumTag(operand) => {
            out.push_str(&format!("{pad}EnumTag\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::EnumField {
            operand,
            variant,
            index,
        } => {
            out.push_str(&format!("{pad}EnumField v{variant} f{index}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
    }
}

fn dump_call(
    module: &Module,
    locals: &Arena<Local>,
    call: &Call,
    destination: Option<LocalId>,
    indent: usize,
    out: &mut String,
) {
    let pad = "  ".repeat(indent);
    let callee = match &call.target.callee {
        Callee::User(id) => format!("@{}", module.functions[*id].symbol),
        Callee::Monomorphized(id) => format!("@{}", module.meta.instances[*id].symbol),
        Callee::Extern(id) => format!(
            "extern{} @{}",
            id.into_raw(),
            module.extern_functions[*id].native_symbol
        ),
        Callee::CoroutineSuspend { register } => format!(
            "@coroutine_suspend[register=@{}]",
            module.meta.instances[*register].symbol
        ),
        Callee::Closure(function_type) => {
            format!(
                "<closure:function_type{}>",
                function_type.into_raw().into_u32()
            )
        }
        Callee::FunctionBridge(function_type) => format!(
            "<function_bridge:function_type{}>",
            function_type.into_raw().into_u32()
        ),
        Callee::Runtime(function) => format!("@{}", function.symbol()),
    };
    let kind = match &call.target.kind {
        CallKind::Direct => "direct".to_string(),
        CallKind::Virtual { slot } => format!("virtual[{slot}]"),
        CallKind::Interface { interface, slot } => {
            format!("interface {}[{slot}]", module.interfaces[*interface].name)
        }
        CallKind::Closure { function_type } => {
            format!(
                "closure[function_type{}]",
                function_type.into_raw().into_u32()
            )
        }
        CallKind::FunctionBridge { function_type } => format!(
            "function_bridge[function_type{}]",
            function_type.into_raw().into_u32()
        ),
    };
    match destination {
        Some(local) => out.push_str(&format!(
            "{pad}call {}: {} = {callee} {kind}\n",
            locals[local].name,
            type_name(module, &locals[local].ty)
        )),
        None => out.push_str(&format!("{pad}call {callee} {kind}\n")),
    }
    for arg in &call.args {
        dump_expr(module, locals, arg, indent + 1, out);
    }
}
