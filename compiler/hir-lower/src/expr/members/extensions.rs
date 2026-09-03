use super::*;

impl Lowerer {
    pub(in crate::expr) fn finish_extension_call(
        &mut self,
        candidates: &[hir::FunctionId],
        name: &str,
        receiver: hir::Expr,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let explicit_type_args = self.resolve_call_type_args(call.type_args)?;
        let resolved = self.resolve_extension_overload(
            name,
            candidates,
            receiver,
            crate::overload::OverloadCall {
                explicit_type_args: &explicit_type_args,
                arg_exprs: call.args,
                span: call.span,
                expected_result: expected,
            },
            sink,
        )?;
        let callee = self.materialize_resolved_callee(&resolved);
        self.check_call_effects(callee, call.span);
        Some(hir::Expr {
            kind: ExprKind::Call {
                callee,
                args: resolved.args,
            },
            ty: resolved.return_ty,
            span: call.span,
        })
    }

    /// `m.toArray()` / `a.toMutableArray()` (spec 10.4). The receiver and
    /// result use exact intrinsic class applications; only the clone operation
    /// itself remains compiler-lowered.
    pub(in crate::expr) fn lower_array_method_conversion(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        args: &[ast::Expr],
        span: Span,
        target_kind: ArrayKind,
        element: TypeId,
    ) -> Option<hir::Expr> {
        if !args.is_empty() {
            self.error(
                span,
                format!(
                    "method `{}` takes exactly 0 arguments, but {} were supplied",
                    name.text,
                    args.len()
                ),
            );
            return None;
        }
        let ty = self.array_type(target_kind, element);
        Some(hir::Expr {
            kind: ExprKind::ArrayClone(Box::new(receiver)),
            ty,
            span,
        })
    }
}
