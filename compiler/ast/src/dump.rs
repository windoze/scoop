use crate::*;

/// Indented text dump for golden tests (`scoopc build --emit=ast`).
pub fn dump(file: &SourceFile) -> String {
    let mut out = String::from("SourceFile\n");
    for decl in &file.declarations {
        match decl {
            Decl::Global(g) => {
                dump_annotations(&g.annotations, 2, &mut out);
                out.push_str(&format!(
                    "  {} {}: {}{}\n",
                    if g.mutable { "var" } else { "val" },
                    g.name.text,
                    dump_type_ref(&g.ty),
                    if g.init.is_some() { " = <expr>" } else { "" }
                ));
            }
            Decl::Enum(e) => {
                dump_annotations(&e.annotations, 2, &mut out);
                let type_params = if e.type_params.is_empty() {
                    String::new()
                } else {
                    format!(
                        "<{}>",
                        e.type_params
                            .iter()
                            .map(dump_type_param)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                let interfaces = if e.interfaces.is_empty() {
                    String::new()
                } else {
                    let names: Vec<String> = e.interfaces.iter().map(dump_type_ref).collect();
                    format!(" : {}", names.join(", "))
                };
                let where_clause = dump_where_clause(e.where_clause.as_ref());
                out.push_str(&format!(
                    "  enum {}{}{}{}\n",
                    e.name.text, type_params, interfaces, where_clause
                ));
                for variant in &e.variants {
                    match &variant.kind {
                        VariantDeclKind::Unit => {
                            out.push_str(&format!("    {}\n", variant.name.text))
                        }
                        VariantDeclKind::Positional(types) => {
                            let types: Vec<String> = types.iter().map(dump_type_ref).collect();
                            out.push_str(&format!(
                                "    {}({})\n",
                                variant.name.text,
                                types.join(", ")
                            ));
                        }
                        VariantDeclKind::Named(fields) | VariantDeclKind::Constructor(fields) => {
                            let kind = if matches!(variant.kind, VariantDeclKind::Constructor(_)) {
                                "ctor"
                            } else {
                                "named"
                            };
                            out.push_str(&format!("    {} <{}>\n", variant.name.text, kind));
                            for field in fields {
                                let default = field
                                    .default
                                    .as_ref()
                                    .map(|_| " = <expr>")
                                    .unwrap_or_default();
                                out.push_str(&format!(
                                    "      {}: {}{}\n",
                                    field.name.text,
                                    dump_type_ref(&field.ty),
                                    default
                                ));
                            }
                        }
                    }
                }
            }
            Decl::Class(c) => {
                dump_annotations(&c.annotations, 2, &mut out);
                let modifier = match c.modifier {
                    ClassModifier::Final => "",
                    ClassModifier::Open => "open ",
                    ClassModifier::Abstract => "abstract ",
                };
                let type_params = dump_type_params(&c.type_params);
                let ctor = match &c.constructor {
                    ClassConstructorDecl::Omitted => "()".to_string(),
                    ClassConstructorDecl::Declared(properties) => format!(
                        "({})",
                        properties
                            .iter()
                            .map(|p| format!(
                                "{}{}: {}",
                                if p.mutable { "var " } else { "val " },
                                p.name.text,
                                dump_type_ref(&p.ty)
                            ))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                };
                let base = c
                    .base_class
                    .as_ref()
                    .map(|(ty, args)| format!(" : {}(<{} args>)", dump_type_ref(ty), args.len()))
                    .unwrap_or_default();
                let ifaces = if c.interfaces.is_empty() {
                    String::new()
                } else {
                    let names: Vec<String> = c.interfaces.iter().map(dump_type_ref).collect();
                    format!(", {}", names.join(", "))
                };
                let where_clause = dump_where_clause(c.where_clause.as_ref());
                out.push_str(&format!(
                    "  {modifier}class {}{}{}{}{}{}\n",
                    c.name.text, type_params, ctor, base, ifaces, where_clause
                ));
                for method in &c.methods {
                    let suspend = if method.is_suspend { "suspend " } else { "" };
                    let operator = if method.operator.is_some() {
                        "operator "
                    } else {
                        ""
                    };
                    out.push_str(&format!(
                        "    {operator}{suspend}fun {}\n",
                        method.name.text
                    ));
                }
            }
            Decl::Interface(i) => {
                dump_annotations(&i.annotations, 2, &mut out);
                let params = if i.type_params.is_empty() {
                    String::new()
                } else {
                    let params: Vec<String> = i.type_params.iter().map(dump_type_param).collect();
                    format!("<{}>", params.join(", "))
                };
                let parents = if i.parents.is_empty() {
                    String::new()
                } else {
                    format!(
                        " : {}",
                        i.parents
                            .iter()
                            .map(dump_type_ref)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                let where_clause = dump_where_clause(i.where_clause.as_ref());
                out.push_str(&format!(
                    "  interface {}{}{}{}\n",
                    i.name.text, params, parents, where_clause
                ));
                for method in &i.methods {
                    let suspend = if method.is_suspend { "suspend " } else { "" };
                    let operator = if method.operator.is_some() {
                        "operator "
                    } else {
                        ""
                    };
                    out.push_str(&format!(
                        "    {operator}{suspend}fun {}\n",
                        method.name.text
                    ));
                }
            }
            Decl::Struct(s) => {
                dump_annotations(&s.annotations, 2, &mut out);
                let type_params = if s.type_params.is_empty() {
                    String::new()
                } else {
                    format!(
                        "<{}>",
                        s.type_params
                            .iter()
                            .map(dump_type_param)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                let interfaces = if s.interfaces.is_empty() {
                    String::new()
                } else {
                    let names: Vec<String> = s.interfaces.iter().map(dump_type_ref).collect();
                    format!(" : {}", names.join(", "))
                };
                let representation = if s.fields.is_omitted() {
                    " <representation omitted>"
                } else {
                    ""
                };
                let where_clause = dump_where_clause(s.where_clause.as_ref());
                out.push_str(&format!(
                    "  struct {}{}{}{}{}\n",
                    s.name.text, type_params, representation, interfaces, where_clause
                ));
                for field in &s.fields {
                    out.push_str(&format!(
                        "    field {}: {}\n",
                        field.name.text,
                        dump_type_ref(&field.ty)
                    ));
                }
                for method in &s.methods {
                    let suspend = if method.is_suspend { "suspend " } else { "" };
                    let operator = if method.operator.is_some() {
                        "operator "
                    } else {
                        ""
                    };
                    out.push_str(&format!(
                        "    {operator}{suspend}fun {}\n",
                        method.name.text
                    ));
                }
            }
            Decl::Function(f) => {
                dump_annotations(&f.annotations, 2, &mut out);
                let type_params = if f.type_params.is_empty() {
                    String::new()
                } else {
                    format!(
                        "<{}>",
                        f.type_params
                            .iter()
                            .map(dump_type_param)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                let params: Vec<String> = f
                    .params
                    .iter()
                    .map(|p| format!("{}: {}", p.name.text, dump_type_ref(&p.ty)))
                    .collect();
                let ret = f
                    .return_ty
                    .as_ref()
                    .map(|t| format!(": {}", dump_type_ref(t)))
                    .unwrap_or_default();
                let modifier = match (f.modifier, f.is_override) {
                    (MethodModifier::Final, false) => "",
                    (MethodModifier::Final, true) => "final ",
                    (MethodModifier::Open, false) => "open ",
                    // `override` is open by default, so avoid printing
                    // a redundant effective-modality marker.
                    (MethodModifier::Open, true) => "",
                    (MethodModifier::Abstract, _) => "abstract ",
                };
                let flags = format!(
                    "{}{}{}{}",
                    modifier,
                    if f.is_override { "override " } else { "" },
                    if f.operator.is_some() {
                        "operator "
                    } else {
                        ""
                    },
                    if f.is_suspend { "suspend " } else { "" }
                );
                let receiver = f
                    .receiver_ty
                    .as_ref()
                    .map(|ty| format!("{}.", dump_type_ref(ty)))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "  {flags}fun {receiver}{}{}({}){}{}\n",
                    f.name.text,
                    type_params,
                    params.join(", "),
                    ret,
                    dump_where_clause(f.where_clause.as_ref())
                ));
                match &f.body {
                    FunctionBody::Block(block) => dump_block(block, 2, &mut out),
                    FunctionBody::Expr(expr) => {
                        out.push_str(
                            "    =
",
                        );
                        dump_expr(expr, 3, &mut out);
                    }
                    FunctionBody::None => {}
                }
            }
        }
    }
    out
}

fn dump_annotations(annotations: &[Annotation], indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    for annotation in annotations {
        let args = if annotation.args.is_empty() {
            String::new()
        } else {
            let args: Vec<String> = annotation
                .args
                .iter()
                .map(|arg| {
                    let name = arg
                        .name
                        .as_ref()
                        .map(|name| format!("{} = ", name.text))
                        .unwrap_or_default();
                    let value = match &arg.value {
                        AnnotationLiteral::String(value) => format!("{value:?}"),
                        AnnotationLiteral::Int(value) => value.to_string(),
                        AnnotationLiteral::Boolean(value) => value.to_string(),
                    };
                    format!("{name}{value}")
                })
                .collect();
            format!("({})", args.join(", "))
        };
        out.push_str(&format!("{pad}@{}{}\n", annotation.name.text, args));
    }
}

fn dump_type_ref(ty: &TypeRef) -> String {
    match &ty.kind {
        TypeRefKind::Named(name) => name.text.clone(),
        TypeRefKind::Generic(name, args) => {
            let inner: Vec<String> = args.iter().map(dump_type_ref).collect();
            format!("{}<{}>", name.text, inner.join(", "))
        }
        TypeRefKind::Unit => "Unit".to_string(),
        TypeRefKind::Tuple(elements) => {
            let inner: Vec<String> = elements.iter().map(dump_type_ref).collect();
            format!("({})", inner.join(", "))
        }
        TypeRefKind::Function(function) => {
            let parameters: Vec<String> = function.parameters.iter().map(dump_type_ref).collect();
            let suspend = if function.is_suspend { "suspend " } else { "" };
            format!(
                "{suspend}({}) -> {}",
                parameters.join(", "),
                dump_type_ref(&function.return_type)
            )
        }
        TypeRefKind::Nullable(inner) => match inner.kind {
            TypeRefKind::Function(_) => format!("({})?", dump_type_ref(inner)),
            _ => format!("{}?", dump_type_ref(inner)),
        },
    }
}

fn dump_block(block: &Block, indent: usize, out: &mut String) {
    for statement in &block.statements {
        dump_statement(statement, indent, out);
    }
}

fn dump_statement(statement: &Statement, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    match &statement.kind {
        StatementKind::Expr(expr) => dump_expr(expr, indent, out),
        StatementKind::LocalFunction(function) => {
            let suspend = if function.is_suspend { "suspend " } else { "" };
            let type_params = if function.type_params.is_empty() {
                String::new()
            } else {
                let names: Vec<_> = function.type_params.iter().map(dump_type_param).collect();
                format!("<{}>", names.join(", "))
            };
            let params: Vec<_> = function
                .params
                .iter()
                .map(|param| format!("{}: {}", param.name.text, dump_type_ref(&param.ty)))
                .collect();
            let return_ty = function
                .return_ty
                .as_ref()
                .map(|ty| format!(": {}", dump_type_ref(ty)))
                .unwrap_or_default();
            out.push_str(&format!(
                "{pad}{suspend}fun {}{}({}){return_ty}\n",
                function.name.text,
                type_params,
                params.join(", ")
            ));
            match &function.body {
                FunctionBody::Block(block) => dump_block(block, indent + 1, out),
                FunctionBody::Expr(expr) => dump_expr(expr, indent + 1, out),
                FunctionBody::None => {}
            }
        }
        StatementKind::Return { value } => {
            out.push_str(&format!("{pad}return\n"));
            if let Some(value) = value {
                dump_expr(value, indent + 1, out);
            }
        }
        StatementKind::ValDecl(decl) => {
            let keyword = if decl.mutable { "var" } else { "val" };
            let ty = decl.ty.as_ref().map(dump_type_ref);
            let ty = ty.map(|t| format!(": {t}")).unwrap_or_default();
            out.push_str(&format!(
                "{pad}{keyword} {}{ty}\n",
                dump_pattern(&decl.target)
            ));
            dump_expr(&decl.init, indent + 1, out);
        }
        StatementKind::When(when) => {
            out.push_str(&format!("{pad}when\n"));
            dump_expr(&when.subject, indent + 1, out);
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
                dump_block(&arm.body, indent + 2, out);
            }
            if let Some(else_body) = &when.else_body {
                out.push_str(&format!("{pad}  else\n"));
                dump_block(else_body, indent + 2, out);
            }
        }
        StatementKind::Assign(assign) => {
            match &assign.target {
                AssignTarget::Local(name) => out.push_str(&format!("{pad}assign {}\n", name.text)),
                AssignTarget::Index { .. } => out.push_str(&format!("{pad}assign []\n")),
                AssignTarget::Field { name, .. } => {
                    out.push_str(&format!("{pad}assign .{}\n", name.text))
                }
            }
            match &assign.target {
                AssignTarget::Index {
                    receiver, index, ..
                } => {
                    dump_expr(receiver, indent + 1, out);
                    dump_expr(index, indent + 1, out);
                }
                AssignTarget::Field { receiver, .. } => {
                    dump_expr(receiver, indent + 1, out);
                }
                AssignTarget::Local(_) => {}
            }
            dump_expr(&assign.value, indent + 1, out);
        }
        StatementKind::If(if_) => {
            out.push_str(&format!("{pad}if\n"));
            dump_expr(&if_.cond, indent + 1, out);
            dump_block(&if_.then_block, indent + 1, out);
            if let Some(else_block) = &if_.else_block {
                out.push_str(&format!("{pad}else\n"));
                dump_block(else_block, indent + 1, out);
            }
        }
        StatementKind::Try(try_) => {
            out.push_str(&format!("{pad}try\n"));
            dump_block(&try_.body, indent + 1, out);
            for catch in &try_.catches {
                out.push_str(&format!(
                    "{pad}catch {}: {}\n",
                    catch.name.text,
                    dump_type_ref(&catch.ty)
                ));
                dump_block(&catch.body, indent + 1, out);
            }
            if let Some(finally_body) = &try_.finally_body {
                out.push_str(&format!("{pad}finally\n"));
                dump_block(finally_body, indent + 1, out);
            }
        }
        StatementKind::Throw(expr) => {
            out.push_str(&format!("{pad}throw\n"));
            dump_expr(expr, indent + 1, out);
        }
        StatementKind::While(while_) => {
            out.push_str(&format!("{pad}while\n"));
            dump_expr(&while_.cond, indent + 1, out);
            dump_block(&while_.body, indent + 1, out);
        }
        StatementKind::Block(block) => {
            out.push_str(&format!("{pad}block\n"));
            dump_block(block, indent + 1, out);
        }
        StatementKind::SafetyBlock { mode, block } => {
            let name = match mode {
                SafetyMode::Safe => "Safe",
                SafetyMode::Unsafe => "Unsafe",
            };
            out.push_str(&format!("{pad}@{name} block\n"));
            dump_block(block, indent + 1, out);
        }
    }
}

fn dump_type_param(param: &TypeParamDecl) -> String {
    let variance = match param.variance {
        Variance::Invariant => "",
        Variance::In => "in ",
        Variance::Out => "out ",
    };
    let bound = match &param.inline_bound {
        None => String::new(),
        Some(bound) => format!(" : {}", dump_type_bound(bound)),
    };
    format!("{variance}{}{bound}", param.name.text)
}

fn dump_type_params(params: &[TypeParamDecl]) -> String {
    if params.is_empty() {
        String::new()
    } else {
        format!(
            "<{}>",
            params
                .iter()
                .map(dump_type_param)
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn dump_type_bound(bound: &TypeBound) -> String {
    match bound {
        TypeBound::Kind(TypeParamKindBound::Value) => "value".to_string(),
        TypeBound::Kind(TypeParamKindBound::Ref) => "ref".to_string(),
        TypeBound::Upper(ty) => dump_type_ref(ty),
    }
}

fn dump_where_clause(clause: Option<&WhereClause>) -> String {
    let Some(clause) = clause else {
        return String::new();
    };
    format!(
        " where {}",
        clause
            .constraints
            .iter()
            .map(|constraint| format!(
                "{} : {}",
                constraint.parameter.text,
                dump_type_bound(&constraint.bound)
            ))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// Compact one-line pattern rendering for dumps.
pub fn dump_pattern(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Binding(name) => name.text.clone(),
        Pattern::Wildcard { .. } => "_".to_string(),
        Pattern::Literal { expr, .. } => format!("{:?}", expr).chars().take(40).collect(),
        Pattern::Positional {
            path,
            elements,
            rest,
            ..
        } => {
            let path = path
                .iter()
                .map(|p| p.text.as_str())
                .collect::<Vec<_>>()
                .join(".");
            let mut parts: Vec<String> = elements.iter().map(dump_pattern).collect();
            if rest.is_some() {
                parts.push("..".to_string());
            }
            format!("{}({})", path, parts.join(", "))
        }
        Pattern::Named {
            path, fields, rest, ..
        } => {
            let path = path
                .iter()
                .map(|p| p.text.as_str())
                .collect::<Vec<_>>()
                .join(".");
            let mut parts: Vec<String> = fields
                .iter()
                .map(|f| match &f.rename {
                    Some(rename) => format!("{}: {}", f.name.text, rename.text),
                    None => f.name.text.clone(),
                })
                .collect();
            if rest.is_some() {
                parts.push("..".to_string());
            }
            format!("{}{{{}}}", path, parts.join(", "))
        }
        Pattern::Tuple { elements, rest, .. } => {
            let mut parts: Vec<String> = elements.iter().map(dump_pattern).collect();
            if let Some(rest) = rest {
                parts.push(format!("..@{}", rest.start));
            }
            format!("({})", parts.join(", "))
        }
    }
}

fn dump_expr(expr: &Expr, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    match expr {
        Expr::StringLiteral { value, .. } => {
            out.push_str(&format!("{pad}StringLiteral {value:?}\n"));
        }
        Expr::IntLiteral { value, .. } => out.push_str(&format!("{pad}IntLiteral {value}\n")),
        Expr::BoolLiteral { value, .. } => out.push_str(&format!("{pad}BoolLiteral {value}\n")),
        Expr::UnitLiteral { .. } => out.push_str(&format!("{pad}UnitLiteral\n")),
        Expr::TupleLiteral { elements, .. } => {
            out.push_str(&format!("{pad}TupleLiteral\n"));
            for element in elements {
                dump_expr(element, indent + 1, out);
            }
        }
        Expr::StructInit { name, args, .. } => {
            out.push_str(&format!("{pad}StructInit {}\n", name.text));
            for arg in args {
                dump_expr(arg, indent + 1, out);
            }
        }
        Expr::Var(ident) => out.push_str(&format!("{pad}Var {}\n", ident.text)),
        Expr::Lambda {
            id,
            is_suspend,
            parameters,
            body,
            ..
        } => {
            out.push_str(&format!("{pad}Lambda {} suspend={is_suspend}\n", id.0));
            match parameters {
                None => out.push_str(&format!("{pad}  parameters omitted\n")),
                Some(parameters) => {
                    for parameter in parameters {
                        let ty = parameter
                            .ty
                            .as_ref()
                            .map_or_else(|| "_".to_string(), dump_type_ref);
                        out.push_str(&format!(
                            "{pad}  param {}: {ty}\n",
                            dump_pattern(&parameter.target)
                        ));
                    }
                }
            }
            dump_block(body, indent + 1, out);
        }
        Expr::AnonymousFunction {
            id,
            is_suspend,
            params,
            return_ty,
            body,
            ..
        } => {
            let return_ty = return_ty
                .as_ref()
                .map_or_else(|| "_".to_string(), dump_type_ref);
            out.push_str(&format!(
                "{pad}AnonymousFunction {} suspend={is_suspend} return={return_ty}\n",
                id.0
            ));
            for param in params {
                out.push_str(&format!(
                    "{pad}  param {}: {}\n",
                    param.name.text,
                    dump_type_ref(&param.ty)
                ));
            }
            dump_block(body, indent + 1, out);
        }
        Expr::CallableReference {
            id, receiver, name, ..
        } => {
            out.push_str(&format!("{pad}CallableReference {} {}\n", id.0, name.text));
            if let Some(receiver) = receiver {
                dump_expr(receiver, indent + 1, out);
            }
        }
        Expr::FieldAccess(access) => {
            let selector = match &access.selector {
                FieldSelector::Name(name) => name.text.clone(),
                FieldSelector::Index(index, _) => format!("_{index}"),
            };
            let marker = if access.safe { "?" } else { "" };
            out.push_str(&format!("{pad}FieldAccess {marker}{selector}\n"));
            dump_expr(&access.receiver, indent + 1, out);
        }
        Expr::Call(call) => {
            let type_args = dump_call_type_args(&call.type_args);
            out.push_str(&format!("{pad}Call {}{type_args}\n", call.callee.text));
            for arg in &call.args {
                dump_expr(arg, indent + 1, out);
            }
        }
        Expr::Invoke { callee, args, .. } => {
            out.push_str(&format!("{pad}Invoke\n"));
            dump_expr(callee, indent + 1, out);
            for arg in args {
                dump_expr(arg, indent + 1, out);
            }
        }
        Expr::Binary { op, lhs, rhs, .. } => {
            out.push_str(&format!("{pad}Binary {op:?}\n"));
            dump_expr(lhs, indent + 1, out);
            dump_expr(rhs, indent + 1, out);
        }
        Expr::Unary { op, operand, .. } => {
            out.push_str(&format!("{pad}Unary {op:?}\n"));
            dump_expr(operand, indent + 1, out);
        }
        Expr::NullAssert { operand, .. } => {
            out.push_str(&format!("{pad}NullAssert\n"));
            dump_expr(operand, indent + 1, out);
        }
        Expr::Elvis { lhs, rhs, .. } => {
            out.push_str(&format!("{pad}Elvis\n"));
            dump_expr(lhs, indent + 1, out);
            dump_expr(rhs, indent + 1, out);
        }
        Expr::This { .. } => out.push_str(&format!("{pad}This\n")),
        Expr::MethodCall {
            receiver,
            name,
            type_args,
            args,
            ..
        } => {
            let type_args = dump_call_type_args(type_args);
            out.push_str(&format!("{pad}MethodCall {}{type_args}\n", name.text));
            dump_expr(receiver, indent + 1, out);
            for arg in args {
                dump_expr(arg, indent + 1, out);
            }
        }
        Expr::Is {
            operand,
            ty,
            negated,
            ..
        } => {
            out.push_str(&format!("{pad}Is {} {negated}\n", dump_type_ref(ty)));
            dump_expr(operand, indent + 1, out);
        }
        Expr::Cast {
            operand,
            ty,
            optional,
            ..
        } => {
            out.push_str(&format!(
                "{pad}Cast {} optional={optional}\n",
                dump_type_ref(ty)
            ));
            dump_expr(operand, indent + 1, out);
        }
        Expr::ArrayLiteral { elements, .. } => {
            out.push_str(&format!("{pad}ArrayLiteral\n"));
            for element in elements {
                dump_expr(element, indent + 1, out);
            }
        }
        Expr::Index {
            receiver, index, ..
        } => {
            out.push_str(&format!("{pad}Index\n"));
            dump_expr(receiver, indent + 1, out);
            dump_expr(index, indent + 1, out);
        }
        Expr::If(if_) => {
            out.push_str(&format!("{pad}IfExpression\n"));
            dump_expr(&if_.cond, indent + 1, out);
            dump_block(&if_.then_block, indent + 1, out);
            if let Some(else_block) = &if_.else_block {
                out.push_str(&format!("{pad}  else\n"));
                dump_block(else_block, indent + 1, out);
            }
        }
        Expr::When(when) => {
            out.push_str(&format!("{pad}WhenExpression\n"));
            dump_expr(&when.subject, indent + 1, out);
            for arm in &when.arms {
                out.push_str(&format!("{pad}  arm {}\n", dump_pattern(&arm.pattern)));
                dump_block(&arm.body, indent + 2, out);
            }
            if let Some(else_body) = &when.else_body {
                out.push_str(&format!("{pad}  else\n"));
                dump_block(else_body, indent + 2, out);
            }
        }
        Expr::Try(try_) => {
            out.push_str(&format!("{pad}TryExpression\n"));
            dump_block(&try_.body, indent + 1, out);
            for catch in &try_.catches {
                out.push_str(&format!(
                    "{pad}  catch {}: {}\n",
                    catch.name.text,
                    dump_type_ref(&catch.ty)
                ));
                dump_block(&catch.body, indent + 2, out);
            }
            if let Some(finally_body) = &try_.finally_body {
                out.push_str(&format!("{pad}  finally\n"));
                dump_block(finally_body, indent + 2, out);
            }
        }
    }
}

fn dump_call_type_args(type_args: &[TypeRef]) -> String {
    if type_args.is_empty() {
        String::new()
    } else {
        format!(
            "<{}>",
            type_args
                .iter()
                .map(dump_type_ref)
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}
