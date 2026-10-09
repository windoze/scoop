use super::*;

impl Lowerer {
    pub(super) fn build_open_enum_equality(
        &mut self,
        ty: hir::TypeId,
        lhs: hir::Expr,
        rhs: hir::Expr,
        span: ast::Span,
        stack: &mut Vec<hir::TypeId>,
    ) -> Result<hir::Expr, String> {
        let mut cases = Vec::new();
        for variant in self.enum_variants(ty) {
            let target = variant.application;
            let mut comparisons = [lhs.clone(), rhs.clone()]
                .into_iter()
                .map(|operand| hir::Expr {
                    kind: hir::ExprKind::VariantTest {
                        operand: Box::new(operand),
                        variant: target,
                    },
                    ty: self.boolean,
                    span,
                    origin: self.expression_origin(span),
                })
                .collect::<Vec<_>>();
            for (index, (name, field_ty)) in variant.fields.into_iter().enumerate() {
                let field = self.enum_variant_field_at(target, index as u32);
                let project = |operand: hir::Expr| hir::Expr {
                    kind: hir::ExprKind::VariantPayloadProject {
                        operand: Box::new(operand),
                        field,
                    },
                    ty: field_ty,
                    span,
                    origin: self.expression_origin(span),
                };
                let left = project(lhs.clone());
                let right = project(rhs.clone());
                comparisons.push(self.build_derived_field_comparison(
                    field_ty,
                    left,
                    right,
                    &format!("{}.{}.{name}", variant.owner_name, variant.name),
                    span,
                    stack,
                )?);
            }
            // Keep payload evaluation inside both successful tag-test edges.
            // A Boolean join before the payload would lose this dominance.
            cases.push(
                comparisons
                    .into_iter()
                    .rev()
                    .reduce(|rhs, lhs| hir::Expr {
                        kind: hir::ExprKind::Binary {
                            op: hir::BinOp::And,
                            lhs: Box::new(lhs),
                            rhs: Box::new(rhs),
                        },
                        ty: self.boolean,
                        span,
                        origin: self.expression_origin(span),
                    })
                    .expect("each variant comparison starts with two tag tests"),
            );
        }
        Ok(cases
            .into_iter()
            .reduce(|lhs, rhs| hir::Expr {
                kind: hir::ExprKind::Binary {
                    op: hir::BinOp::Or,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                ty: self.boolean,
                span,
                origin: self.expression_origin(span),
            })
            .unwrap_or_else(|| self.bool_expr(false, self.boolean, span)))
    }
}
