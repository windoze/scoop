use super::*;

mod expr;
use expr::dump_expr;

fn generic_delegate_name(module: &Module, reference: &GenericDelegateReference) -> String {
    let name = match reference.template {
        GenericDelegateTemplateSource::Defined(template) => {
            let property = module.generic_delegate_templates[template].property;
            &module.properties[property].name
        }
        GenericDelegateTemplateSource::Imported(template) => {
            &module.imported_generic_delegate_templates[template].diagnostic_path
        }
    };
    let arguments = reference
        .arguments
        .iter()
        .map(|ty| type_name(module, *ty))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{name}<{arguments}>")
}

fn initializing_field_name(module: &Module, field: InitializingClassFieldRef) -> &str {
    match field {
        InitializingClassFieldRef::Declared { field, .. } => {
            &module.properties[module.class_fields[field].property].name
        }
        InitializingClassFieldRef::Imported { owner, field } => {
            let Type::ImportedClass(class) = &module.types[owner] else {
                unreachable!("an imported initializing field has a class owner")
            };
            &class
                .fields
                .iter()
                .find(|candidate| candidate.identity == field)
                .expect("a resolved initializing field belongs to its declaring class")
                .name
        }
    }
}

pub(super) fn dump_statements(
    module: &Module,
    locals: &Arena<Local>,
    statements: &[Statement],
    indent: usize,
    out: &mut String,
) {
    for statement in statements {
        let pad = "  ".repeat(indent);
        match &statement.kind {
            StatementKind::GenericDelegateEnsure(reference) => out.push_str(&format!(
                "{pad}ensure delegate {}\n",
                generic_delegate_name(module, reference)
            )),
            StatementKind::Expr(expr) => dump_expr(module, locals, expr, indent, out),
            StatementKind::InitializationEnsure(unit) => out.push_str(&format!(
                "{pad}ensure init{} {}\n",
                unit.into_raw(),
                module.initialization_units[*unit].display_name
            )),
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
                    AssignTarget::GenericDelegateStorage(reference) => out.push_str(&format!(
                        "{pad}assign delegate {}\n",
                        generic_delegate_name(module, reference)
                    )),
                    AssignTarget::Local(local) => {
                        out.push_str(&format!("{pad}assign {}\n", locals[*local].name))
                    }
                    AssignTarget::Global(global) => out.push_str(&format!(
                        "{pad}assign global {}\n",
                        module.globals[*global].name
                    )),
                    AssignTarget::SingletonPublishedRoot(root) => out.push_str(&format!(
                        "{pad}assign singleton-root#{}\n",
                        root.into_raw().into_u32()
                    )),
                    AssignTarget::Field { receiver, .. } => {
                        out.push_str(&format!("{pad}assign .field\n"));
                        dump_expr(module, locals, receiver, indent + 1, out);
                    }
                    AssignTarget::InitializingClassField { field, .. } => out.push_str(&format!(
                        "{pad}assign initializing .{}\n",
                        initializing_field_name(module, *field)
                    )),
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
            StatementKind::While {
                target: _,
                condition_setup,
                cond,
                body,
            } => {
                out.push_str(&format!("{pad}while\n"));
                if !condition_setup.is_empty() {
                    out.push_str(&format!("{pad}  condition setup\n"));
                    dump_statements(module, locals, condition_setup, indent + 2, out);
                }
                dump_expr(module, locals, cond, indent + 1, out);
                dump_statements(module, locals, body, indent + 1, out);
            }
            StatementKind::For(plan) => {
                out.push_str(&format!("{pad}for loop{}\n", plan.target().into_raw()));
                if !plan.source_setup().is_empty() {
                    out.push_str(&format!("{pad}  source setup\n"));
                    dump_statements(module, locals, plan.source_setup(), indent + 2, out);
                }
                out.push_str(&format!(
                    "{pad}  source {}\n",
                    dump_binding_temporary(module, locals, plan.source())
                ));
                dump_expr(module, locals, plan.source_init(), indent + 2, out);
                if !plan.iterator_setup().is_empty() {
                    out.push_str(&format!("{pad}  iterator setup\n"));
                    dump_statements(module, locals, plan.iterator_setup(), indent + 2, out);
                }
                let conformance = plan.conformance();
                out.push_str(&format!(
                    "{pad}  iterator call -> {}\n",
                    dump_binding_temporary(module, locals, conformance.source())
                ));
                dump_expr(module, locals, plan.iterator_call(), indent + 2, out);
                out.push_str(&format!(
                    "{pad}  conformance {} as {}\n",
                    dump_binding_temporary(module, locals, conformance.source()),
                    dump_binding_temporary(module, locals, conformance.iterator())
                ));

                let next = plan.next();
                let option_core = next.option();
                let next_application = &module.method_applications[next.callable()];
                let option_application = &module.enum_applications[option_core.application()];
                let option = &module.enums[option_application.template];
                let some =
                    &option.variants[option_core.some_payload().variant().local_index() as usize];
                let none = &option.variants[option_core.none().local_index() as usize];
                out.push_str(&format!(
                    "{pad}  next {} -> {} [{}.{}/{}; {}.{}]\n",
                    module.functions[next_application.function].name,
                    dump_binding_temporary(module, locals, next.result()),
                    option.name,
                    some.name,
                    some.fields[option_core.some_payload().local_index() as usize].name,
                    option.name,
                    none.name,
                ));
                out.push_str(&format!(
                    "{pad}  element {}\n",
                    dump_binding_temporary(module, locals, next.element())
                ));
                let binding = plan.binding();
                out.push_str(&format!(
                    "{pad}  binding {}\n",
                    dump_binding_shape(module, locals, &binding.shape)
                ));
                for action in &binding.actions {
                    match action {
                        IrrefutableBindingAction::Project {
                            source,
                            result,
                            projection,
                            ..
                        } => {
                            let projection = match projection {
                                BindingProjection::TupleIndex(index) => format!("tuple#{index}"),
                                BindingProjection::StructField(field) => {
                                    format!("field#{}", field.local_index())
                                }
                            };
                            out.push_str(&format!(
                                "{pad}    project {} {projection} -> {}\n",
                                dump_binding_temporary(module, locals, *source),
                                dump_binding_temporary(module, locals, *result),
                            ));
                        }
                        IrrefutableBindingAction::Component {
                            source,
                            index,
                            result,
                            setup,
                            call,
                            ..
                        } => {
                            out.push_str(&format!(
                                "{pad}    component{} {} -> {}\n",
                                index.get(),
                                dump_binding_temporary(module, locals, *source),
                                dump_binding_temporary(module, locals, *result),
                            ));
                            if !setup.is_empty() {
                                out.push_str(&format!("{pad}      setup\n"));
                                dump_statements(module, locals, setup, indent + 4, out);
                            }
                            out.push_str(&format!("{pad}      call\n"));
                            dump_expr(module, locals, call, indent + 4, out);
                        }
                        IrrefutableBindingAction::Bind { source, target, .. } => {
                            out.push_str(&format!(
                                "{pad}    bind {} -> {}{}: {}\n",
                                dump_binding_temporary(module, locals, *source),
                                if target.mutability.is_mutable() {
                                    "var "
                                } else {
                                    ""
                                },
                                locals[target.local].name,
                                type_name(module, target.ty),
                            ));
                        }
                    }
                }
                out.push_str(&format!("{pad}  body\n"));
                dump_statements(module, locals, plan.body(), indent + 2, out);
            }
            StatementKind::Break { target } => {
                out.push_str(&format!("{pad}break loop{}\n", target.into_raw()));
            }
            StatementKind::Continue { target } => {
                out.push_str(&format!("{pad}continue loop{}\n", target.into_raw()));
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
                    if let Some(guard) = &arm.guard {
                        if !guard.setup.is_empty() {
                            out.push_str(&format!("{}    guard setup\n", pad));
                            dump_statements(module, locals, &guard.setup, indent + 3, out);
                        }
                        out.push_str(&format!("{}    guard condition\n", pad));
                        dump_expr(module, locals, &guard.condition, indent + 3, out);
                    }
                    dump_statements(module, locals, &arm.body, indent + 2, out);
                }
                match &when.fallback {
                    WhenFallback::Else(body) => {
                        out.push_str(&format!("{pad}  else\n"));
                        dump_statements(module, locals, body, indent + 2, out);
                    }
                    WhenFallback::Impossible(ExhaustivenessProof::IrrefutableArm {
                        subject_ty,
                    }) => out.push_str(&format!(
                        "{pad}  impossible <irrefutable {}>\n",
                        type_name(module, *subject_ty),
                    )),
                    WhenFallback::Impossible(ExhaustivenessProof::PatternMatrix { subject_ty }) => {
                        out.push_str(&format!(
                            "{pad}  impossible <pattern matrix for {}>\n",
                            type_name(module, *subject_ty),
                        ))
                    }
                    WhenFallback::Impossible(ExhaustivenessProof::EnumPatternMatrix {
                        subject_ty,
                        ..
                    }) => out.push_str(&format!(
                        "{pad}  impossible <enum pattern matrix for {}>\n",
                        type_name(module, *subject_ty),
                    )),
                }
            }
        }
    }
}

fn dump_binding_temporary(
    module: &Module,
    locals: &Arena<Local>,
    temporary: BindingTemporary,
) -> String {
    format!(
        "{}: {}",
        locals[temporary.local].name,
        type_name(module, temporary.ty)
    )
}

fn dump_binding_shape(
    module: &Module,
    locals: &Arena<Local>,
    shape: &IrrefutableBindingShape,
) -> String {
    match shape {
        IrrefutableBindingShape::Binding(binding) => format!(
            "{}{}: {}",
            if binding.mutability.is_mutable() {
                "var "
            } else {
                ""
            },
            locals[binding.local].name,
            type_name(module, binding.ty),
        ),
        IrrefutableBindingShape::Wildcard => "_".to_string(),
        IrrefutableBindingShape::Tuple(elements) => format!(
            "({})",
            elements
                .iter()
                .map(|element| dump_binding_shape(module, locals, element))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        IrrefutableBindingShape::Struct {
            application,
            fields,
        } => format!(
            "{} {{{}}}",
            type_name(
                module,
                module.struct_applications[*application].canonical_type
            ),
            fields
                .iter()
                .map(|(field, shape)| format!(
                    "#{}: {}",
                    field.local_index(),
                    dump_binding_shape(module, locals, shape)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        IrrefutableBindingShape::Class {
            application,
            components,
        } => format!(
            "{}({})",
            type_name(
                module,
                module.class_applications[*application].canonical_type
            ),
            components
                .iter()
                .map(|(index, shape)| format!(
                    "#{}: {}",
                    index.get(),
                    dump_binding_shape(module, locals, shape)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
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
        Pattern::ImportedVariant {
            variant, fields, ..
        } => {
            let fields = fields
                .iter()
                .map(|(index, pattern)| format!("{index}: {}", dump_pattern(pattern)))
                .collect::<Vec<_>>();
            format!("dependency variant {variant}({})", fields.join(", "))
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
        MethodOwnerApplication::Object(_) => &[],
    }
}

pub(super) fn generic_method_owner_arguments(
    module: &Module,
    owner: GenericMethodOwner,
) -> &[TypeId] {
    match owner {
        GenericMethodOwner::Class(id) => &module.class_applications[id].arguments,
        GenericMethodOwner::Struct(id) => &module.struct_applications[id].arguments,
        GenericMethodOwner::Enum(id) => &module.enum_applications[id].arguments,
        GenericMethodOwner::Object(_) => &[],
    }
}
