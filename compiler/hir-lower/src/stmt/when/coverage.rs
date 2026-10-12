use super::*;

impl Lowerer {
    /// Ordinary calls and named constants do not establish a finite value
    /// set. Source literals and unit variants with known structural or
    /// integer equality supply rows to the existing pattern matrix.
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
            if let Some(value) = ast::LiteralPattern::from_expr(source)
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
                        value,
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
                if probe.integral_enum_kind(ty).is_none()
                    && !matches!(
                        probe.derived_equality_candidate(ty, source.span()),
                        Ok(Some(_))
                    )
                {
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
