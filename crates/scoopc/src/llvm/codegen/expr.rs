//! 表达式 codegen（T0102d：从 `codegen/mod.rs` 拆分）。

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(super) fn codegen_expr_in_expected_context(
        &mut self,
        expr: &hir::Expr,
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let value = match &expr.kind {
            hir::ExprKind::UnresolvedIdent { name } => {
                self.codegen_unresolved_ident(expr.span, name, expected)
            }
            hir::ExprKind::Call { callee, args } => {
                self.codegen_call(expr.span, callee, args, expected, Some(expr.ty))
            }
            hir::ExprKind::Perform {
                effect_ty,
                op,
                args,
            } => self.codegen_perform_expr(expr.span, *effect_ty, op, args, expected),
            hir::ExprKind::Handle(handle) => self.codegen_handle_expr(expr.span, handle, expected),
            hir::ExprKind::Block(block) => {
                self.codegen_block_value_in_expected_context(block, expected)
            }
            hir::ExprKind::When { subject, arms } => {
                self.codegen_when_expr(expr.span, subject, arms, expected)
            }
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self.codegen_if_expr(
                expr.span,
                expr.ty,
                cond,
                then_branch,
                else_branch.as_deref(),
                expected,
            ),
            _ => self.codegen_expr(expr),
        }?;

        if let Some(target) = expected {
            if target == CgTy::Unit {
                if value.ty == CgTy::Never {
                    Ok(value)
                } else {
                    Ok(CgValue::unit())
                }
            } else {
                self.coerce_value(expr.span, value, target)
            }
        } else {
            Ok(value)
        }
    }

    pub(super) fn codegen_expr(
        &mut self,
        expr: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match &expr.kind {
            hir::ExprKind::Missing | hir::ExprKind::Todo(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "expression",
                    at: expr.span.into(),
                })
            }
            hir::ExprKind::UnresolvedIdent { .. } => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unresolved ident (missing expected type context)",
                at: expr.span.into(),
            }),
            hir::ExprKind::Literal(lit) => self.codegen_literal(expr.span, expr.ty, lit),
            hir::ExprKind::VarRef(v) => self.codegen_var_ref(expr.span, v),
            hir::ExprKind::StructLit { ty, fields } => {
                self.codegen_struct_lit(expr.span, *ty, fields)
            }
            hir::ExprKind::TupleLit { elements } => {
                self.codegen_tuple_lit(expr.span, expr.ty, elements)
            }
            hir::ExprKind::InterpolatedString { raw, parts } => {
                self.codegen_interpolated_string(expr.span, *raw, parts)
            }
            hir::ExprKind::Unary {
                op, expr: inner, ..
            } => self.codegen_unary(expr.span, expr.ty, *op, inner),
            hir::ExprKind::Binary { lhs, op, rhs, .. } => {
                self.codegen_binary(expr.span, *op, lhs, rhs)
            }
            hir::ExprKind::TypeCheck {
                expr: inner,
                op,
                target_ty,
                ..
            } => self.codegen_type_check_expr(expr.span, *op, inner, *target_ty),
            hir::ExprKind::Cast {
                expr: inner,
                op,
                target_ty,
                ..
            } => self.codegen_cast_expr(expr.span, *op, inner, *target_ty, expr.ty),
            hir::ExprKind::Block(block) => self.codegen_block_value(block),
            hir::ExprKind::Call { callee, args } => {
                self.codegen_call(expr.span, callee, args, None, Some(expr.ty))
            }
            hir::ExprKind::MemberAccess { receiver, member } => {
                self.codegen_member_access(expr.span, receiver, member)
            }
            hir::ExprKind::When { subject, arms } => {
                self.codegen_when_expr(expr.span, subject, arms, None)
            }
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self.codegen_if_expr(
                expr.span,
                expr.ty,
                cond,
                then_branch,
                else_branch.as_deref(),
                None,
            ),

            // 后续任务接入 MIR/CFG codegen
            hir::ExprKind::Closure(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "expression kind",
                at: expr.span.into(),
            }),
            hir::ExprKind::Perform {
                effect_ty,
                op,
                args,
            } => self.codegen_perform_expr(expr.span, *effect_ty, op, args, None),
            hir::ExprKind::Handle(handle) => {
                // T1611: infer expected type from HIR when not in expected context,
                // so statement-position handles don't need `val _: Unit = ...` workaround.
                let inferred = self.cg_ty_of(expr.ty);
                self.codegen_handle_expr(expr.span, handle, inferred)
            }
        }
    }
}
