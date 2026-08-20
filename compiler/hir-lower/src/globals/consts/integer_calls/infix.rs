use scoop_ast as ast;
use scoop_hir as hir;

use crate::Lowerer;
use crate::globals::{PendingConst, PendingOrdinary};

use super::super::{
    ConstIntegerIntrinsicKind, ConstState, EvaluatedConst, ResolvedConstIntegerIntrinsic,
    evaluate_integer_no_gc_operation,
};

impl Lowerer {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::globals::consts) fn evaluate_const_integer_infix(
        &mut self,
        lhs: &ast::Expr,
        target: &ast::InfixTarget,
        rhs: &ast::Expr,
        expected: Option<hir::TypeId>,
        file: usize,
        declarations: &[PendingConst<'_>],
        ordinary: &[PendingOrdinary<'_>],
        states: &mut [ConstState],
        stack: &mut Vec<usize>,
        span: ast::Span,
    ) -> Option<EvaluatedConst> {
        let ast::InfixTarget::Named(name) = target else {
            self.error(
                span,
                "const initializer cannot invoke an untyped infix callable".to_string(),
            );
            return None;
        };
        let receiver_expected = self
            .select_const_integer_literal_receiver_kind(
                lhs,
                Some(rhs),
                &name.text,
                expected,
                true,
                file,
                declarations,
                ordinary,
                states,
                stack,
            )
            .map(|kind| self.integer_type(kind));
        let left = self.evaluate_const_expression(
            lhs,
            receiver_expected,
            file,
            declarations,
            ordinary,
            states,
            stack,
        )?;
        let hir::Type::Integer(kind) = self.types[left.ty] else {
            self.error(
                name.span,
                "integer const intrinsic requires an integer receiver".to_string(),
            );
            return None;
        };
        let Some(ResolvedConstIntegerIntrinsic {
            kind: ConstIntegerIntrinsicKind::NoGcOperation(operation),
            is_infix,
            ..
        }) = self.resolve_const_integer_intrinsic(kind, &name.text)
        else {
            self.error(
                name.span,
                "const initializer did not resolve to the exact typed core integer intrinsic"
                    .to_string(),
            );
            return None;
        };
        if !is_infix || operation.arity() != hir::IntegerOperationArity::Binary {
            self.error(
                name.span,
                "const infix call did not resolve to a typed core integer infix intrinsic"
                    .to_string(),
            );
            return None;
        }
        let shift = matches!(
            operation,
            hir::NoGcIntegerOperation::Shl
                | hir::NoGcIntegerOperation::Shr
                | hir::NoGcIntegerOperation::Ushr
        );
        let right_expected = if shift {
            self.integer_type(hir::IntegerKind::SIGNED_64)
        } else {
            left.ty
        };
        let right = self.evaluate_const_expression(
            rhs,
            Some(right_expected),
            file,
            declarations,
            ordinary,
            states,
            stack,
        )?;
        let (
            hir::ConstPropertyValue::Integer(left_value),
            hir::ConstPropertyValue::Integer(right_value),
        ) = (left.value, right.value)
        else {
            self.error(
                span,
                "integer const intrinsic requires integer operands".to_string(),
            );
            return None;
        };
        if right.ty != right_expected {
            self.error(
                span,
                format!(
                    "integer const intrinsic expected {}, found {}",
                    self.type_name(right_expected),
                    self.type_name(right.ty)
                ),
            );
            return None;
        }
        let value = evaluate_integer_no_gc_operation(operation, left_value, Some(right_value))?;
        Some(EvaluatedConst {
            value,
            ty: self.integer_no_gc_result_type(kind, operation),
        })
    }
}
