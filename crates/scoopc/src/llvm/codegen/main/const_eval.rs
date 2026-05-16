//! Constant evaluation for top-level initializers (int / float / bool).

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn const_eval_float_expr(
        &self,
        expr: &hir::Expr,
        target_ty: CgTy,
    ) -> Option<BasicValueEnum<'ctx>> {
        let value = self.const_eval_float_expr_value(expr)?;
        match target_ty {
            CgTy::Float64 => Some(self.context.f64_type().const_float(value).into()),
            CgTy::Float32 => Some(
                self.context
                    .f32_type()
                    .const_float(f64::from(value as f32))
                    .into(),
            ),
            _ => None,
        }
    }

    pub(in crate::llvm::codegen) fn const_eval_float_expr_value(
        &self,
        expr: &hir::Expr,
    ) -> Option<f64> {
        match &expr.kind {
            hir::ExprKind::Literal(hir::LiteralKind::Float64(value)) => Some(*value),
            hir::ExprKind::Literal(hir::LiteralKind::Float32(value)) => Some(f64::from(*value)),
            hir::ExprKind::Unary {
                op: ast::UnaryOp::Neg,
                expr: inner,
                ..
            } => Some(-self.const_eval_float_expr_value(inner)?),
            _ => None,
        }
    }

    pub(in crate::llvm::codegen) fn const_eval_int_expr_bits(
        &self,
        expr: &hir::Expr,
        int_ty: IntTy,
    ) -> Result<Option<u128>, LlvmEmitError> {
        match &expr.kind {
            hir::ExprKind::Literal(hir::LiteralKind::Int) => Ok(Some(u128::from(
                self.int_literal_bits_for_ty(expr.span, int_ty)?,
            ))),
            hir::ExprKind::Literal(hir::LiteralKind::SynthInt(v)) => {
                Ok(Some(mask_to_bits(*v as u128, int_ty.bits)))
            }
            hir::ExprKind::Unary {
                op: ast::UnaryOp::Neg,
                expr: inner,
                ..
            } if matches!(inner.kind, hir::ExprKind::Literal(hir::LiteralKind::Int)) => Ok(Some(
                u128::from(self.negated_int_literal_bits_for_ty(expr.span, inner.span, int_ty)?),
            )),
            hir::ExprKind::Unary {
                op: ast::UnaryOp::Neg,
                expr: inner,
                ..
            } => Ok(self
                .const_eval_int_expr_bits(inner, int_ty)?
                .map(|v| mask_to_bits(0u128.wrapping_sub(v), int_ty.bits))),
            hir::ExprKind::Unary {
                op: ast::UnaryOp::BitNot,
                expr: inner,
                ..
            } => Ok(self
                .const_eval_int_expr_bits(inner, int_ty)?
                .map(|v| mask_to_bits(!v, int_ty.bits))),
            _ => Ok(None),
        }
    }

    pub(in crate::llvm::codegen) fn const_eval_bool_expr(&self, expr: &hir::Expr) -> Option<bool> {
        match &expr.kind {
            hir::ExprKind::Literal(hir::LiteralKind::Bool(v)) => Some(*v),
            hir::ExprKind::Unary {
                op: ast::UnaryOp::Not,
                expr: inner,
                ..
            } => Some(!self.const_eval_bool_expr(inner)?),
            _ => None,
        }
    }
}
