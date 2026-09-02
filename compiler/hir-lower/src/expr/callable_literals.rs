use super::*;

impl Lowerer {
    pub(super) fn lower_lambda(
        &mut self,
        is_suspend: bool,
        parameters: Option<&[ast::LambdaParam]>,
        body: &ast::Block,
        span: Span,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        if block_contains_return(body) {
            self.error(
                span,
                "a lambda cannot use `return`; use an anonymous function for local returns"
                    .to_string(),
            );
            return None;
        }
        let expected_signature = expected.and_then(|ty| match self.types[ty] {
            Type::Function(id) => Some((ty, self.function_types[id].clone())),
            _ => None,
        });
        if let Some((_, signature)) = &expected_signature
            && signature.is_suspend != is_suspend
        {
            self.error(
                span,
                "ordinary and suspend function types are incompatible".to_string(),
            );
            return None;
        }
        let source_parameters: Vec<Option<&ast::LambdaParam>> = match parameters {
            Some(parameters) => parameters.iter().map(Some).collect(),
            None => match &expected_signature {
                Some((_, signature)) if signature.parameter_types.len() == 1 => vec![None],
                Some((_, signature)) if signature.parameter_types.is_empty() => Vec::new(),
                Some((_, signature)) => {
                    self.error(
                        span,
                        format!(
                            "lambda omits its parameter list, but the expected type has {} parameters",
                            signature.parameter_types.len()
                        ),
                    );
                    return None;
                }
                None => Vec::new(),
            },
        };
        if let Some((_, signature)) = &expected_signature
            && source_parameters.len() != signature.parameter_types.len()
        {
            self.error(
                span,
                format!(
                    "lambda has {} parameter(s), but the expected function type has {}",
                    source_parameters.len(),
                    signature.parameter_types.len()
                ),
            );
            return None;
        }

        let capture_environment = self.capture_environment();
        let outer_locals = std::mem::take(&mut self.locals);
        let outer_scopes = std::mem::replace(&mut self.scopes, Scopes::new());
        let outer_return_ty = self.current_return_ty;
        let outer_fn_name = std::mem::take(&mut self.current_fn_name);
        let outer_owner = self.current_owner;
        let outer_this = self.current_this.take();
        let outer_smart_casts = std::mem::take(&mut self.smart_casts);
        self.capture_contexts.push(CaptureContext {
            available: capture_environment,
            captures: Vec::new(),
            by_binding: std::collections::HashMap::new(),
        });
        let function_number = self.next_lambda_function;
        self.next_lambda_function += 1;
        self.current_fn_name = format!("$lambda.{function_number}");
        self.push_suspension_context(if is_suspend {
            SuspensionContext::SuspendFunction
        } else {
            SuspensionContext::Forbidden(ForbiddenSuspendContext::Function)
        });
        self.push_safety_context(hir::Safety::Safe);
        self.push_scope();

        let lowered = (|| {
            let mut abi_params = Vec::with_capacity(source_parameters.len());
            let mut parameter_types = Vec::with_capacity(source_parameters.len());
            let mut prefix = Vec::new();
            for (index, parameter) in source_parameters.iter().enumerate() {
                let expected_ty = expected_signature
                    .as_ref()
                    .map(|(_, signature)| signature.parameter_types[index]);
                let explicit_ty = match parameter.and_then(|parameter| parameter.ty.as_ref()) {
                    Some(ty) => Some(self.resolve_type_ref(ty)?),
                    None => None,
                };
                let parameter_ty = match (explicit_ty, expected_ty) {
                    (Some(explicit), Some(expected)) => {
                        if !self.types_equal(explicit, expected) {
                            let found = self.type_name(explicit);
                            let expected = self.type_name(expected);
                            let at = parameter
                                .expect("an explicit type belongs to a parameter")
                                .span;
                            self.error(
                                at,
                                format!(
                                    "lambda parameter type is {found}, but the expected type is {expected}"
                                ),
                            );
                            return None;
                        }
                        explicit
                    }
                    (Some(explicit), None) => explicit,
                    (None, Some(expected)) => expected,
                    (None, None) => {
                        let at = parameter.map_or(span, |parameter| parameter.span);
                        self.error(
                            at,
                            "lambda parameter requires a type when there is no expected function type"
                                .to_string(),
                        );
                        return None;
                    }
                };
                parameter_types.push(parameter_ty);
                let target = parameter.map(|parameter| &parameter.target);
                let binding_name = match target {
                    Some(ast::Pattern::Binding(name)) => Some(name.clone()),
                    None => Some(ast::Ident {
                        text: "it".to_string(),
                        span,
                    }),
                    _ => None,
                };
                if let Some(name) = binding_name {
                    let pattern = ast::Pattern::Binding(name.clone());
                    let hir::Pattern::Binding { local } = self.lower_pattern(
                        &pattern,
                        parameter_ty,
                        PatternCtx {
                            mutable: false,
                            in_when: false,
                        },
                    )?
                    else {
                        unreachable!("a binding parameter lowers to a binding")
                    };
                    abi_params.push(hir::Param {
                        name: name.text,
                        ty: parameter_ty,
                        local,
                    });
                } else {
                    let local = self.alloc_local(format!("$arg.{index}"), parameter_ty, false);
                    let pattern = self.lower_pattern(
                        target.expect("non-binding source parameter has a pattern"),
                        parameter_ty,
                        PatternCtx {
                            mutable: false,
                            in_when: false,
                        },
                    )?;
                    prefix.push(hir::Statement {
                        kind: hir::StatementKind::ValDecl {
                            pattern,
                            init: hir::Expr {
                                kind: ExprKind::Local(local),
                                ty: parameter_ty,
                                span,
                            },
                        },
                        span,
                    });
                    abi_params.push(hir::Param {
                        name: format!("$arg.{index}"),
                        ty: parameter_ty,
                        local,
                    });
                }
            }

            let expected_return = expected_signature
                .as_ref()
                .map(|(_, signature)| signature.return_type);
            let mut value_block = self.lower_value_block(body, expected_return)?;
            let return_ty = value_block
                .value
                .as_ref()
                .map_or(expected_return.unwrap_or(self.unit), |value| value.ty);
            if let Some(expected_return) = expected_return
                && !self.types_equal(expected_return, self.unit)
                && !self.types_equal(return_ty, expected_return)
            {
                let expected = self.type_name(expected_return);
                let found = self.type_name(return_ty);
                self.error(
                    body.span,
                    format!("lambda result must be of type {expected}, found {found}"),
                );
                return None;
            }
            let return_ty = expected_return.unwrap_or(return_ty);
            self.current_return_ty = return_ty;
            prefix.append(&mut value_block.statements);
            if let Some(value) = value_block.value.take() {
                if self.types_equal(return_ty, self.unit) {
                    if !matches!(value.kind, ExprKind::UnitLiteral) {
                        prefix.push(hir::Statement {
                            span: value.span,
                            kind: hir::StatementKind::Expr(value),
                        });
                    }
                    prefix.push(hir::Statement {
                        span: body.span,
                        kind: hir::StatementKind::Return { value: None },
                    });
                } else {
                    let value = self.adapt_to(value, return_ty);
                    prefix.push(hir::Statement {
                        span: value.span,
                        kind: hir::StatementKind::Return { value: Some(value) },
                    });
                }
            }
            let function_ty = self.intern_function_type(is_suspend, parameter_types, return_ty);
            let Type::Function(function_type) = self.types[function_ty] else {
                unreachable!()
            };
            let closure_local = self.alloc_local("$closure".to_string(), function_ty, false);
            let mut params = Vec::with_capacity(abi_params.len() + 1);
            params.push(hir::Param {
                name: "$closure".to_string(),
                ty: function_ty,
                local: closure_local,
            });
            params.extend(abi_params);
            let type_params = self.type_params_in_scope.clone();
            let function = self.functions.alloc(hir::Function {
                name: self.current_fn_name.clone(),
                genericity: hir::FunctionGenericity::Plain,
                is_suspend,
                params,
                return_ty,
                attributes: hir::FunctionAttributes::default(),
                kind: hir::FunctionKind::User(hir::Body {
                    locals: std::mem::take(&mut self.locals),
                    statements: prefix,
                }),
                method: None,
                span,
            });
            if !type_params.is_empty() {
                self.register_generic(function, type_params.clone());
            }
            let captures = self.finish_current_captures();
            let id = self.lambdas.alloc(hir::Lambda {
                function,
                function_type,
                owner_type_param_count: type_params.len(),
                captures,
                span,
            });
            Some(hir::Expr {
                kind: ExprKind::Lambda(id),
                ty: function_ty,
                span,
            })
        })();

        self.pop_scope();
        self.pop_safety_context();
        self.pop_suspension_context();
        self.capture_contexts.pop();
        self.locals = outer_locals;
        self.scopes = outer_scopes;
        self.current_return_ty = outer_return_ty;
        self.current_fn_name = outer_fn_name;
        self.current_owner = outer_owner;
        self.current_this = outer_this;
        self.smart_casts = outer_smart_casts;
        lowered
    }

    pub(super) fn lower_anonymous_function(
        &mut self,
        is_suspend: bool,
        source_params: &[ast::Param],
        source_return_ty: Option<&ast::TypeRef>,
        body: &ast::Block,
        span: Span,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let expected_signature = expected.and_then(|ty| match self.types[ty] {
            Type::Function(id) => Some((ty, self.function_types[id].clone())),
            _ => None,
        });
        if let Some((_, signature)) = &expected_signature {
            if signature.is_suspend != is_suspend {
                self.error(
                    span,
                    "ordinary and suspend function types are incompatible".to_string(),
                );
                return None;
            }
            if source_params.len() != signature.parameter_types.len() {
                self.error(
                    span,
                    format!(
                        "anonymous function has {} parameter(s), but the expected function type has {}",
                        source_params.len(),
                        signature.parameter_types.len()
                    ),
                );
                return None;
            }
        }

        let mut parameter_types = Vec::with_capacity(source_params.len());
        for (index, parameter) in source_params.iter().enumerate() {
            let ty = self.resolve_type_ref(&parameter.ty)?;
            if let Some((_, signature)) = &expected_signature {
                let expected = signature.parameter_types[index];
                if !self.types_equal(ty, expected) {
                    let found = self.type_name(ty);
                    let expected = self.type_name(expected);
                    self.error(
                        parameter.ty.span,
                        format!(
                            "anonymous-function parameter type is {found}, but the expected type is {expected}"
                        ),
                    );
                    return None;
                }
            }
            parameter_types.push(ty);
        }
        let explicit_return = match source_return_ty {
            Some(return_ty) => Some(self.resolve_type_ref(return_ty)?),
            None => None,
        };
        let expected_return = expected_signature
            .as_ref()
            .map(|(_, signature)| signature.return_type);
        if let (Some(explicit), Some(expected)) = (explicit_return, expected_return)
            && !self.types_equal(explicit, expected)
        {
            let found = self.type_name(explicit);
            let expected = self.type_name(expected);
            self.error(
                source_return_ty.expect("explicit return type").span,
                format!(
                    "anonymous-function return type is {found}, but the expected type is {expected}"
                ),
            );
            return None;
        }
        let known_return = explicit_return.or(expected_return);

        let capture_environment = self.capture_environment();
        let outer_locals = std::mem::take(&mut self.locals);
        let outer_scopes = std::mem::replace(&mut self.scopes, Scopes::new());
        let outer_return_ty = self.current_return_ty;
        let outer_return_inference = self.return_inference.take();
        let outer_fn_name = std::mem::take(&mut self.current_fn_name);
        let outer_owner = self.current_owner;
        let outer_this = self.current_this.take();
        let outer_smart_casts = std::mem::take(&mut self.smart_casts);
        self.capture_contexts.push(CaptureContext {
            available: capture_environment,
            captures: Vec::new(),
            by_binding: std::collections::HashMap::new(),
        });
        let function_number = self.next_anonymous_function;
        self.next_anonymous_function += 1;
        self.current_fn_name = format!("$anonymous.{function_number}");
        self.current_return_ty = known_return.unwrap_or(self.unit);
        self.return_inference = known_return.is_none().then(ReturnInference::default);
        self.push_suspension_context(if is_suspend {
            SuspensionContext::SuspendFunction
        } else {
            SuspensionContext::Forbidden(ForbiddenSuspendContext::Function)
        });
        self.push_safety_context(hir::Safety::Safe);
        self.push_scope();

        let lowered = (|| {
            let mut params = Vec::with_capacity(source_params.len());
            for (parameter, ty) in source_params.iter().zip(&parameter_types) {
                if self.scopes.is_declared_here(&parameter.name.text) {
                    self.error(
                        parameter.name.span,
                        format!("duplicate parameter `{}`", parameter.name.text),
                    );
                    return None;
                }
                let local = self.alloc_local(parameter.name.text.clone(), *ty, false);
                self.scopes.declare(parameter.name.text.clone(), local);
                params.push(hir::Param {
                    name: parameter.name.text.clone(),
                    ty: *ty,
                    local,
                });
            }

            let diagnostics_before = self.diagnostics.len();
            let mut statements = self.lower_block(body);
            let return_ty = if let Some(known) = known_return {
                known
            } else {
                let inference = self
                    .return_inference
                    .take()
                    .expect("return inference is active");
                if inference.saw_bare && !inference.value_types.is_empty() {
                    self.error(
                        body.span,
                        "anonymous function mixes bare and value returns".to_string(),
                    );
                    return None;
                }
                if inference.value_types.is_empty() {
                    self.unit
                } else {
                    self.least_upper_bound(&inference.value_types)
                }
            };
            self.current_return_ty = return_ty;
            if self.diagnostics.len() == diagnostics_before
                && !self.types_equal(return_ty, self.unit)
                && statements_can_fall_through(&statements)
            {
                let found = self.type_name(return_ty);
                self.error(
                    body.span,
                    format!(
                        "anonymous function returning {found} may complete without returning a value"
                    ),
                );
                return None;
            }
            if known_return.is_none() {
                statements = self.adapt_inferred_returns(statements, return_ty);
            }
            if self.types_equal(return_ty, self.unit) && statements_can_fall_through(&statements) {
                statements.push(hir::Statement {
                    kind: hir::StatementKind::Return { value: None },
                    span: body.span,
                });
            }

            let function_ty =
                self.intern_function_type(is_suspend, parameter_types.clone(), return_ty);
            let Type::Function(function_type) = self.types[function_ty] else {
                unreachable!()
            };
            let closure_local = self.alloc_local("$closure".to_string(), function_ty, false);
            let mut abi_params = Vec::with_capacity(params.len() + 1);
            abi_params.push(hir::Param {
                name: "$closure".to_string(),
                ty: function_ty,
                local: closure_local,
            });
            abi_params.extend(params);
            let type_params = self.type_params_in_scope.clone();
            let function = self.functions.alloc(hir::Function {
                name: self.current_fn_name.clone(),
                genericity: hir::FunctionGenericity::Plain,
                is_suspend,
                params: abi_params,
                return_ty,
                attributes: hir::FunctionAttributes::default(),
                kind: hir::FunctionKind::User(hir::Body {
                    locals: std::mem::take(&mut self.locals),
                    statements,
                }),
                method: None,
                span,
            });
            if !type_params.is_empty() {
                self.register_generic(function, type_params.clone());
            }
            let captures = self.finish_current_captures();
            let id = self.anonymous_functions.alloc(hir::AnonymousFunction {
                function,
                function_type,
                owner_type_param_count: type_params.len(),
                captures,
                span,
            });
            Some(hir::Expr {
                kind: ExprKind::AnonymousFunction(id),
                ty: function_ty,
                span,
            })
        })();

        self.pop_scope();
        self.pop_safety_context();
        self.pop_suspension_context();
        self.capture_contexts.pop();
        self.locals = outer_locals;
        self.scopes = outer_scopes;
        self.current_return_ty = outer_return_ty;
        self.return_inference = outer_return_inference;
        self.current_fn_name = outer_fn_name;
        self.current_owner = outer_owner;
        self.current_this = outer_this;
        self.smart_casts = outer_smart_casts;
        lowered
    }
}
