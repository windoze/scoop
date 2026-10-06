use super::*;
use scoop_ast::{TypeRef, TypeRefKind};

impl Parser {
    /// Only a dotted/callable-reference suffix commits an applied qualifier.
    /// A comparison or a generic call retains its original parsing path.
    pub(super) fn parse_applied_qualifier(
        &mut self,
        receiver: &Expr,
    ) -> Result<Option<TypeRef>, Diagnostic> {
        let Some(mut ty) = qualifier_type(receiver) else {
            return Ok(None);
        };
        let saved = self.pos;
        let Ok((arguments, end)) = self.parse_nominal_arguments(ty.span.end) else {
            self.pos = saved;
            return Ok(None);
        };
        if !matches!(self.peek().kind, TokenKind::Dot | TokenKind::DoubleColon) {
            self.pos = saved;
            return Ok(None);
        }
        ty.kind = match ty.kind {
            TypeRefKind::Named(name) => TypeRefKind::Generic(name, arguments),
            TypeRefKind::Qualified {
                path,
                arguments: previous,
            } if previous.is_empty() => TypeRefKind::Qualified { path, arguments },
            TypeRefKind::AppliedMember {
                owner,
                name,
                arguments: previous,
            } if previous.is_empty() => TypeRefKind::AppliedMember {
                owner,
                name,
                arguments,
            },
            _ => {
                self.pos = saved;
                return Ok(None);
            }
        };
        ty.span.end = end;
        Ok(Some(ty))
    }
}

fn qualifier_type(expression: &Expr) -> Option<TypeRef> {
    match expression {
        Expr::Var(name) => Some(TypeRef {
            kind: TypeRefKind::Named(name.clone()),
            span: name.span,
        }),
        Expr::TypeQualifier(ty) => Some(ty.clone()),
        Expr::FieldAccess(access) if access.navigation == Navigation::Direct => {
            let FieldSelector::Name(name) = &access.selector else {
                return None;
            };
            Some(qualifier_type(&access.receiver)?.with_member(
                name.clone(),
                Vec::new(),
                access.span.end,
            ))
        }
        _ => None,
    }
}
