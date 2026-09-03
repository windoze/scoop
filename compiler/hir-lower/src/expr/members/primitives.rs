use super::*;

impl Lowerer {
    pub(in crate::expr) fn normalize_primitive_method_call(
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
            hir::IntrinsicFunctionKind::PrimitiveUnary(kind) => {
                debug_assert!(args.is_empty());
                ExprKind::PrimitiveUnary {
                    kind,
                    operand: Box::new(receiver),
                }
            }
            hir::IntrinsicFunctionKind::PrimitiveBinary(kind) => {
                let [argument] = args else {
                    unreachable!("validated primitive binary intrinsic has one argument")
                };
                ExprKind::PrimitiveBinary {
                    kind,
                    lhs: Box::new(receiver),
                    rhs: Box::new(argument.clone()),
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
