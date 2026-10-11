use super::*;

impl ConstructorReferenceDeclaration {
    pub(in crate::expr::callable_references) fn commit(
        self,
        state: &mut Lowerer,
        arguments: &[TypeId],
        ty: TypeId,
    ) -> Result<hir::Expr, String> {
        let Type::Function(function) = state.types[ty] else {
            unreachable!("a constructor reference has a complete function type");
        };
        let signature = state.function_types[function].clone();
        let arguments = if let Some(targets) = &self.alias_arguments {
            let bindings = self
                .signature
                .owner_parameters
                .iter()
                .chain(&self.signature.callable_parameters)
                .map(|parameter| parameter.id)
                .zip(arguments.iter().copied())
                .collect::<Vec<_>>();
            targets
                .iter()
                .map(|ty| state.instantiate_method_ty(*ty, &bindings))
                .collect()
        } else {
            self.fixed_arguments.unwrap_or_else(|| arguments.to_vec())
        };
        let span = self.span;
        let generate = |state: &mut Lowerer, binding: Option<hir::BindingId>| {
            state.lower_synthesized_lambda(
                false,
                &signature.parameter_types,
                signature.return_type,
                span,
                |state, mut args| {
                    let receiver = if let Some(binding) = binding {
                        let outer = state.lower_capture_binding(binding, "$outer", span)?;
                        let static_type = outer.ty;
                        args.insert(0, outer);
                        hir::SourceCallReceiver::Receiver { static_type }
                    } else {
                        hir::SourceCallReceiver::NoReceiver
                    };
                    let value = self.target.construct(
                        state,
                        arguments,
                        args,
                        receiver,
                        signature.return_type,
                        span,
                    )?;
                    Some(crate::stmt::ValueBlock {
                        statements: Vec::new(),
                        value: Some(value),
                    })
                },
            )
        };
        let value = if let Some((outer, _)) = self.enclosing {
            // A normal lambda parameter snapshots the receiver before the
            // returned adapter captures it, including reads of mutable locals.
            let factory =
                state.lower_synthesized_lambda(false, &[outer.ty], ty, span, |state, values| {
                    let ExprKind::Local(local) = values[0].kind else {
                        unreachable!("a synthesized parameter is a local binding");
                    };
                    let value = generate(state, Some(state.locals[local].binding))?;
                    Some(crate::stmt::ValueBlock {
                        statements: Vec::new(),
                        value: Some(value),
                    })
                });
            factory.map(|callee| {
                let Type::Function(function_type) = state.types[callee.ty] else {
                    unreachable!("a constructor binding factory is a function value");
                };
                hir::Expr {
                    kind: ExprKind::CallableCall {
                        callee: Box::new(callee),
                        function_type,
                        args: vec![outer],
                    },
                    ty,
                    span,
                    origin: state.expression_origin(span),
                }
            })
        } else {
            generate(state, None)
        };
        value.ok_or_else(|| "constructor reference adapter could not be typed".into())
    }
}

impl ConstructorReferenceTarget {
    fn construct(
        self,
        state: &mut Lowerer,
        type_args: Vec<TypeId>,
        args: Vec<hir::Expr>,
        receiver: hir::SourceCallReceiver<TypeId>,
        ty: TypeId,
        span: Span,
    ) -> Option<hir::Expr> {
        let kind = match self {
            Self::Local(source) => {
                return state.finish_named_nominal(
                    crate::constructor_resolution::ResolvedNominalConstructor {
                        source,
                        type_args,
                        args,
                    },
                    span,
                );
            }
            Self::Dependency(declaration) => {
                let callee = match state.select_imported_callable_declaration_use_with_kind(
                    *declaration,
                    MemberCallKind::Ordinary,
                ) {
                    Ok(callee) => callee,
                    Err(error) => {
                        state.error(
                            span,
                            format!("failed to select imported constructor reference: {error}"),
                        );
                        return None;
                    }
                };
                ExprKind::Call {
                    callee: hir::CallableTarget::Dependency(callee),
                    binding: None,
                    args,
                    receiver,
                }
            }
            Self::Imported(template) => {
                match state.constructor_template_application(template, ty) {
                    hir::ConstructorApplicationRef::Class(constructor) => {
                        ExprKind::ClassInit { constructor, args }
                    }
                    hir::ConstructorApplicationRef::Struct(constructor) => {
                        ExprKind::StructInit { constructor, args }
                    }
                }
            }
        };
        Some(hir::Expr {
            kind,
            ty,
            span,
            origin: state.expression_origin(span),
        })
    }
}
