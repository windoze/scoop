//! Type conditions test the saved subject while ordinary expressions retain
//! the path's smart casts and short-circuit evaluation order.

use super::*;

impl Lowerer {
    pub(super) fn lower_subject_predicate(
        &mut self,
        source: &ast::Expr,
        subject: &Subject,
    ) -> Option<hir::WhenGuard> {
        let incoming = self.smart_casts.clone();
        let predicate = self.lower_subject_predicate_inner(source, subject)?;
        self.advance_smart_casts(incoming, &predicate.setup, Some(&predicate.condition));
        Some(predicate)
    }

    fn lower_subject_predicate_inner(
        &mut self,
        source: &ast::Expr,
        subject: &Subject,
    ) -> Option<hir::WhenGuard> {
        match source {
            ast::Expr::Is {
                ty, negated, span, ..
            } => Some(hir::WhenGuard {
                setup: Vec::new(),
                condition: self.lower_is_value(subject.value.clone(), ty, *negated, *span)?,
            }),
            ast::Expr::Binary {
                op: ast::BinOp::Or,
                lhs,
                rhs,
                span,
                ..
            } => {
                let mut left = self.lower_subject_predicate(lhs, subject)?;
                let facts = self.checked_condition_facts(&left.condition, false);
                let right = self
                    .with_smart_casts(facts, |this| this.lower_subject_predicate(rhs, subject))?;
                left.condition = self.finish_short_circuit(
                    hir::BinOp::Or,
                    left.condition,
                    right.condition,
                    right.setup,
                    *span,
                    &mut left.setup,
                );
                Some(left)
            }
            _ => self.lower_when_predicate(source),
        }
    }
}
