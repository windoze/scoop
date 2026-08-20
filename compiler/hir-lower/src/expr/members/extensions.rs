use super::*;

impl Lowerer {
    /// Normalize a winning core `m.toArray()` / `a.toMutableArray()` target
    /// after ordinary member applicability and specificity have completed.
    pub(in crate::expr) fn normalize_array_method_call(
        &mut self,
        function: hir::FunctionId,
        receiver: hir::Expr,
        args: &[hir::Expr],
        ty: TypeId,
        span: Span,
    ) -> Option<hir::Expr> {
        let hir::FunctionKind::Intrinsic(intrinsic) = self.functions[function].kind else {
            return None;
        };
        self.normalize_array_intrinsic_call(intrinsic.kind, receiver, args, ty, span)
    }

    pub(in crate::expr) fn normalize_array_intrinsic_call(
        &mut self,
        intrinsic: hir::IntrinsicFunctionKind,
        receiver: hir::Expr,
        args: &[hir::Expr],
        ty: TypeId,
        span: Span,
    ) -> Option<hir::Expr> {
        let kind = match intrinsic {
            hir::IntrinsicFunctionKind::Array(
                hir::ArrayIntrinsic::ImmutableLength | hir::ArrayIntrinsic::MutableLength,
            ) => {
                debug_assert!(args.is_empty());
                ExprKind::ArrayLen(Box::new(receiver))
            }
            hir::IntrinsicFunctionKind::Array(kind) => {
                debug_assert!(args.is_empty());
                let (source_kind, target_kind) = match kind {
                    hir::ArrayIntrinsic::ToImmutable => (ArrayKind::Mutable, ArrayKind::Immutable),
                    hir::ArrayIntrinsic::ToMutable => (ArrayKind::Immutable, ArrayKind::Mutable),
                    hir::ArrayIntrinsic::ImmutableLength | hir::ArrayIntrinsic::MutableLength => {
                        unreachable!("length is handled above")
                    }
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
            hir::IntrinsicFunctionKind::ArrayAccess(access) => {
                if let Err(error) = self.prepare_array_bounds_exception_type() {
                    self.error(span, error.diagnostic("array bounds exception type"));
                    return None;
                }
                match (access, args) {
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
                    _ => {
                        unreachable!("validated array access intrinsic has a fixed argument shape")
                    }
                }
            }
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
