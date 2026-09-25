use super::*;

impl Lowerer {
    /// `expr is T` / `expr !is T` (M6): the static premise is that the
    /// operand could ever hold a `T` (`could_hold`); a check between
    /// unrelated types is useless and diagnosed.
    pub(super) fn lower_is(
        &mut self,
        operand: &ast::Expr,
        ty_ref: &ast::TypeRef,
        negated: bool,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let operand = self.lower_expr(operand, sink, None)?;
        let check_ty = self.resolve_type_ref(ty_ref)?;
        if !self.could_hold(operand.ty, check_ty) {
            let found = self.type_name(operand.ty);
            let check = self.type_name(check_ty);
            self.error(
                span,
                format!("useless type check: `{found}` can never be `{check}`"),
            );
            return None;
        }
        let is_expr = hir::Expr {
            kind: ExprKind::IsInstance {
                operand: Box::new(operand),
                check_ty,
            },
            ty: self.boolean,
            span,
            origin: self.expression_origin(span),
        };
        if negated {
            Some(hir::Expr {
                kind: ExprKind::Unary {
                    op: hir::UnOp::Not,
                    operand: Box::new(is_expr),
                },
                ty: self.boolean,
                span,
                origin: self.expression_origin(span),
            })
        } else {
            Some(is_expr)
        }
    }

    /// `expr as T` / `expr as? T` (M6). An upcast is free (`adapt_to`:
    /// boxing for value types, a retype for references — `as?` wraps
    /// the result in `Some`). A downcast lowers to `ExprKind::Cast`;
    /// for a value-type target the non-optional form is followed by
    /// `Unbox` (the check keeps the reference, the unbox extracts the
    /// payload), while `as?` yields `Option<T>` directly (the payload
    /// unboxing is part of the optional-cast semantics — `Cast::ty`
    /// is `Option<T>`, so no separate `Unbox` node can be attached).
    pub(super) fn lower_cast(
        &mut self,
        operand: &ast::Expr,
        ty_ref: &ast::TypeRef,
        optional: bool,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let operand = self.lower_expr(operand, sink, None)?;
        let target = self.resolve_type_ref(ty_ref)?;
        if !self.could_hold(operand.ty, target) {
            let found = self.type_name(operand.ty);
            let check = self.type_name(target);
            self.error(
                span,
                format!("cast from `{found}` to `{check}` can never succeed"),
            );
            return None;
        }
        if optional && self.option_enumeration().is_none() {
            // The missing core `Option` was already diagnosed.
            return None;
        }
        if self.is_subtype(operand.ty, target) {
            let adapted = self.adapt_to(operand, target);
            if optional {
                let ty = self.option_type(target);
                return Some(hir::Expr {
                    kind: ExprKind::SomeWrap(Box::new(adapted)),
                    ty,
                    span,
                    origin: self.expression_origin(span),
                });
            }
            return Some(adapted);
        }
        if optional {
            let ty = self.option_type(target);
            return Some(hir::Expr {
                kind: ExprKind::Cast {
                    operand: Box::new(operand),
                    check_ty: target,
                    optional: true,
                },
                ty,
                span,
                origin: self.expression_origin(span),
            });
        }
        let cast = hir::Expr {
            kind: ExprKind::Cast {
                operand: Box::new(operand),
                check_ty: target,
                optional: false,
            },
            ty: target,
            span,
            origin: self.expression_origin(span),
        };
        if self.is_value_ty(target) {
            Some(hir::Expr {
                kind: ExprKind::Unbox(Box::new(cast)),
                ty: target,
                span,
                origin: self.expression_origin(span),
            })
        } else {
            Some(cast)
        }
    }
}
