use super::body::{dump_statements, generic_method_owner_arguments};
use super::*;

/// Indented text dump for golden tests (`scoopc build --emit=hir`).
pub fn dump(module: &Module) -> String {
    let mut out = String::from("Module\n");
    let object_backings = module
        .objects
        .iter()
        .map(|(_, declaration)| declaration.backing_class)
        .collect::<std::collections::HashSet<_>>();
    for (id, decl) in module.structs.iter() {
        if id == module.ffi_core.ptr
            || id == module.ffi_core.fun_ptr
            || id == module.ffi_core.pinned_ptr
            || id == module.ffi_core.gc_handle
            || id == module.foreign_callback_core.callback
            || id == module.source_location_core.location
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
            nominal_declaration_name(module, &decl.name, decl.owner),
            type_params,
            interfaces,
            attributes
        ));
        for (index, field) in decl.semantic_fields().iter().enumerate() {
            out.push_str(&format!(
                "    field{index} {}: {}\n",
                field.name,
                type_name(module, field.ty)
            ));
        }
        for &property in &decl.properties {
            dump_property(module, property, 2, &mut out);
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
            nominal_declaration_name(module, &decl.name, decl.owner),
            type_params,
            interfaces,
            attributes
        ));
        for variant in &decl.variants {
            let fields: Vec<String> = variant
                .fields
                .iter()
                .map(|f| format!("{}: {}", f.name, type_name(module, f.ty)))
                .collect();
            out.push_str(&format!("    {}({})\n", variant.name, fields.join(", ")));
        }
        for &property in &decl.properties {
            dump_property(module, property, 2, &mut out);
        }
    }
    for (class, decl) in module.classes.iter() {
        if object_backings.contains(&class) {
            continue;
        }
        if matches!(decl.representation, ClassRepresentation::Intrinsic(_)) {
            continue;
        }
        let modifier = match decl.modifier {
            ClassModifier::Final => "",
            ClassModifier::Open => "open ",
            ClassModifier::Abstract => "abstract ",
        };
        let ctor: Vec<String> = decl
            .constructors
            .iter()
            .map(|constructor| &module.class_constructors[*constructor])
            .find(|constructor| matches!(constructor.kind, ClassConstructorKind::Primary { .. }))
            .map(|constructor| {
                constructor
                    .parameters
                    .iter()
                    .map(|parameter| {
                        format!("{}: {}", parameter.name, type_name(module, parameter.ty))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let type_params = if decl.type_params.is_empty() {
            String::new()
        } else {
            dump_type_params(module, &decl.type_params)
        };
        let interfaces = dump_interface_list(module, &decl.interfaces);
        out.push_str(&format!(
            "  {modifier}class {}{}({}){}\n",
            nominal_declaration_name(module, &decl.name, decl.owner),
            type_params,
            ctor.join(", "),
            interfaces
        ));
        for &field in &decl.fields {
            let physical = &module.class_fields[field];
            out.push_str(&format!(
                "    field{} property{}: {}\n",
                field.into_raw(),
                physical.property.into_raw(),
                type_name(module, physical.ty)
            ));
        }
        for &property in &decl.properties {
            dump_property(module, property, 2, &mut out);
        }
    }
    for (object, declaration) in module.objects.iter() {
        let object_type = &module.object_types[declaration.object_type];
        let singleton = &module.singleton_values[declaration.singleton_value];
        let backing = &module.classes[declaration.backing_class];
        let mut supertypes = Vec::new();
        if let Some(base) = backing.base_class {
            supertypes.push(type_name(module, base));
        }
        supertypes.extend(
            backing
                .interfaces
                .iter()
                .map(|interface| type_name(module, *interface)),
        );
        let supertypes = if supertypes.is_empty() {
            String::new()
        } else {
            format!(" : {}", supertypes.join(", "))
        };
        out.push_str(&format!(
            "  {} {}{} <object{} type{} value{} root{} init{}{}>\n",
            match declaration.kind {
                ObjectKind::Standalone => "object",
                ObjectKind::Companion(_) => "companion object",
            },
            nominal_declaration_name(module, &declaration.name, declaration.owner),
            supertypes,
            object.into_raw(),
            declaration.object_type.into_raw(),
            declaration.singleton_value.into_raw(),
            singleton.published_root.into_raw(),
            singleton.initialization.into_raw(),
            match declaration.kind {
                ObjectKind::Standalone => String::new(),
                ObjectKind::Companion(relation) => {
                    format!(" companion{}", relation.into_raw())
                }
            },
        ));
        debug_assert_eq!(object_type.declaration, object);
        for &field in &backing.fields {
            let physical = &module.class_fields[field];
            out.push_str(&format!(
                "    field{} property{}: {}\n",
                field.into_raw(),
                physical.property.into_raw(),
                type_name(module, physical.ty)
            ));
        }
        for &property in &backing.properties {
            dump_property(module, property, 2, &mut out);
        }
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
            nominal_declaration_name(module, &decl.name, decl.owner),
            type_params,
            parents
        ));
        for method in &decl.methods {
            let member = &module.interface_methods[*method];
            let function = &module.functions[member.function];
            let params: Vec<String> = function
                .params
                .iter()
                .skip(1)
                .map(|param| format!("{}: {}", param.name, type_name(module, param.ty)))
                .collect();
            let implementation = match member.implementation {
                InterfaceMemberImplementation::Body => " <default>",
                InterfaceMemberImplementation::AbstractSlot => "",
            };
            out.push_str(&format!(
                "    {}{}fun {}({}): {}{}{}\n",
                if function.modifiers.operator.is_some()
                    || function.modifiers.property_delegate_operator.is_some()
                {
                    "operator "
                } else {
                    ""
                },
                if function.is_suspend { "suspend " } else { "" },
                function.name.rsplit('.').next().unwrap_or(&function.name),
                params.join(", "),
                type_name(module, function.return_ty),
                dump_function_attributes(function.attributes),
                implementation,
            ));
            if member.implementation == InterfaceMemberImplementation::Body
                && let FunctionKind::User(body) = &function.kind
            {
                dump_statements(module, &body.locals, &body.statements, 3, &mut out);
            }
        }
        for &method in &decl.private_methods {
            let function = &module.functions[method];
            let params = function
                .params
                .iter()
                .skip(1)
                .map(|param| format!("{}: {}", param.name, type_name(module, param.ty)))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!(
                "    private fun {}({params}): {}{} <helper>\n",
                function.name.rsplit('.').next().unwrap_or(&function.name),
                type_name(module, function.return_ty),
                dump_function_attributes(function.attributes),
            ));
            if let FunctionKind::User(body) = &function.kind {
                dump_statements(module, &body.locals, &body.statements, 3, &mut out);
            }
        }
        for &property in &decl.properties {
            dump_property(module, property, 2, &mut out);
        }
    }
    for (property, declaration) in module.properties.iter() {
        if matches!(
            declaration.owner,
            PropertyOwner::TopLevel | PropertyOwner::Extension(_)
        ) {
            dump_property(module, property, 1, &mut out);
        }
    }
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
            if module.properties[global.property]
                .capability
                .setter()
                .is_some()
            {
                "var"
            } else {
                "val"
            },
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
            module.source_location_core.current,
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
        let operator = if function.modifiers.operator.is_some()
            || function.modifiers.property_delegate_operator.is_some()
        {
            "operator "
        } else {
            ""
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

fn dump_property(module: &Module, id: PropertyId, indent: usize, out: &mut String) {
    let property = &module.properties[id];
    let modifier = match property.modifier {
        MethodModifier::Final => "",
        MethodModifier::Open => "open ",
        MethodModifier::Abstract => "abstract ",
    };
    let override_ = if property.is_override {
        "override "
    } else {
        ""
    };
    let mutability = if property.capability.setter().is_some() {
        "var"
    } else {
        "val"
    };
    let getter = property.capability.getter();
    let getter = format!(
        "getter{}={}",
        getter.into_raw(),
        dump_accessor_implementation(module, module.property_getters[getter].implementation)
    );
    let setter = property
        .capability
        .setter()
        .map_or_else(String::new, |setter| {
            format!(
                " setter{}={}",
                setter.into_raw(),
                dump_accessor_implementation(
                    module,
                    module.property_setters[setter].implementation
                )
            )
        });
    let representation = match &property.representation {
        PropertyRepresentation::Stored(stored) => match stored.backing {
            PropertyBacking::TopLevelGlobal {
                storage,
                initialization,
            } => {
                let initialization = match initialization {
                    TopLevelInitialization::Image => "image".to_string(),
                    TopLevelInitialization::Runtime(unit) => {
                        format!("init{}", unit.into_raw())
                    }
                };
                format!("stored global{} {initialization}", storage.into_raw())
            }
            PropertyBacking::ClassField { field, initializer } => format!(
                "stored field{} init={}",
                field.into_raw(),
                match initializer {
                    ClassPropertyInitializer::PrimaryParameter(parameter) => {
                        format!("parameter{}", parameter.into_raw())
                    }
                    ClassPropertyInitializer::Expression => "expression".to_string(),
                    ClassPropertyInitializer::SyntheticNone => "synthetic-none".to_string(),
                }
            ),
            PropertyBacking::StructField { owner, index } => {
                format!("stored struct{}-field{index}", owner.into_raw())
            }
        },
        PropertyRepresentation::AccessorOnly => "accessor-only".to_string(),
        PropertyRepresentation::Delegated { storage } => {
            let delegate = &module.delegate_storages[*storage];
            let location = match delegate.location {
                DelegateStorageLocation::ClassField(field) => {
                    format!("class-field{}", field.into_raw())
                }
            };
            format!(
                "delegated storage{} type={} location={location}",
                storage.into_raw(),
                type_name(module, delegate.ty)
            )
        }
        PropertyRepresentation::Const { value } => format!("const {value:?}"),
        PropertyRepresentation::NativeStorage { storage } => {
            format!("native-storage global{}", storage.into_raw())
        }
    };
    let overrides = if property.overrides.is_empty() {
        String::new()
    } else {
        format!(
            " overrides=[{}]",
            property
                .overrides
                .iter()
                .map(|property| property.into_raw().to_string())
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    let (type_params, receiver, property_ty) = match property.owner {
        PropertyOwner::Extension(extension) => {
            let extension = &module.extension_properties[extension];
            let params = if extension.type_params.is_empty() {
                String::new()
            } else {
                format!("{} ", dump_type_params(module, &extension.type_params))
            };
            (
                params,
                format!(
                    "{}.",
                    type_name_with_params(module, extension.receiver_ty, &extension.type_params)
                ),
                type_name_with_params(module, property.ty, &extension.type_params),
            )
        }
        _ => (String::new(), String::new(), type_name(module, property.ty)),
    };
    out.push_str(&format!(
        "{}property{} {modifier}{override_}{mutability} {type_params}{receiver}{}: {property_ty} {getter}{setter} <{representation}>{overrides}\n",
        "  ".repeat(indent),
        id.into_raw(),
        property.name,
    ));
}

fn dump_accessor_implementation(
    module: &Module,
    implementation: PropertyAccessorImplementation,
) -> String {
    match implementation {
        PropertyAccessorImplementation::Storage => "storage".to_string(),
        PropertyAccessorImplementation::Constant => "constant".to_string(),
        PropertyAccessorImplementation::Body(function) => {
            format!("body({})", module.functions[function].name)
        }
        PropertyAccessorImplementation::AbstractSlot(function) => {
            format!("abstract({})", module.functions[function].name)
        }
    }
}

fn dump_type_params(module: &Module, params: &[TypeParamDecl]) -> String {
    let params = params
        .iter()
        .map(|param| {
            let bounds = match &param.bounds {
                TypeParamBounds::Unconstrained => String::new(),
                TypeParamBounds::Value { .. } => " : value".to_string(),
                TypeParamBounds::Ref { .. } => " : ref".to_string(),
                TypeParamBounds::Nominal(bounds) => {
                    let mut rendered = Vec::new();
                    if let Some(bound) = &bounds.class {
                        let application = &module.class_applications[bound.application];
                        let name = &module.classes[application.template].name;
                        let value = if application.arguments.is_empty() {
                            name.clone()
                        } else {
                            let arguments = application
                                .arguments
                                .iter()
                                .map(|ty| type_name_with_params(module, *ty, params))
                                .collect::<Vec<_>>()
                                .join(", ");
                            format!("{name}<{arguments}>")
                        };
                        rendered.push((bound.span.start, value));
                    }
                    rendered.extend(bounds.interfaces.iter().map(|bound| {
                        let application = &module.interface_applications[bound.application];
                        let name = &module.interfaces[application.template].name;
                        let value = if application.arguments.is_empty() {
                            name.clone()
                        } else {
                            let arguments = application
                                .arguments
                                .iter()
                                .map(|ty| type_name_with_params(module, *ty, params))
                                .collect::<Vec<_>>()
                                .join(", ");
                            format!("{name}<{arguments}>")
                        };
                        (bound.span.start, value)
                    }));
                    rendered.sort_by_key(|(start, _)| *start);
                    format!(
                        " : {}",
                        rendered
                            .into_iter()
                            .map(|(_, value)| value)
                            .collect::<Vec<_>>()
                            .join(" & ")
                    )
                }
            };
            format!("{}{bounds}", param.name)
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
