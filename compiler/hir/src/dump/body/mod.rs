use super::*;

mod expr;
use expr::dump_expr;

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
            StatementKind::Expr(expr) => dump_expr(module, locals, expr, indent, out),
            StatementKind::InitializationEnsure(unit) => out.push_str(&format!(
                "{pad}ensure init{} {}\n",
                unit.into_raw(),
                module.initialization_units[*unit].stable_key
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
                    AssignTarget::InitializingClassField { field, .. } => out.push_str(&format!(
                        "{pad}assign initializing .{}\n",
                        module.properties[module.class_fields[*field].property].name
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
