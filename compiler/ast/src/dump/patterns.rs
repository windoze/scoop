use super::super::*;

/// Compact one-line pattern rendering for dumps.
pub fn dump_pattern(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Binding(name) => name.text.clone(),
        Pattern::Wildcard { .. } => "_".to_string(),
        Pattern::Literal { value, .. } => dump_literal(value),
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
                .map(|field| match &*field.subpattern {
                    Pattern::Binding(binding) if binding == &field.field => {
                        field.field.text.clone()
                    }
                    subpattern => {
                        format!("{}: {}", field.field.text, dump_pattern(subpattern))
                    }
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
            let trailing_comma = if elements.len() == 1 && rest.is_none() {
                ","
            } else {
                ""
            };
            format!("({}{trailing_comma})", parts.join(", "))
        }
    }
}

fn dump_literal(value: &LiteralPattern) -> String {
    match value {
        LiteralPattern::Integer { value, negated } => {
            format!("{}{value}", if *negated { "-" } else { "" })
        }
        LiteralPattern::Float { value, negated } => {
            format!("{}{value}", if *negated { "-" } else { "" })
        }
        LiteralPattern::Char(value) => format!("{value:?}"),
        LiteralPattern::String(value) => format!("{value:?}"),
        LiteralPattern::Boolean(value) => value.to_string(),
        LiteralPattern::Unit => "()".to_string(),
    }
}
