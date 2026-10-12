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
        let operand = self.lower_type_check_operand(operand, sink)?;
        self.lower_is_value(operand, ty_ref, negated, span)
    }

    pub(crate) fn lower_type_check_operand(
        &mut self,
        operand: &ast::Expr,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        // A mutable local may hold another value on a loop backedge. Type
        // tests inspect its declared storage, independently of the current view.
        // Function views can allocate adapters, whose type capabilities differ.
        let storage = self
            .function_smart_cast_storage(operand)
            .map(|(binding, _)| binding);
        let storage = storage.or_else(|| match operand {
            ast::Expr::Var(name) => self
                .scopes
                .lookup(&name.text)
                .filter(|&local| self.locals[local].mutable)
                .map(|local| self.locals[local].binding),
            _ => None,
        });
        let saved = storage.and_then(|binding| self.smart_casts.types.remove(&binding));
        let operand = self.lower_expr(operand, sink, None);
        if let (Some(binding), Some(types)) = (storage, saved) {
            self.smart_casts.types.insert(binding, types);
        }
        operand
    }

    pub(crate) fn lower_is_value(
        &mut self,
        operand: hir::Expr,
        ty_ref: &ast::TypeRef,
        negated: bool,
        span: Span,
    ) -> Option<hir::Expr> {
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
        let operand = match operand {
            hir::Expr {
                kind: ExprKind::Unbox(source),
                ..
            } if !self.is_value_ty(source.ty) => *source,
            operand => self.adapt_to(operand, self.any),
        };
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
        let operand = self.lower_type_check_operand(operand, sink)?;
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
        if optional && !self.has_option_protocol() {
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
        if let Err(error) = self.prepare_cast_exception_type() {
            self.error(
                span,
                format!("cannot resolve cast exception type: {error:?}"),
            );
            return None;
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
