use super::body::{dump_statements, generic_method_owner_arguments};
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
