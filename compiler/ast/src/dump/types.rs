use super::super::*;

pub(super) fn dump_annotations(annotations: &[Annotation], indent: usize, out: &mut String) {
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
                    let value = dump_annotation_literal(&arg.value);
                    format!("{name}{value}")
                })
                .collect();
            format!("({})", args.join(", "))
        };
        out.push_str(&format!("{pad}@{}{}\n", annotation.name.text, args));
    }
}

pub(super) fn dump_type_ref(ty: &TypeRef) -> String {
    match &ty.kind {
        TypeRefKind::Named(name) => name.text.clone(),
        TypeRefKind::Generic(name, args) => {
            let inner: Vec<String> = args.iter().map(dump_type_ref).collect();
            format!("{}<{}>", name.text, inner.join(", "))
        }
        TypeRefKind::Qualified { path, arguments } => {
            let mut name = path
                .iter()
                .map(|segment| segment.text.as_str())
                .collect::<Vec<_>>()
                .join(".");
            if !arguments.is_empty() {
                let inner: Vec<String> = arguments.iter().map(dump_type_ref).collect();
                name.push('<');
                name.push_str(&inner.join(", "));
                name.push('>');
            }
            name
        }
        TypeRefKind::Unit => "Unit".to_string(),
        TypeRefKind::AppliedMember {
            owner,
            name,
            arguments,
        } => {
            let mut result = format!("{}.{}", dump_type_ref(owner), name.text);
            if !arguments.is_empty() {
                result.push_str(&format!(
                    "<{}>",
                    arguments
                        .iter()
                        .map(dump_type_ref)
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            result
        }
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

pub(super) fn dump_annotation_literal(value: &AnnotationLiteral) -> String {
    match value {
        AnnotationLiteral::String(value) => format!("{value:?}"),
        AnnotationLiteral::Int(literal) => literal.to_string(),
        AnnotationLiteral::Float(literal) => literal.to_string(),
        AnnotationLiteral::Boolean(value) => value.to_string(),
        AnnotationLiteral::Char(value) => format!("{value:?}"),
        AnnotationLiteral::SignedInt { negative, literal } => {
            format!("{}{literal}", if *negative { "-" } else { "+" })
        }
        AnnotationLiteral::SignedFloat { negative, literal } => {
            format!("{}{literal}", if *negative { "-" } else { "+" })
        }
        AnnotationLiteral::ConstReference(reference) => dump_constant_reference(reference),
        AnnotationLiteral::Array(elements) => format!(
            "[{}]",
            elements
                .iter()
                .map(|element| dump_annotation_literal(&element.value))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

pub(super) fn dump_annotation_prefix(annotations: &[Annotation]) -> String {
    let mut text = String::new();
    dump_annotations(annotations, 0, &mut text);
    text.replace('\n', " ")
}

pub(super) fn dump_annotation_class(
    declaration: &AnnotationClassDecl,
    indent: usize,
    out: &mut String,
) {
    dump_annotations(&declaration.annotations, indent, out);
    let parameters = declaration
        .parameters
        .iter()
        .map(|parameter| {
            let default = parameter
                .default
                .as_ref()
                .map(|value| format!(" = {}", dump_annotation_literal(value)))
                .unwrap_or_default();
            format!(
                "val {}: {}{default}",
                parameter.name.text,
                dump_type_ref(&parameter.ty)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    out.push_str(&format!(
        "{}{}annotation class {}({parameters})\n",
        "  ".repeat(indent),
        super::declarations::dump_visibility(declaration.visibility),
        declaration.name.text
    ));
}

fn dump_constant_reference(reference: &Expr) -> String {
    match reference {
        Expr::Var(name) => name.text.clone(),
        Expr::TypeQualifier(ty) => dump_type_ref(ty),
        Expr::FieldAccess(access) => {
            let selector = match &access.selector {
                FieldSelector::Name(name) => name.text.clone(),
                FieldSelector::Index(index, _) => format!("_{index}"),
            };
            format!("{}.{}", dump_constant_reference(&access.receiver), selector)
        }
        _ => {
            let mut output = String::new();
            super::expressions::dump_expr(reference, 0, &mut output);
            output.trim().to_string()
        }
    }
}
