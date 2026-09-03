use super::super::*;

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
