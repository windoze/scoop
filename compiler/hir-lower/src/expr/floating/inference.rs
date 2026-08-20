//! Closed IEEE literal candidates share the ordinary expression transaction.

use super::*;

pub(crate) fn float_literal_candidate_kinds(expr: &ast::Expr) -> Option<&'static [hir::FloatKind]> {
    let literal = match expr {
        ast::Expr::FloatLiteral(literal) => literal,
        ast::Expr::Unary {
            op: ast::UnOp::Plus | ast::UnOp::Neg,
            operand,
            ..
        } => {
            let ast::Expr::FloatLiteral(literal) = &**operand else {
                return None;
            };
            literal
        }
        _ => return None,
    };
    Some(match literal.suffix {
        ast::FloatSuffix::None => &[hir::FloatKind::F64, hir::FloatKind::F32],
        ast::FloatSuffix::Float => &[hir::FloatKind::F32],
    })
}

pub(crate) fn float_literal_default_kind(expr: &ast::Expr) -> Option<hir::FloatKind> {
    float_literal_candidate_kinds(expr).map(|kinds| kinds[0])
}

impl Lowerer {
    pub(in crate::expr) fn probe_float_literal_receiver(
        &self,
        receiver: &ast::Expr,
        expected: Option<TypeId>,
        mut lower: impl FnMut(&mut Lowerer, hir::Expr, &mut Vec<hir::Statement>) -> Option<hir::Expr>,
    ) -> Option<SuccessfulExprLayer> {
        let kinds = float_literal_candidate_kinds(receiver)?;
        let expected_kind = expected.and_then(|ty| self.float_kind(ty));
        let mut successful = Vec::new();
        for &kind in kinds {
            let Ok(layer) = self.probe_expr_layer(|state, sink| {
                let ty = state.core_float_type(kind).ok()?;
                let receiver = state.lower_expr(receiver, sink, Some(ty))?;
                lower(state, receiver, sink)
            }) else {
                continue;
            };
            let preserves_kind = match &layer.expression.kind {
                ExprKind::FloatUnary {
                    kind: actual,
                    operation,
                    ..
                } if *actual == kind => !operation.is_predicate(),
                ExprKind::FloatBinary {
                    kind: actual,
                    operation,
                    ..
                } if *actual == kind => !operation.is_predicate(),
                ExprKind::FloatConversion {
                    conversion:
                        hir::HirFloatConversion::ToInteger { source, .. }
                        | hir::HirFloatConversion::BetweenFloats { source, .. },
                    ..
                } if *source == kind => false,
                _ => continue,
            };
            successful.push((kind, preserves_kind, layer));
        }
        let selected = expected_kind
            .and_then(|expected| {
                successful
                    .iter()
                    .position(|(kind, preserves, _)| *kind == expected && *preserves)
            })
            .or_else(|| successful.iter().position(|(kind, _, _)| *kind == kinds[0]))
            .or_else(|| (successful.len() == 1).then_some(0))?;
        Some(successful.swap_remove(selected).2)
    }
}
