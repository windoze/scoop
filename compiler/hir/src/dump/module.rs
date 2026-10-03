use super::body::{dump_statements, generic_method_owner_arguments};
use super::*;

mod functions;
mod properties;
use properties::dump_property;

/// Indented text dump for golden tests (`scoopc build --emit=hir`).
pub fn dump(output: &ExportHirOutput) -> String {
    dump_with(output.module(), |module, out| match output.output_kind() {
        ConeOutputKind::Library => out.push_str("  output library\n"),
        ConeOutputKind::Executable { local_entry } => out.push_str(&format!(
            "  output executable {}\n",
            module.functions[local_entry.local_function().function()].name
        )),
    })
}

/// Dump a raw Export HIR module for tests which intentionally inspect an
/// intermediate graph before output-kind selection.
pub fn dump_module(module: &Module) -> String {
    dump_with(module, |_, _| {})
}

fn dump_with(module: &Module, write_entry: impl FnOnce(&Module, &mut String)) -> String {
    let mut out = String::from("Module\n");
    let defined_core = match &module.core_protocols {
        CoreProtocols::Defined(protocols) => Some(protocols),
        CoreProtocols::Imported(_) => None,
    };
    let object_backings = module
        .objects
        .iter()
        .map(|(_, declaration)| declaration.backing_class)
        .collect::<std::collections::HashSet<_>>();
    for (_, alias) in module.type_aliases.iter() {
        out.push_str(&format!(
            "  typealias {} = {}\n",
            alias.name,
            type_name(module, alias.target)
        ));
    }
    for (id, decl) in module.structs.iter() {
        if defined_core.is_some_and(|core| {
            id == core.ffi.ptr
                || id == core.ffi.fun_ptr
                || id == core.ffi.pinned_ptr
                || id == core.ffi.gc_handle
                || id == core.foreign_callbacks.callback
                || id == core.source_location.location
        }) {
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
        if defined_core.is_some_and(|core| {
            id == core.foreign_callbacks.modes.enumeration()
                || id == core.foreign_callbacks.states.enumeration()
        }) {
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
                type_name(module, module.class_field_definition(field).ty)
            ));
        }
        for &property in &decl.properties {
            dump_property(module, property, 2, &mut out);
        }
        if let ReleasePolicy::SynchronousGcFree {
            hook: ExportReleaseHookRef::Template(hook),
        } = decl.release_policy
        {
            let hook = &module.release_hooks[hook];
            let requirements = hook
                .requirements
                .iter()
                .map(|id| {
                    decl.type_params
                        .iter()
                        .find(|parameter| parameter.id == *id)
                        .expect("release requirements bind owner parameters")
                        .name
                        .as_str()
                })
                .collect::<Vec<_>>();
            out.push_str(&format!(
                "    release <requires-release-value [{}]>\n",
                requirements.join(", ")
            ));
            dump_statements(
                module,
                &hook.body.locals,
                &hook.body.statements,
                3,
                &mut out,
            );
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
                type_name(module, module.class_field_definition(field).ty)
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
                    .map(|parent| type_name(module, *parent))
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
            if global.mutable { "var" } else { "val" },
            global.name,
            type_name(module, global.ty),
            id.into_raw()
        ));
    }
    for &id in &module.top_level {
        if defined_core.is_some_and(|core| {
            [
                core.ffi.address_of,
                core.ffi.size_of,
                core.ffi.align_of,
                core.ffi.gc_pin_raw,
                core.ffi.gc_unpin_raw,
                core.ffi.gc_get_handle_raw,
                core.ffi.gc_release_handle_raw,
                core.foreign_callbacks.register,
                core.foreign_callbacks.retain,
                core.foreign_callbacks.release,
                core.foreign_callbacks.query_state,
                core.foreign_callbacks.failure,
                core.source_location.current,
            ]
            .contains(&id)
        }) {
            continue;
        }
        functions::dump_function(module, &module.functions[id], &mut out);
    }
    write_entry(module, &mut out);
    for (_, instantiation) in module.instantiations.iter() {
        let function = module.generic_functions[instantiation.generic].function;
        if defined_core.is_some_and(|core| {
            [
                core.ffi.gc_pin_raw,
                core.ffi.gc_unpin_raw,
                core.ffi.gc_get_handle_raw,
                core.ffi.gc_release_handle_raw,
            ]
            .contains(&function)
        }) {
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
            let bounds = match &param.bounds {
                TypeParamBounds::Unconstrained => String::new(),
                TypeParamBounds::Value { .. } => " : value".to_string(),
                TypeParamBounds::Ref { .. } => " : ref".to_string(),
                TypeParamBounds::Nominal(bounds) => {
                    let rendered = bounds
                        .in_source_order()
                        .into_iter()
                        .map(|bound| {
                            let ty = bound.ty();
                            type_name_with_params(module, ty, params)
                        })
                        .collect::<Vec<_>>();
                    format!(" : {}", rendered.join(" & "))
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
            dump_c_layout_value(layout.aligned),
            dump_c_layout_value(layout.packed)
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

fn dump_c_layout_value(value: HirCLayoutValue) -> u8 {
    value.bytes().unwrap_or(0)
}

fn dump_interface_list(module: &Module, interfaces: &[TypeId]) -> String {
    if interfaces.is_empty() {
        String::new()
    } else {
        let names: Vec<String> = interfaces.iter().map(|&ty| type_name(module, ty)).collect();
        format!(" : {}", names.join(", "))
    }
}
