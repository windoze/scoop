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

pub(super) fn dump_type_ref(ty: &TypeRef) -> String {
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
