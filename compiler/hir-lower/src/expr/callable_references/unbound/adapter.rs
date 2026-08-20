use super::*;

impl Lowerer {
    pub(in crate::expr::callable_references) fn lower_unbound_reference_adapter(
        &mut self,
        ty: TypeId,
        span: Span,
        bind: impl FnOnce(&mut Self, &hir::Expr, TypeId) -> Result<ReferenceTarget, String>,
    ) -> Result<hir::Expr, String> {
        let Type::Function(function) = self.types[ty] else {
            unreachable!("an unbound reference has a complete function signature");
        };
        let signature = self.function_types[function].clone();
        let bound = self.intern_function_type(
            signature.is_suspend,
            signature.parameter_types[1..].to_vec(),
            signature.return_type,
        );
        let bound_ty = self.function_types[bound].canonical_type;
        let mut failure = None;
        let value = self.lower_synthesized_lambda(
            signature.is_suspend,
            &signature.parameter_types,
            signature.return_type,
            span,
            |state, values| {
                let mut values = values.into_iter();
                let receiver = values
                    .next()
                    .expect("an unbound reference has a receiver parameter");
                let target = match bind(state, &receiver, bound_ty) {
                    Ok(ReferenceTarget::Callable(target)) => target,
                    Ok(ReferenceTarget::Value(_)) => {
                        unreachable!("binding a member selects a callable")
                    }
                    Err(message) => {
                        failure = Some(message);
                        return None;
                    }
                };
                let args = values.collect::<Vec<_>>();
                let kind = match target {
                    hir::CallableReferenceTarget::BoundMember { receiver, callee } => {
                        ExprKind::MethodCall {
                            receiver,
                            callee,
                            args,
                        }
                    }
                    hir::CallableReferenceTarget::BoundExtension { receiver, callee } => {
                        let receiver_ty = receiver.ty;
                        ExprKind::Call {
                            callee,
                            binding: None,
                            args: std::iter::once(*receiver).chain(args).collect(),
                            receiver: hir::SourceCallReceiver::Receiver {
                                static_type: receiver_ty,
                            },
                        }
                    }
                    hir::CallableReferenceTarget::BoundIntrinsic {
                        receiver,
                        intrinsic,
                        ..
                    } => {
                        let value = state.unbound_intrinsic_call(
                            intrinsic,
                            *receiver,
                            args,
                            signature.return_type,
                            span,
                        );
                        return Some(crate::stmt::ValueBlock {
                            statements: Vec::new(),
                            value: Some(value),
                        });
                    }
                    hir::CallableReferenceTarget::Named(_)
                    | hir::CallableReferenceTarget::Local { .. } => {
                        unreachable!("an unbound member adapter binds its receiver")
                    }
                };
                Some(crate::stmt::ValueBlock {
                    statements: Vec::new(),
                    value: Some(hir::Expr {
                        kind,
                        ty: signature.return_type,
                        span,
                        origin: state.expression_origin(span),
                    }),
                })
            },
        );
        value.ok_or_else(|| {
            failure.unwrap_or_else(|| "unbound reference adapter could not be typed".into())
        })
    }

    fn unbound_intrinsic_call(
        &mut self,
        intrinsic: hir::PrimitiveMemberIntrinsic,
        receiver: hir::Expr,
        args: Vec<hir::Expr>,
        ty: TypeId,
        span: Span,
    ) -> hir::Expr {
        use hir::PrimitiveMemberIntrinsic as Intrinsic;
        let kind = match intrinsic {
            Intrinsic::Float(kind) => {
                return self.normalize_float_method_call(kind, receiver, &args, ty, span);
            }
            Intrinsic::Char(kind) => {
                return self.normalize_char_method_call(kind, receiver, &args, ty, span);
            }
            Intrinsic::Integer(kind) => {
                return self.normalize_integer_method_call(kind, receiver, &args, ty, span);
            }
            Intrinsic::Unary(kind) => {
                assert!(args.is_empty(), "a unary member has no value parameters");
                ExprKind::PrimitiveUnary {
                    kind,
                    operand: Box::new(receiver),
                }
            }
            Intrinsic::Binary(kind) => {
                let [rhs]: [hir::Expr; 1] = args
                    .try_into()
                    .expect("a binary member has one value parameter");
                ExprKind::PrimitiveBinary {
                    kind,
                    lhs: Box::new(receiver),
                    rhs: Box::new(rhs),
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
