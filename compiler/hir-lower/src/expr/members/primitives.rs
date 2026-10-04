use super::*;

impl Lowerer {
    pub(crate) fn normalize_primitive_method_call(
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
        let kind = match intrinsic.kind {
            hir::IntrinsicFunctionKind::Char(kind) => {
                return Some(self.normalize_char_method_call(kind, receiver, args, ty, span));
            }
            hir::IntrinsicFunctionKind::Integer(kind) => {
                return Some(self.normalize_integer_method_call(kind, receiver, args, ty, span));
            }
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

    pub(in crate::expr) fn normalize_integer_method_call(
        &mut self,
        intrinsic: hir::IntegerIntrinsicKind,
        receiver: hir::Expr,
        args: &[hir::Expr],
        ty: TypeId,
        span: Span,
    ) -> hir::Expr {
        let kind = match intrinsic {
            hir::IntegerIntrinsicKind::NoGcOperation { kind, operation } => {
                ExprKind::IntegerOperation {
                    operation: hir::IntegerOperation::NoGc { kind, operation },
                    arguments: integer_operation_arguments(operation.arity(), receiver, args),
                }
            }
            hir::IntegerIntrinsicKind::ManagedOperation { kind, operation } => {
                if let Err(error) = self.prepare_arithmetic_exception_type() {
                    self.error(span, error.diagnostic("integer division exception type"));
                }
                ExprKind::IntegerOperation {
                    operation: hir::IntegerOperation::Managed { kind, operation },
                    arguments: integer_operation_arguments(
                        hir::IntegerOperationArity::Binary,
                        receiver,
                        args,
                    ),
                }
            }
            hir::IntegerIntrinsicKind::Conversion {
                source,
                target_kind,
            } => {
                debug_assert!(args.is_empty());
                ExprKind::IntegerConversion {
                    conversion: hir::IntegerConversion {
                        source,
                        target_kind,
                    },
                    operand: Box::new(receiver),
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

fn integer_operation_arguments(
    arity: hir::IntegerOperationArity,
    receiver: hir::Expr,
    args: &[hir::Expr],
) -> hir::HirIntegerOperationArguments {
    match arity {
        hir::IntegerOperationArity::Unary => {
            debug_assert!(args.is_empty());
            hir::HirIntegerOperationArguments::Unary(Box::new(receiver))
        }
        hir::IntegerOperationArity::Binary => {
            let [rhs] = args else {
                unreachable!("validated binary integer intrinsic has one argument")
            };
            hir::HirIntegerOperationArguments::Binary {
                lhs: Box::new(receiver),
                rhs: Box::new(rhs.clone()),
            }
        }
    }
}
