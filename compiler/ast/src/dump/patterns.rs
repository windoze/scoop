use super::super::*;

/// Compact one-line pattern rendering for dumps.
pub fn dump_pattern(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Binding(name) => name.text.clone(),
        Pattern::Wildcard { .. } => "_".to_string(),
        Pattern::Literal { expr, .. } => dump_literal(expr),
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

fn dump_literal(expr: &Expr) -> String {
    match expr {
        Expr::FloatLiteral(value) => value.to_string(),
        Expr::CharLiteral { value, .. } => format!("{value:?}"),
        Expr::StringLiteral { value, .. } => format!("{value:?}"),
        Expr::IntLiteral(literal) => literal.to_string(),
        Expr::BoolLiteral { value, .. } => value.to_string(),
        Expr::UnitLiteral { .. } => "()".to_string(),
        Expr::Unary {
            op: UnOp::Plus,
            operand,
            ..
        } => format!("+{}", dump_literal(operand)),
        Expr::Unary {
            op: UnOp::Neg,
            operand,
            ..
        } => format!("-{}", dump_literal(operand)),
        _ => panic!("a parsed literal pattern contains only literal syntax"),
    }
}
