use super::*;

mod imported;
mod registration;
mod signature;

impl Lowerer {
    pub(crate) fn foreign_callback_argument_expected(
        &mut self,
        function: hir::FunctionId,
        explicit_type_args: &[ResolvedCallTypeArgument],
        argument_map: &crate::call_resolution::arguments::CandidateArgumentMap,
        args: &[ast::CallArgument],
        span: Span,
    ) -> Result<Option<(usize, TypeId)>, ()> {
        if !self
            .foreign_callback_core
            .is_some_and(|core| function == core.register)
        {
            return Ok(None);
        }
        use crate::call_resolution::arguments::ResolvedParameterInput;
        let ResolvedParameterInput::Explicit(context) = argument_map.parameters[1].input else {
            unreachable!("the callback context index is required");
        };
        let ResolvedParameterInput::Explicit(callback) = argument_map.parameters[0].input else {
            unreachable!("the callback closure is required");
        };
        self.foreign_callback_expected(
            explicit_type_args,
            &args[context.index()].expression,
            callback.index(),
            span,
        )
        .map(Some)
    }

    pub(super) fn lower_foreign_callback_registration(
        &mut self,
        core: hir::ForeignCallbackCore,
        function: hir::FunctionId,
        call: &ast::CallExpr,
        resolved: crate::overload::ResolvedCallee,
        sink: &[hir::Statement],
    ) -> Option<hir::Expr> {
        debug_assert_eq!(function, core.register);
        self.check_call_effects(hir::Callable::Function(function), call.span);
        self.normalize_foreign_callback_registration(
            resolved.type_args[0],
            resolved.args,
            resolved.return_ty,
            call.span,
            sink,
        )
    }

    pub(super) fn lower_foreign_callback_call(
        &mut self,
        core: hir::ForeignCallbackCore,
        function: hir::FunctionId,
        operation: hir::ForeignCallbackOperation,
        call: &ast::CallExpr,
        resolved: crate::overload::ResolvedCallee,
    ) -> Option<hir::Expr> {
        let expected = match operation {
            hir::ForeignCallbackOperation::Retain => core.retain,
            hir::ForeignCallbackOperation::Release => core.release,
            hir::ForeignCallbackOperation::State => core.query_state,
            hir::ForeignCallbackOperation::Failure => core.failure,
        };
        debug_assert_eq!(function, expected);
        self.check_call_effects(hir::Callable::Function(function), call.span);
        self.normalize_foreign_callback_operation(
            operation,
            resolved.type_args[0],
            resolved.args,
            resolved.return_ty,
            call.span,
        )
    }

    pub(in crate::expr) fn normalize_foreign_callback_operation(
        &mut self,
        operation: hir::ForeignCallbackOperation,
        native_type: hir::TypeId,
        arguments: Vec<hir::Expr>,
        result_type: hir::TypeId,
        span: Span,
    ) -> Option<hir::Expr> {
        let [callback]: [hir::Expr; 1] = arguments
            .try_into()
            .expect("a callback operation has one argument");
        if !matches!(self.types[native_type], hir::Type::Function(_)) {
            self.error(
                callback.span,
                "`ForeignCallback` type argument must be one concrete function type".into(),
            );
            return None;
        }
        Some(hir::Expr {
            kind: ExprKind::ForeignCallbackOperation {
                operation,
                callback: Box::new(callback),
            },
            ty: result_type,
            span,
            origin: self.expression_origin(span),
        })
    }
}

fn materialized_source<'expr>(
    expr: &'expr hir::Expr,
    sink: &'expr [hir::Statement],
) -> &'expr hir::Expr {
    let mut current = expr;
    while let ExprKind::Local(local) = current.kind {
        let Some(init) = sink.iter().find_map(|statement| {
            let hir::StatementKind::ValDecl { pattern, init } = &statement.kind else {
                return None;
            };
            matches!(pattern, hir::Pattern::Binding { local: bound } if *bound == local)
                .then_some(init)
        }) else {
            break;
        };
        current = init;
    }
    current
}
