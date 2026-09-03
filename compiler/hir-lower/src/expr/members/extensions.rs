use super::*;

impl Lowerer {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::expr) fn finish_extension_call(
        &mut self,
        candidates: &[hir::FunctionId],
        name: &str,
        receiver: hir::Expr,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
        operator_set: bool,
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
                argument_protocol: if operator_set {
                    crate::overload::CallArgumentProtocol::OperatorSet
                } else {
                    crate::overload::CallArgumentProtocol::Ordinary
                },
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
            origin: self.expression_origin(call.span),
        })
    }

    /// Normalize a winning core `m.toArray()` / `a.toMutableArray()` target
    /// after ordinary member applicability and specificity have completed.
    pub(in crate::expr) fn normalize_array_method_call(
        &self,
        function: hir::FunctionId,
        receiver: hir::Expr,
        args: &[hir::Expr],
        ty: TypeId,
        span: Span,
    ) -> Option<hir::Expr> {
        let hir::FunctionKind::Intrinsic(intrinsic) = self.functions[function].kind else {
            return None;
        };
        let kind = match intrinsic.kind {
            hir::IntrinsicFunctionKind::Array(kind) => {
                debug_assert!(args.is_empty());
                let (source_kind, target_kind) = match kind {
                    hir::ArrayIntrinsic::ToImmutable => (ArrayKind::Mutable, ArrayKind::Immutable),
                    hir::ArrayIntrinsic::ToMutable => (ArrayKind::Immutable, ArrayKind::Mutable),
                };
                let source = self
                    .array_type_info(receiver.ty)
                    .expect("validated array intrinsic has an array receiver");
                let target = self
                    .array_type_info(ty)
                    .expect("validated array intrinsic has an array result");
                debug_assert_eq!(source.kind, source_kind);
                debug_assert_eq!(target.kind, target_kind);
                debug_assert!(self.types_equal(source.element, target.element));
                ExprKind::ArrayClone(Box::new(receiver))
            }
            hir::IntrinsicFunctionKind::ArrayAccess(access) => match (access, args) {
                (
                    hir::ArrayAccessKind::ImmutableGet | hir::ArrayAccessKind::MutableGet,
                    [index],
                ) => ExprKind::Index {
                    access,
                    receiver: Box::new(receiver),
                    index: Box::new(index.clone()),
                },
                (hir::ArrayAccessKind::MutableSet, [index, value]) => ExprKind::ArraySet {
                    access,
                    receiver: Box::new(receiver),
                    index: Box::new(index.clone()),
                    value: Box::new(value.clone()),
                },
                _ => unreachable!("validated array access intrinsic has a fixed argument shape"),
            },
            _ => return None,
        };
        Some(hir::Expr {
            kind,
            ty,
            span,
            origin: self.expression_origin(span),
        })
    }
}
