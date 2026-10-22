//! Consume intrinsic identities selected by concrete constraint resolution.

use super::*;

impl Concretizer<'_> {
    pub(super) fn normalize_concrete_intrinsic(
        &mut self,
        mut expression: concrete::Expr,
    ) -> concrete::Expr {
        use concrete::ExprKind as E;
        use export::IntrinsicFunctionKind as I;
        let (callee, receiver, args) = match &expression.kind {
            E::Call { callee, args, .. } => (*callee, None, args.as_slice()),
            E::MethodCall {
                callee,
                receiver,
                args,
            }
            | E::DirectSuperMethodCall {
                callee,
                receiver,
                args,
            } => (
                concrete::CallableTarget::Local(*callee),
                Some(receiver.as_ref()),
                args.as_slice(),
            ),
            _ => return expression,
        };
        let Some(intrinsic) = self.concrete_intrinsic(callee) else {
            return expression;
        };
        if !matches!(
            intrinsic,
            I::Integer(_) | I::Float(_) | I::Char(_) | I::PrimitiveUnary(_) | I::PrimitiveBinary(_)
        ) {
            return expression;
        }
        let arguments = receiver
            .into_iter()
            .chain(args)
            .cloned()
            .collect::<Vec<_>>();
        let mut arguments = arguments.into_iter();
        let lhs = Box::new(
            arguments
                .next()
                .expect("a primitive intrinsic has a receiver"),
        );
        let mut rhs = || {
            Box::new(
                arguments
                    .next()
                    .expect("a binary intrinsic has two operands"),
            )
        };
        expression.kind = match intrinsic {
            I::Integer(export::IntegerIntrinsicKind::Conversion {
                source,
                target_kind,
            }) => E::IntegerConversion {
                conversion: export::IntegerConversion {
                    source,
                    target_kind,
                },
                operand: lhs,
            },
            I::Integer(operation) => {
                let descriptor = match operation {
                    export::IntegerIntrinsicKind::NoGcOperation { kind, operation } => {
                        export::IntegerOperation::NoGc { kind, operation }
                    }
                    export::IntegerIntrinsicKind::ManagedOperation { kind, operation } => {
                        self.lower_arithmetic_exception_type();
                        export::IntegerOperation::Managed { kind, operation }
                    }
                    export::IntegerIntrinsicKind::Conversion { .. } => unreachable!(),
                };
                let kind = descriptor.kind();
                E::IntegerOperation(match descriptor.classify() {
                    export::IntegerOperator::Unary(operation) => export::IntegerExpression::Unary {
                        kind,
                        operation,
                        operand: lhs,
                    },
                    export::IntegerOperator::Binary(operation) => {
                        export::IntegerExpression::Binary {
                            kind,
                            operation,
                            lhs,
                            rhs: rhs(),
                        }
                    }
                })
            }
            I::Float(export::FloatIntrinsicKind::Unary { kind, operation }) => E::FloatUnary {
                kind,
                operation,
                operand: lhs,
            },
            I::Float(export::FloatIntrinsicKind::Binary { kind, operation }) => E::FloatBinary {
                kind,
                operation,
                lhs,
                rhs: rhs(),
            },
            I::Float(export::FloatIntrinsicKind::Conversion(conversion)) => E::FloatConversion {
                conversion,
                operand: lhs,
            },
            I::Char(export::CharIntrinsic::Code) => E::CharCode(lhs),
            I::Char(export::CharIntrinsic::FromCodeUnchecked) => E::CharFromCodeUnchecked(lhs),
            I::Char(kind) => {
                let integer = export::IntegerKind::SIGNED_32;
                let ty = self.intern_type(concrete::TypeKind::Integer(integer), true);
                let code = |operand: Box<concrete::Expr>| {
                    Box::new(concrete::Expr {
                        span: operand.span,
                        origin: operand.origin,
                        ty,
                        kind: E::CharCode(operand),
                    })
                };
                E::IntegerOperation(export::IntegerExpression::Binary {
                    kind: integer,
                    operation: if kind == export::CharIntrinsic::Equals {
                        export::IntegerBinaryOperator::Equals
                    } else {
                        export::IntegerBinaryOperator::CompareTo
                    },
                    lhs: code(lhs),
                    rhs: code(rhs()),
                })
            }
            I::PrimitiveUnary(kind) => E::PrimitiveUnary { kind, operand: lhs },
            I::PrimitiveBinary(kind) => E::PrimitiveBinary {
                kind,
                lhs,
                rhs: rhs(),
            },
            _ => unreachable!("only primitive intrinsics are normalized here"),
        };
        assert!(
            arguments.next().is_none(),
            "intrinsic arity was resolved before concretization"
        );
        expression
    }

    pub(super) fn concrete_intrinsic(
        &self,
        target: concrete::CallableTarget,
    ) -> Option<export::IntrinsicFunctionKind> {
        match target {
            concrete::CallableTarget::Imported(callee) => {
                let source = self.imported_dependency_callables[callee];
                if source.dispatch() != export::ImportedDependencyDispatch::Direct {
                    return None;
                }
                let callable = self
                    .dependencies
                    .callable(source.reference().declaration())?;
                match callable.record().effects().implementation() {
                    export::CallableImplementationV1::Intrinsic(kind) => Some(kind),
                    _ => None,
                }
            }
            concrete::CallableTarget::Local(concrete::Callable::Function(function)) => {
                let key = self
                    .function_keys
                    .get(function.into_raw().into_u32() as usize)?;
                let implementation = match self.function_source(key) {
                    FunctionSource::Local(id) => &self.source.functions[id].kind,
                    FunctionSource::Imported(id) => {
                        &self.source.imported_generic_templates[id].implementation
                    }
                    _ => return None,
                };
                match implementation {
                    export::FunctionKind::Intrinsic(intrinsic) => Some(intrinsic.kind),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}
