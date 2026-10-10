use super::*;

impl Lowerer {
    /// Ordinary calls and named constants do not establish a finite value
    /// set. Only source literals and derived-equality unit variants supply
    /// rows to the existing pattern matrix.
    pub(super) fn ordinary_when_coverage(
        &mut self,
        conditions: &ast::NonEmptyVec<ast::WhenCondition>,
        ty: TypeId,
    ) -> Vec<hir::Pattern> {
        let mut rows = Vec::new();
        for condition in conditions.iter() {
            let ast::WhenCondition::Expression(source) = condition else {
                continue;
            };
            if is_literal(source)
                && (matches!(
                    self.types[ty],
                    Type::Boolean | Type::Unit | Type::String | Type::Integer(_)
                ) || self.is_char_type(ty)
                    || self.float_kind(ty).is_some())
            {
                // Coverage cannot add diagnostics to an already valid value
                // comparison (for example, equality with a wider integer).
                let mut probe = self.clone();
                if let Some(pattern) = probe.lower_pattern(
                    &ast::Pattern::Literal {
                        expr: Box::new(source.clone()),
                        span: source.span(),
                    },
                    ty,
                    PatternCtx {
                        mutable: false,
                        in_when: true,
                    },
                ) {
                    rows.push(pattern);
                }
            } else if matches!(self.types[ty], Type::Enum(_))
                && matches!(source, ast::Expr::Var(_) | ast::Expr::FieldAccess(_))
            {
                let mut probe = self.clone();
                if !matches!(
                    probe.derived_equality_candidate(ty, source.span()),
                    Ok(Some(_))
                ) {
                    continue;
                }
                let mut setup = Vec::new();
                if let Some(value) = probe.lower_expr(source, &mut setup, Some(ty))
                    && let hir::ExprKind::VariantConstruct {
                        variant: application,
                        args: fields,
                    } = value.kind
                    && fields.is_empty()
                    && probe.types_equal(value.ty, ty)
                {
                    rows.push(hir::Pattern::Variant {
                        application,
                        fields: Vec::new(),
                    });
                }
            }
        }
        rows
    }
}

fn is_literal(expr: &ast::Expr) -> bool {
    match expr {
        ast::Expr::IntLiteral(_)
        | ast::Expr::FloatLiteral(_)
        | ast::Expr::BoolLiteral { .. }
        | ast::Expr::CharLiteral { .. }
        | ast::Expr::StringLiteral { .. }
        | ast::Expr::UnitLiteral { .. } => true,
        ast::Expr::Unary {
            op: ast::UnOp::Neg | ast::UnOp::Plus,
            operand,
            ..
        } => matches!(
            operand.as_ref(),
            ast::Expr::IntLiteral(_) | ast::Expr::FloatLiteral(_)
        ),
        _ => false,
    }
}
