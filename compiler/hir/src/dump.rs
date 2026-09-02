use super::*;

/// Indented text dump for golden tests (`scoopc build --emit=hir`).
pub fn dump(module: &Module) -> String {
    let mut out = String::from("Module\n");
    for (id, decl) in module.structs.iter() {
        if id == module.ffi_core.ptr
            || id == module.ffi_core.fun_ptr
            || id == module.ffi_core.pinned_ptr
            || id == module.ffi_core.gc_handle
            || id == module.foreign_callback_core.callback
        {
            continue;
        }
        if matches!(decl.representation, StructRepresentation::Intrinsic(_)) {
            continue;
        }
        let type_params = if decl.type_params.is_empty() {
            String::new()
        } else {
            dump_type_params(module, &decl.type_params)
        };
        let interfaces = dump_interface_list(module, &decl.interfaces);
        let attributes = dump_struct_attributes(decl.attributes);
        out.push_str(&format!(
            "  struct {}{}{}{}\n",
            decl.name, type_params, interfaces, attributes
        ));
        for field in decl.semantic_fields() {
            out.push_str(&format!(
                "    field {}: {}\n",
                field.name,
                type_name(module, field.ty)
            ));
        }
    }
    for (id, decl) in module.enums.iter() {
        if id == module.foreign_callback_core.mode || id == module.foreign_callback_core.state {
            continue;
        }
        let type_params = if decl.type_params.is_empty() {
            String::new()
        } else {
            dump_type_params(module, &decl.type_params)
        };
        let interfaces = dump_interface_list(module, &decl.interfaces);
        let attributes = if decl.no_gc { " <no-gc>" } else { "" };
        out.push_str(&format!(
            "  enum {}{}{}{}\n",
            decl.name, type_params, interfaces, attributes
        ));
        for variant in &decl.variants {
            let fields: Vec<String> = variant
                .fields
                .iter()
                .map(|f| format!("{}: {}", f.name, type_name(module, f.ty)))
                .collect();
            out.push_str(&format!("    {}({})\n", variant.name, fields.join(", ")));
        }
    }
    for (_, decl) in module.classes.iter() {
        if matches!(decl.representation, ClassRepresentation::Intrinsic(_)) {
            continue;
        }
        let modifier = match decl.modifier {
            ClassModifier::Final => "",
            ClassModifier::Open => "open ",
            ClassModifier::Abstract => "abstract ",
        };
        let ctor: Vec<String> = decl
            .semantic_constructor()
            .iter()
            .map(|f| format!("{}: {}", f.name, type_name(module, f.ty)))
            .collect();
        let type_params = if decl.type_params.is_empty() {
            String::new()
        } else {
            dump_type_params(module, &decl.type_params)
        };
        let interfaces = dump_interface_list(module, &decl.interfaces);
        out.push_str(&format!(
            "  {modifier}class {}{}({}){}\n",
            decl.name,
            type_params,
            ctor.join(", "),
            interfaces
        ));
    }
    for (_, decl) in module.interfaces.iter() {
        let type_params = if decl.type_params.is_empty() {
            String::new()
        } else {
            dump_type_params(module, &decl.type_params)
        };
        let parents = if decl.parents.is_empty() {
            String::new()
        } else {
            format!(
                " : {}",
                decl.parents
                    .iter()
                    .map(|parent| type_name(
                        module,
                        module.interface_applications[*parent].canonical_type
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        out.push_str(&format!(
            "  interface {}{}{}\n",
            decl.name, type_params, parents
        ));
        for method in &decl.methods {
            let function = &module.functions[module.interface_methods[*method].function];
            let params: Vec<String> = function
                .params
                .iter()
                .skip(1)
                .map(|param| format!("{}: {}", param.name, type_name(module, param.ty)))
                .collect();
            out.push_str(&format!(
                "    {}{}fun {}({}): {}{}\n",
                match function.method.and_then(|method| method.operator) {
                    Some(OperatorKind::Equals) => "operator ",
                    None => "",
                },
                if function.is_suspend { "suspend " } else { "" },
                function.name.rsplit('.').next().unwrap_or(&function.name),
                params.join(", "),
                type_name(module, function.return_ty),
                dump_function_attributes(function.attributes)
            ));
        }
    }
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
                library,
                thread_local,
            } => format!(
                "extern symbol={native_symbol}{}{}",
                if library.is_empty() {
                    String::new()
                } else {
                    format!(" lib={library}")
                },
                if *thread_local { " thread_local" } else { "" }
            ),
        };
        out.push_str(&format!(
            "  {} {}: {} <global{} {storage}>\n",
            if global.mutable { "var" } else { "val" },
            global.name,
            type_name(module, global.ty),
            id.into_raw()
        ));
    }
    for &id in &module.top_level {
        if [
            module.ffi_core.address_of,
            module.ffi_core.size_of,
            module.ffi_core.align_of,
            module.ffi_core.gc_pin_raw,
            module.ffi_core.gc_unpin_raw,
            module.ffi_core.gc_get_handle_raw,
            module.ffi_core.gc_release_handle_raw,
            module.foreign_callback_core.register,
            module.foreign_callback_core.retain,
            module.foreign_callback_core.release,
            module.foreign_callback_core.query_state,
            module.foreign_callback_core.failure,
        ]
        .contains(&id)
        {
            continue;
        }
        let function = &module.functions[id];
        let function_type_params: Vec<_> = function.type_params().into_iter().cloned().collect();
        let type_params = if function_type_params.is_empty() {
            String::new()
        } else {
            dump_type_params(module, &function_type_params)
        };
        let params: Vec<String> = match function.kind {
            FunctionKind::Extern(id) => module.extern_functions[id]
                .params
                .iter()
                .enumerate()
                .map(|(index, &ty)| format!("arg{}: {}", index + 1, type_name(module, ty)))
                .collect(),
            _ => function
                .params
                .iter()
                .map(|p| format!("{}: {}", p.name, type_name(module, p.ty)))
                .collect(),
        };
        let signature = format!(
            "{}{}({}): {}",
            function.name,
            type_params,
            params.join(", "),
            type_name(module, function.return_ty)
        );
        let suspend = if function.is_suspend { "suspend " } else { "" };
        let operator = match function.method.and_then(|method| method.operator) {
            Some(OperatorKind::Equals) => "operator ",
            None => "",
        };
        let attributes = dump_function_attributes(function.attributes);
        let no_gc_requirements = match &function.genericity {
            FunctionGenericity::Plain => &[][..],
            FunctionGenericity::Generic { definition, .. } => {
                &module.generic_functions[*definition].no_gc_type_params
            }
            FunctionGenericity::OwnerParameterizedMethod {
                no_gc_type_params, ..
            } => no_gc_type_params,
            FunctionGenericity::GenericMethod { definition, .. } => {
                &module.generic_methods[*definition].no_gc_type_params
            }
        };
        let no_gc_condition = if no_gc_requirements.is_empty() {
            String::new()
        } else {
            let parameters = no_gc_requirements
                .iter()
                .map(|parameter| function.type_param(*parameter).name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!(" <requires-gc-free {parameters}>")
        };
        match &function.kind {
            FunctionKind::Intrinsic(intrinsic) => {
                out.push_str(&format!(
                    "  {operator}{suspend}fun {signature}{attributes}{no_gc_condition} <intrinsic {}>\n",
                    intrinsic.kind.name(),
                ));
            }
            FunctionKind::User(body) => {
                out.push_str(&format!(
                    "  {operator}{suspend}fun {signature}{attributes}{no_gc_condition}\n"
                ));
                dump_statements(module, &body.locals, &body.statements, 2, &mut out);
            }
            FunctionKind::DerivedEquality => {
                out.push_str(&format!(
                    "  {operator}{suspend}fun {signature}{attributes}{no_gc_condition} <derived equality>\n"
                ));
            }
            FunctionKind::Extern(id) => {
                let extern_ = &module.extern_functions[*id];
                let abi = match extern_.abi {
                    ExternAbi::C => "c",
                    ExternAbi::Scoop => "scoop",
                };
                let library = if extern_.library.is_empty() {
                    String::new()
                } else {
                    format!(" lib={}", extern_.library)
                };
                out.push_str(&format!(
                    "  fun {signature}{attributes}{no_gc_condition} <extern{} abi={abi} symbol={}{}>\n",
                    id.into_raw(),
                    extern_.native_symbol,
                    library
                ));
            }
        }
    }
    out.push_str(&format!(
        "  entry {}\n",
        module.functions[module.entry].name
    ));
    for (_, instantiation) in module.instantiations.iter() {
        let function = module.generic_functions[instantiation.generic].function;
        if [
            module.ffi_core.gc_pin_raw,
            module.ffi_core.gc_unpin_raw,
            module.ffi_core.gc_get_handle_raw,
            module.ffi_core.gc_release_handle_raw,
        ]
        .contains(&function)
        {
            continue;
        }
        let args: Vec<String> = instantiation
            .type_args
            .iter()
            .map(|t| type_name(module, *t))
            .collect();
        out.push_str(&format!(
            "  instance {}<{}>\n",
            module.functions[function].name,
            args.join(", ")
        ));
    }
    for (_, application) in module.generic_method_applications.iter() {
        let function = module.generic_methods[application.method].function;
        let owner = generic_method_owner_arguments(module, application.owner)
            .iter()
            .map(|argument| type_name(module, *argument))
            .collect::<Vec<_>>()
            .join(", ");
        let method = application
            .method_arguments
            .iter()
            .map(|argument| type_name(module, *argument))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "  method instance {}<owner=[{}], method=[{}]>\n",
            module.functions[function].name, owner, method
        ));
    }
    out
}

fn dump_type_params(module: &Module, params: &[TypeParamDecl]) -> String {
    let params = params
        .iter()
        .map(|param| {
            let variance = match param.variance {
                Variance::Invariant => "",
                Variance::In => "in ",
                Variance::Out => "out ",
            };
            let bounds = match &param.bounds {
                TypeParamBounds::Unconstrained => String::new(),
                TypeParamBounds::Value { .. } => " : value".to_string(),
                TypeParamBounds::Ref { .. } => " : ref".to_string(),
                TypeParamBounds::Interfaces(bounds) => format!(
                    " : {}",
                    bounds
                        .iter()
                        .map(|bound| {
                            let application = &module.interface_applications[bound.application];
                            let name = &module.interfaces[application.template].name;
                            if application.arguments.is_empty() {
                                name.clone()
                            } else {
                                let arguments = application
                                    .arguments
                                    .iter()
                                    .map(|ty| type_name_with_params(module, *ty, params))
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                format!("{name}<{arguments}>")
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" & ")
                ),
            };
            format!("{variance}{}{bounds}", param.name)
        })
        .collect::<Vec<_>>();
    format!("<{}>", params.join(", "))
}

fn dump_function_attributes(attributes: FunctionAttributes) -> String {
    let mut values = Vec::new();
    if attributes.safety == Safety::Unsafe {
        values.push("unsafe");
    }
    if attributes.gc_effect == GcEffect::NoGc {
        values.push("no-gc");
        values.push(match attributes.calling_convention {
            CallingConvention::Cdecl => "cdecl",
        });
    }
    if values.is_empty() {
        String::new()
    } else {
        format!(" <{}>", values.join(" "))
    }
}

fn dump_struct_attributes(attributes: StructAttributes) -> String {
    let mut values = Vec::new();
    if attributes.no_gc {
        values.push("no-gc".to_string());
    }
    if let Some(layout) = attributes.c_layout {
        values.push(format!(
            "c-layout aligned={} packed={}",
            layout.aligned, layout.packed
        ));
    }
    if attributes.interior_mutable {
        values.push("interior-mutable".to_string());
    }
    if values.is_empty() {
        String::new()
    } else {
        format!(" <{}>", values.join(" "))
    }
}

fn dump_interface_list(module: &Module, interfaces: &[TypeId]) -> String {
    if interfaces.is_empty() {
        String::new()
    } else {
        let names: Vec<String> = interfaces.iter().map(|&ty| type_name(module, ty)).collect();
        format!(" : {}", names.join(", "))
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
            StatementKind::LocalFunction(id) => {
                let local = &module.local_functions[*id];
                out.push_str(&format!(
                    "{pad}LocalFunction local{} body={} captures={}\n",
                    id.into_raw(),
                    module.functions[local.function].name,
                    local.captures.len()
                ));
            }
            StatementKind::Return { value } => {
                out.push_str(&format!("{pad}return\n"));
                if let Some(value) = value {
                    dump_expr(module, locals, value, indent + 1, out);
                }
            }
            StatementKind::ValDecl { pattern, init } => {
                out.push_str(&format!("{pad}val {}\n", dump_pattern(pattern)));
                dump_expr(module, locals, init, indent + 1, out);
            }
            StatementKind::Assign { target, value } => {
                match target {
                    AssignTarget::Local(local) => {
                        out.push_str(&format!("{pad}assign {}\n", locals[*local].name))
                    }
                    AssignTarget::Global(global) => out.push_str(&format!(
                        "{pad}assign global {}\n",
                        module.globals[*global].name
                    )),
                    AssignTarget::Field { receiver, .. } => {
                        out.push_str(&format!("{pad}assign .field\n"));
                        dump_expr(module, locals, receiver, indent + 1, out);
                    }
                    AssignTarget::Index { array, index } => {
                        out.push_str(&format!("{pad}assign []\n"));
                        dump_expr(module, locals, array, indent + 1, out);
                        dump_expr(module, locals, index, indent + 1, out);
                    }
                }
                dump_expr(module, locals, value, indent + 1, out);
            }
            StatementKind::If {
                cond,
                then_body,
                else_body,
            } => {
                out.push_str(&format!("{pad}if\n"));
                dump_expr(module, locals, cond, indent + 1, out);
                dump_statements(module, locals, then_body, indent + 1, out);
                if let Some(else_body) = else_body {
                    out.push_str(&format!("{pad}else\n"));
                    dump_statements(module, locals, else_body, indent + 1, out);
                }
            }
            StatementKind::While { cond, body } => {
                out.push_str(&format!("{pad}while\n"));
                dump_expr(module, locals, cond, indent + 1, out);
                dump_statements(module, locals, body, indent + 1, out);
            }
            StatementKind::Try(try_) => {
                out.push_str(&format!("{pad}try\n"));
                dump_statements(module, locals, &try_.body, indent + 1, out);
                for catch in &try_.catches {
                    out.push_str(&format!(
                        "{pad}catch {}: {}\n",
                        locals[catch.local].name,
                        type_name(module, catch.ty)
                    ));
                    dump_statements(module, locals, &catch.body, indent + 1, out);
                }
                if let Some(finally_body) = &try_.finally_body {
                    out.push_str(&format!("{pad}finally\n"));
                    dump_statements(module, locals, finally_body, indent + 1, out);
                }
            }
            StatementKind::Throw(expr) => {
                out.push_str(&format!("{pad}throw\n"));
                dump_expr(module, locals, expr, indent + 1, out);
            }
            StatementKind::When(when) => {
                out.push_str(&format!("{pad}when\n"));
                dump_expr(module, locals, &when.subject, indent + 1, out);
                for arm in &when.arms {
                    out.push_str(&format!(
                        "{}  arm {}{}\n",
                        pad,
                        dump_pattern(&arm.pattern),
                        if arm.guard.is_some() {
                            " if <guard>"
                        } else {
                            ""
                        }
                    ));
                    dump_statements(module, locals, &arm.body, indent + 2, out);
                }
                if let Some(else_body) = &when.else_body {
                    out.push_str(&format!("{pad}  else\n"));
                    dump_statements(module, locals, else_body, indent + 2, out);
                }
            }
        }
    }
}

/// Compact one-line pattern rendering for dumps.
pub fn dump_pattern(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Binding { local } => format!("local{}", local.into_raw()),
        Pattern::Wildcard => "_".to_string(),
        Pattern::Literal { value, .. } => {
            format!("<lit {:?}>", value.kind).chars().take(40).collect()
        }
        Pattern::Variant {
            variant, fields, ..
        } => {
            let fields: Vec<String> = fields
                .iter()
                .map(|(i, p)| format!("{i}: {}", dump_pattern(p)))
                .collect();
            format!("variant{}({})", variant, fields.join(", "))
        }
        Pattern::Tuple(elements) => {
            let parts: Vec<String> = elements.iter().map(dump_pattern).collect();
            format!("({})", parts.join(", "))
        }
        Pattern::Struct { fields, .. } => {
            let fields: Vec<String> = fields
                .iter()
                .map(|(i, p)| format!("{i}: {}", dump_pattern(p)))
                .collect();
            format!("struct({})", fields.join(", "))
        }
    }
}

/// Compose application arguments only for the human-readable HIR dump. No
/// semantic consumer receives this flattened presentation value.
fn callable_dump_parts(module: &Module, callable: Callable) -> (FunctionId, Vec<TypeId>) {
    match callable {
        Callable::Function(function) => (function, Vec::new()),
        Callable::Generic(id) => {
            let resolved = &module.instantiations[id];
            (
                module.generic_functions[resolved.generic].function,
                resolved.type_args.clone(),
            )
        }
        Callable::Method(id) => {
            let application = &module.method_applications[id];
            (
                application.function,
                method_owner_arguments(module, application.owner).to_vec(),
            )
        }
        Callable::GenericMethod(id) => {
            let application = &module.generic_method_applications[id];
            let mut arguments = generic_method_owner_arguments(module, application.owner).to_vec();
            arguments.extend(application.method_arguments.iter().copied());
            (
                module.generic_methods[application.method].function,
                arguments,
            )
        }
    }
}

fn method_owner_arguments(module: &Module, owner: MethodOwnerApplication) -> &[TypeId] {
    match owner {
        MethodOwnerApplication::Class(id) => &module.class_applications[id].arguments,
        MethodOwnerApplication::Struct(id) => &module.struct_applications[id].arguments,
        MethodOwnerApplication::Enum(id) => &module.enum_applications[id].arguments,
        MethodOwnerApplication::Interface(id) => &module.interface_applications[id].arguments,
    }
}

fn generic_method_owner_arguments(module: &Module, owner: GenericMethodOwner) -> &[TypeId] {
    match owner {
        GenericMethodOwner::Class(id) => &module.class_applications[id].arguments,
        GenericMethodOwner::Struct(id) => &module.struct_applications[id].arguments,
        GenericMethodOwner::Enum(id) => &module.enum_applications[id].arguments,
    }
}

fn dump_expr(module: &Module, locals: &Arena<Local>, expr: &Expr, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    let ty = type_name(module, expr.ty);
    match &expr.kind {
        ExprKind::StringLiteral(value) => {
            out.push_str(&format!("{pad}StringLiteral {value:?} : {ty}\n"));
        }
        ExprKind::IntLiteral(value) => out.push_str(&format!("{pad}IntLiteral {value} : {ty}\n")),
        ExprKind::BoolLiteral(value) => out.push_str(&format!("{pad}BoolLiteral {value} : {ty}\n")),
        ExprKind::UnitLiteral => out.push_str(&format!("{pad}UnitLiteral : {ty}\n")),
        ExprKind::TupleLiteral(elements) => {
            out.push_str(&format!("{pad}TupleLiteral : {ty}\n"));
            for element in elements {
                dump_expr(module, locals, element, indent + 1, out);
            }
        }
        ExprKind::ClassInit { application, args } => {
            let application = &module.class_applications[*application];
            let name = &module.classes[application.template].name;
            let arguments = application
                .arguments
                .iter()
                .map(|ty| type_name(module, *ty))
                .collect::<Vec<_>>();
            let constructed = if arguments.is_empty() {
                name.clone()
            } else {
                format!("{name}<{}>", arguments.join(", "))
            };
            out.push_str(&format!("{pad}ClassInit {} : {ty}\n", constructed));
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::ConstructorParam(parameter) => out.push_str(&format!(
            "{pad}ConstructorParam #{} : {ty}\n",
            parameter.into_raw()
        )),
        ExprKind::StructInit { application, args } => {
            let application = &module.struct_applications[*application];
            out.push_str(&format!(
                "{pad}StructInit {} : {ty}\n",
                module.structs[application.template].name
            ));
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::VariantConstruct {
            application,
            variant,
            args,
        } => {
            let application = &module.enum_applications[*application];
            let decl = &module.enums[application.template];
            let type_args = if application.arguments.is_empty() {
                String::new()
            } else {
                let args: Vec<String> = application
                    .arguments
                    .iter()
                    .map(|t| type_name(module, *t))
                    .collect();
                format!("<{}>", args.join(", "))
            };
            out.push_str(&format!(
                "{pad}VariantConstruct {}.{}{type_args} : {ty}\n",
                decl.name, decl.variants[*variant as usize].name
            ));
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::Local(local) => {
            out.push_str(&format!("{pad}Local {} : {ty}\n", locals[*local].name));
        }
        ExprKind::GlobalRead(global) => out.push_str(&format!(
            "{pad}GlobalRead {} : {ty}\n",
            module.globals[*global].name
        )),
        ExprKind::Capture(binding) => {
            out.push_str(&format!(
                "{pad}Capture binding{} : {ty}\n",
                binding.into_raw()
            ));
        }
        ExprKind::Lambda(id) => {
            let lambda = &module.lambdas[*id];
            out.push_str(&format!(
                "{pad}Lambda lambda{} invoke={} captures={} : {ty}\n",
                id.into_raw(),
                module.functions[lambda.function].name,
                lambda.captures.len()
            ));
            for capture in &lambda.captures {
                out.push_str(&format!(
                    "{}capture {} binding{} : {}\n",
                    "  ".repeat(indent + 1),
                    capture.name,
                    capture.binding.into_raw(),
                    type_name(module, capture.ty)
                ));
            }
        }
        ExprKind::AnonymousFunction(id) => {
            let anonymous = &module.anonymous_functions[*id];
            out.push_str(&format!(
                "{pad}AnonymousFunction anonymous{} invoke={} captures={} : {ty}\n",
                id.into_raw(),
                module.functions[anonymous.function].name,
                anonymous.captures.len()
            ));
            for capture in &anonymous.captures {
                out.push_str(&format!(
                    "{}capture {} binding{} : {}\n",
                    "  ".repeat(indent + 1),
                    capture.name,
                    capture.binding.into_raw(),
                    type_name(module, capture.ty)
                ));
            }
        }
        ExprKind::CallableReference(id) => {
            let reference = &module.callable_references[*id];
            let (function, receiver) = match &reference.target {
                CallableReferenceTarget::Named(callable) => {
                    (callable_function(module, *callable), None)
                }
                CallableReferenceTarget::Local { callee, .. } => {
                    (callable_function(module, *callee), None)
                }
                CallableReferenceTarget::BoundMember { receiver, callee } => (
                    method_callee_function(module, *callee),
                    Some(receiver.as_ref()),
                ),
                CallableReferenceTarget::BoundExtension { receiver, callee } => {
                    (callable_function(module, *callee), Some(receiver.as_ref()))
                }
            };
            out.push_str(&format!(
                "{pad}CallableReference reference{} target={} captures={} : {ty}\n",
                id.into_raw(),
                module.functions[function].name,
                reference.captures.len()
            ));
            if let Some(receiver) = receiver {
                dump_expr(module, locals, receiver, indent + 1, out);
            }
        }
        ExprKind::FunctionCoercion {
            source,
            coercion,
            target_type,
        } => {
            let conversion = &module.function_coercions[*coercion];
            debug_assert_eq!(conversion.target, *target_type);
            out.push_str(&format!(
                "{pad}FunctionCoercion coercion{} {} -> {} : {ty}\n",
                coercion.into_raw().into_u32(),
                type_name(
                    module,
                    module
                        .types
                        .iter()
                        .find_map(|(ty, value)| {
                            matches!(value, Type::Function(id) if *id == conversion.source)
                                .then_some(ty)
                        })
                        .expect("a function signature has a canonical type")
                ),
                type_name(
                    module,
                    module
                        .types
                        .iter()
                        .find_map(|(ty, value)| {
                            matches!(value, Type::Function(id) if *id == conversion.target)
                                .then_some(ty)
                        })
                        .expect("a function signature has a canonical type")
                )
            ));
            dump_expr(module, locals, source, indent + 1, out);
        }
        ExprKind::PtrFromUInt(operand) => {
            out.push_str(&format!("{pad}PtrFromUInt : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::PtrToUInt(operand) => {
            out.push_str(&format!("{pad}PtrToUInt : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::PtrCast(operand) => {
            out.push_str(&format!("{pad}PtrCast : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::PtrLoad { pointer, offset } => {
            out.push_str(&format!("{pad}PtrLoad : {ty}\n"));
            dump_expr(module, locals, pointer, indent + 1, out);
            if let Some(offset) = offset {
                dump_expr(module, locals, offset, indent + 1, out);
            }
        }
        ExprKind::PtrStore {
            pointer,
            offset,
            value,
        } => {
            out.push_str(&format!("{pad}PtrStore : {ty}\n"));
            dump_expr(module, locals, pointer, indent + 1, out);
            if let Some(offset) = offset {
                dump_expr(module, locals, offset, indent + 1, out);
            }
            dump_expr(module, locals, value, indent + 1, out);
        }
        ExprKind::PtrOffset {
            pointer,
            offset,
            subtract,
        } => {
            out.push_str(&format!("{pad}PtrOffset subtract={subtract} : {ty}\n"));
            dump_expr(module, locals, pointer, indent + 1, out);
            dump_expr(module, locals, offset, indent + 1, out);
        }
        ExprKind::AddressOf(Place::Local(local)) => {
            out.push_str(&format!("{pad}AddressOf {} : {ty}\n", locals[*local].name));
        }
        ExprKind::AddressOf(Place::Global(global)) => out.push_str(&format!(
            "{pad}AddressOf global {} : {ty}\n",
            module.globals[*global].name
        )),
        ExprKind::SizeOf(value_ty) => out.push_str(&format!(
            "{pad}SizeOf {} : {ty}\n",
            type_name(module, *value_ty)
        )),
        ExprKind::AlignOf(value_ty) => out.push_str(&format!(
            "{pad}AlignOf {} : {ty}\n",
            type_name(module, *value_ty)
        )),
        ExprKind::FunPtrNull => out.push_str(&format!("{pad}FunPtrNull : {ty}\n")),
        ExprKind::FunctionAddress(function) => out.push_str(&format!(
            "{pad}FunctionAddress {} : {ty}\n",
            module.functions[*function].name
        )),
        ExprKind::ForeignCallbackRegister {
            registration,
            closure,
        } => {
            let registration_id = *registration;
            let registration = &module.foreign_callback_registrations[registration_id];
            out.push_str(&format!(
                "{pad}ForeignCallbackRegister registration{} native=function_type{} managed=function_type{} context={} mode={:?} : {ty}\n",
                registration_id.into_raw(),
                registration.native_function_type.into_raw(),
                registration.managed_function_type.into_raw(),
                registration.context_index,
                registration.mode,
            ));
            dump_expr(module, locals, closure, indent + 1, out);
        }
        ExprKind::ForeignCallbackOperation {
            operation,
            callback,
        } => {
            out.push_str(&format!("{pad}ForeignCallback{operation:?} : {ty}\n"));
            dump_expr(module, locals, callback, indent + 1, out);
        }
        ExprKind::FieldAccess { receiver, field } => {
            let field = match field {
                FieldRef::StructField { index, .. } => format!("field {index}"),
                FieldRef::TupleIndex(index) => format!("_{}", index + 1),
                FieldRef::ClassField { index, .. } => format!("class field {index}"),
            };
            out.push_str(&format!("{pad}FieldAccess {field} : {ty}\n"));
            dump_expr(module, locals, receiver, indent + 1, out);
        }
        ExprKind::Call { callee, args } => {
            let (function, type_args) = callable_dump_parts(module, *callee);
            let callee = &module.functions[function];
            let type_args = if type_args.is_empty() {
                String::new()
            } else {
                let args: Vec<String> = type_args.iter().map(|t| type_name(module, *t)).collect();
                format!("<{}>", args.join(", "))
            };
            out.push_str(&format!("{pad}Call {}{type_args} : {ty}\n", callee.name));
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::LocalFunctionCall {
            local_function,
            callee,
            captures,
            args,
        } => {
            let (function, type_args) = callable_dump_parts(module, *callee);
            let type_args = if type_args.is_empty() {
                String::new()
            } else {
                let args: Vec<String> = type_args.iter().map(|t| type_name(module, *t)).collect();
                format!("<{}>", args.join(", "))
            };
            out.push_str(&format!(
                "{pad}LocalFunctionCall local{} {}{type_args} captures={} : {ty}\n",
                local_function.into_raw(),
                module.functions[function].name,
                captures.len()
            ));
            for capture in captures {
                dump_expr(module, locals, capture, indent + 1, out);
            }
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::CallableCall {
            callee,
            function_type,
            args,
        } => {
            out.push_str(&format!(
                "{pad}CallableCall function_type{} : {ty}\n",
                function_type.into_raw()
            ));
            dump_expr(module, locals, callee, indent + 1, out);
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::Binary { op, lhs, rhs } => {
            out.push_str(&format!("{pad}Binary {op:?} : {ty}\n"));
            dump_expr(module, locals, lhs, indent + 1, out);
            dump_expr(module, locals, rhs, indent + 1, out);
        }
        ExprKind::Unary { op, operand } => {
            out.push_str(&format!("{pad}Unary {op:?} : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::MethodCall {
            receiver,
            callee,
            args,
        } => {
            let function = method_callee_function(module, *callee);
            let target = match callee {
                MethodCallee::Callable(_) => module.functions[function].name.clone(),
                MethodCallee::Bound(bound) => {
                    let bound = &module.bound_callable_refs[*bound];
                    let interface = &module.interface_applications[bound.bound];
                    format!(
                        "bound T{} via {} -> {}",
                        bound.receiver_parameter.into_raw(),
                        type_name(module, interface.canonical_type),
                        module.functions[function].name
                    )
                }
                MethodCallee::DerivedEquality(_) => {
                    format!("{} <derived>", module.functions[function].name)
                }
            };
            out.push_str(&format!("{pad}MethodCall {} : {ty}\n", target));
            dump_expr(module, locals, receiver, indent + 1, out);
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::Box(operand) => {
            out.push_str(&format!("{pad}Box : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::Unbox(operand) => {
            out.push_str(&format!("{pad}Unbox : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::IsInstance { operand, check_ty } => {
            out.push_str(&format!(
                "{pad}IsInstance {} : {ty}\n",
                type_name(module, *check_ty)
            ));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::Cast { operand, optional } => {
            out.push_str(&format!("{pad}Cast optional={optional} : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::ArrayLiteral(elements) => {
            out.push_str(&format!("{pad}ArrayLiteral : {ty}\n"));
            for element in elements {
                dump_expr(module, locals, element, indent + 1, out);
            }
        }
        ExprKind::Index { receiver, index } => {
            out.push_str(&format!("{pad}Index : {ty}\n"));
            dump_expr(module, locals, receiver, indent + 1, out);
            dump_expr(module, locals, index, indent + 1, out);
        }
        ExprKind::ArrayLen(operand) => {
            out.push_str(&format!("{pad}ArrayLen : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::ArrayClone(operand) => {
            out.push_str(&format!("{pad}ArrayClone : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::SomeWrap(operand) => {
            out.push_str(&format!("{pad}SomeWrap : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::NoneLiteral => out.push_str(&format!("{pad}NoneLiteral : {ty}\n")),
        ExprKind::IsSome(operand) => {
            out.push_str(&format!("{pad}IsSome : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::Unwrap {
            operand,
            trap_on_none,
        } => {
            out.push_str(&format!("{pad}Unwrap trap={trap_on_none} : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
    }
}
