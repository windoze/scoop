use super::*;

impl Lowerer {
    pub(crate) fn reject_expression_body_return(&mut self, expr: &ast::Expr) -> bool {
        if crate::expr::expr_contains_return(expr) {
            self.error(
                expr.span(),
                "an expression-bodied function cannot use explicit `return`".into(),
            );
            true
        } else {
            false
        }
    }

    pub(crate) fn unreachable_expression(&mut self, span: Span) -> hir::Expr {
        hir::Expr {
            kind: hir::ExprKind::Unreachable,
            ty: self.nothing_type(),
            span,
            origin: self.expression_origin(span),
        }
    }

    pub(crate) fn lower_return(
        &mut self,
        value: Option<&ast::Expr>,
        span: Span,
        out: &mut Vec<hir::Statement>,
    ) -> Option<()> {
        if self
            .initialization_context
            .as_ref()
            .is_some_and(|context| context.capture_depth == self.capture_contexts.len())
        {
            self.error(
                span,
                "`return` is not allowed in an initializer or constructor body".into(),
            );
            return None;
        }
        if self.lowering_default_template {
            self.error(
                span,
                "`return` is not allowed in a default expression".into(),
            );
            return None;
        }
        let target = self
            .current_source_context
            .map(|context| self.source_contexts[context].subject());
        if let Some(hir::SourceContextSubject::Function(function)) = target
            && self
                .current_initialization_unit
                .is_some_and(|unit| self.initialization_units[unit].initializer == *function)
        {
            self.error(
                span,
                "`return` is not allowed in a top-level initializer".into(),
            );
            return None;
        }
        if !matches!(
            target,
            Some(hir::SourceContextSubject::Function(_))
                | Some(hir::SourceContextSubject::LexicalCallable {
                    role: scoop_identity::LexicalCallableRole::AnonymousFunctionBody,
                    ..
                })
        ) {
            self.error(span, "`return` requires an enclosing function body".into());
            return None;
        }
        if self.return_inference.is_some() {
            let value = match value {
                Some(source) => {
                    let value = self.lower_expr(source, out, None)?;
                    if !self.is_nothing_ty(value.ty) {
                        self.return_inference
                            .as_mut()
                            .expect("inference is active")
                            .value_types
                            .push(value.ty);
                    }
                    Some(value)
                }
                None => {
                    self.return_inference
                        .as_mut()
                        .expect("inference is active")
                        .saw_bare = true;
                    None
                }
            };
            out.push(hir::Statement {
                kind: hir::StatementKind::Return { value },
                span,
            });
            return Some(());
        }
        let return_ty = self.current_return_ty;
        let value = match value {
            None => {
                if !self.types_equal(return_ty, self.unit) {
                    let name = self.current_fn_name.clone();
                    let expected = self.type_name(return_ty);
                    self.error(
                        span,
                        format!(
                            "`return` without a value in function `{name}` returning {expected}"
                        ),
                    );
                    return None;
                }
                None
            }
            Some(source) => {
                let value = self.lower_expr(source, out, Some(return_ty))?;
                if !self.is_subtype(value.ty, return_ty) {
                    let name = self.current_fn_name.clone();
                    let expected = self.type_name(return_ty);
                    let found = self.type_name(value.ty);
                    let message = self.with_nominal_invariance_detail(
                        format!(
                            "`return` value of `{name}` must be of type {expected}, found {found}"
                        ),
                        value.ty,
                        return_ty,
                    );
                    self.error(source.span(), message);
                    return None;
                }
                Some(self.adapt_to(value, return_ty))
            }
        };
        self.push_return(value, span, out);
        Some(())
    }
}
