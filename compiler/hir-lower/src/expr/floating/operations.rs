use super::*;

impl Lowerer {
    pub(in crate::expr) fn lower_float_comparison(
        &mut self,
        kind: hir::FloatKind,
        comparison: hir::BinOp,
        lhs: hir::Expr,
        rhs: &ast::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let ty = lhs.ty;
        let (lhs, rhs) = self.lower_equality_rhs(lhs, rhs, sink, Some(ty))?;
        if !self.types_equal(lhs.ty, rhs.ty) {
            self.error(
                span,
                format!(
                    "floating comparison requires matching operand types, found {} and {}",
                    self.type_name(lhs.ty),
                    self.type_name(rhs.ty)
                ),
            );
            return None;
        }
        let operation = match comparison {
            hir::BinOp::Lt => hir::FloatBinaryOperator::Less,
            hir::BinOp::Le => hir::FloatBinaryOperator::LessEqual,
            hir::BinOp::Gt => hir::FloatBinaryOperator::Greater,
            hir::BinOp::Ge => hir::FloatBinaryOperator::GreaterEqual,
            _ => unreachable!("ordered comparison has one of four source operators"),
        };
        Some(hir::Expr {
            kind: ExprKind::FloatBinary {
                kind,
                operation,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            ty: self.boolean,
            span,
            origin: self.expression_origin(span),
        })
    }

    pub(in crate::expr) fn normalize_float_method_call(
        &mut self,
        intrinsic: hir::FloatIntrinsicKind,
        receiver: hir::Expr,
        args: &[hir::Expr],
        ty: TypeId,
        span: Span,
    ) -> hir::Expr {
        let kind = match intrinsic {
            hir::FloatIntrinsicKind::Unary { kind, operation } => ExprKind::FloatUnary {
                kind,
                operation,
                operand: Box::new(receiver),
            },
            hir::FloatIntrinsicKind::Binary { kind, operation } => {
                let [rhs] = args else {
                    unreachable!("validated float binary member has one argument")
                };
                ExprKind::FloatBinary {
                    kind,
                    operation,
                    lhs: Box::new(receiver),
                    rhs: Box::new(rhs.clone()),
                }
            }
            hir::FloatIntrinsicKind::Conversion(conversion) => ExprKind::FloatConversion {
                conversion,
                operand: Box::new(receiver),
            },
        };
        hir::Expr {
            kind,
            ty,
            span,
            origin: self.expression_origin(span),
        }
    }
}
