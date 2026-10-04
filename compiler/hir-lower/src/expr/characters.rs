//! Normalize selected Char methods without losing the source nominal type.

use super::*;

impl Lowerer {
    pub(in crate::expr) fn normalize_char_method_call(
        &mut self,
        intrinsic: hir::CharIntrinsic,
        receiver: hir::Expr,
        args: &[hir::Expr],
        ty: TypeId,
        span: Span,
    ) -> hir::Expr {
        let kind = match intrinsic {
            hir::CharIntrinsic::Code => ExprKind::CharCode(Box::new(receiver)),
            hir::CharIntrinsic::FromCodeUnchecked => {
                ExprKind::CharFromCodeUnchecked(Box::new(receiver))
            }
            hir::CharIntrinsic::Equals | hir::CharIntrinsic::CompareTo => {
                let [rhs] = args else {
                    unreachable!("validated Char comparison has one operand");
                };
                let int = self.integer_type(hir::IntegerKind::SIGNED_32);
                let code = |operand: hir::Expr| hir::Expr {
                    origin: operand.origin,
                    span: operand.span,
                    ty: int,
                    kind: ExprKind::CharCode(Box::new(operand)),
                };
                ExprKind::IntegerOperation {
                    operation: hir::IntegerOperation::NoGc {
                        kind: hir::IntegerKind::SIGNED_32,
                        operation: if intrinsic == hir::CharIntrinsic::Equals {
                            hir::NoGcIntegerOperation::Equals
                        } else {
                            hir::NoGcIntegerOperation::CompareTo
                        },
                    },
                    arguments: hir::HirIntegerOperationArguments::Binary {
                        lhs: Box::new(code(receiver)),
                        rhs: Box::new(code(rhs.clone())),
                    },
                }
            }
        };
        hir::Expr {
            kind,
            ty,
            span,
            origin: self.expression_origin(span),
        }
    }
}
