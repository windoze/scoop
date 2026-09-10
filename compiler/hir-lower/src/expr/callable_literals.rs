use super::*;
mod anonymous;

impl Lowerer {
    pub(super) fn lower_lambda(
        &mut self,
        is_suspend: bool,
        parameters: Option<&[ast::LambdaParam]>,
        body: &ast::Block,
        span: Span,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        self.with_pattern_transaction(|state| {
            state.lower_lambda_inner(is_suspend, parameters, body, span, expected)
        })
    }

    /// A lambda is one lowering owner: its complete parameter header, binding
    /// plans, body, captures and generated entities commit together. Callers
    /// enter through `lower_lambda`, which supplies the single owner-level
    /// transaction around this implementation.
    fn lower_lambda_inner(
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
        let literal_origin = self.expression_origin(span);
        let definition_path = self
            .definition_paths
            .next(scoop_identity::StructuralDefinitionSiteRole::Lambda);
        let outer_definition_paths = std::mem::replace(
            &mut self.definition_paths,
            crate::definition_paths::DefinitionPathContext::nested(&definition_path),
        );
        let outer_locals = std::mem::take(&mut self.locals);
        let outer_scopes = std::mem::replace(&mut self.scopes, Scopes::new());
        let outer_return_ty = self.current_return_ty;
        let outer_fn_name = std::mem::take(&mut self.current_fn_name);
        let outer_loop_targets = std::mem::take(&mut self.loop_targets);
        let outer_source_context = self.current_source_context;
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
        self.set_source_context(hir::SourceContextSubject::LexicalCallable {
            root: self.current_definition_root(),
            path: definition_path.clone(),
            role: scoop_identity::LexicalCallableRole::LambdaBody,
        });
        self.push_suspension_context(if is_suspend {
            SuspensionContext::SuspendFunction
        } else {
            SuspensionContext::Forbidden(ForbiddenSuspendContext::Function)
        });
        self.push_safety_context(hir::Safety::Safe);
        self.push_scope();
        let outer_default_template = std::mem::replace(&mut self.lowering_default_template, false);

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
                    let hir::Pattern::Binding { local } = self.lower_pattern_inner(
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
                    let plan = self.lower_irrefutable_binding_plan_from_subject(
                        target.expect("non-binding source parameter has a pattern"),
                        hir::BindingTemporary {
                            local,
                            ty: parameter_ty,
                        },
                        false,
                    )?;
                    prefix.extend(plan.into_statements());
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
            let access = self.local_declaration_access();
            let link_stem = self.local_callable_link_stem(
                self.current_file,
                crate::globals::LocalCallableScope::SourceLocal,
                &self.current_fn_name,
                crate::globals::LocalCallableLinkRole::Lambda(function_number),
            );
            let function = self.functions.alloc(hir::Function {
                link_stem,
                name: self.current_fn_name.clone(),
                access,
                override_access: Vec::new(),
                genericity: hir::FunctionGenericity::Plain,
                is_suspend,
                modifiers: hir::CallableModifiers::default(),
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
            self.function_files.insert(function, self.current_file);
            let captures = self.finish_current_captures();
            let id = self.lambdas.alloc(hir::Lambda {
                definition_root: self.current_definition_root(),
                definition_path: definition_path.clone(),
                function,
                function_type,
                owner_type_param_count: type_params.len(),
                body_type_arguments: hir::CallableBodyTypeArguments::Lexical,
                captures,
                span,
            });
            Some(hir::Expr {
                kind: ExprKind::Lambda(id),
                ty: function_ty,
                span,
                origin: literal_origin,
            })
        })();
        self.lowering_default_template = outer_default_template;

        self.pop_scope();
        self.pop_safety_context();
        self.pop_suspension_context();
        self.capture_contexts.pop();
        self.locals = outer_locals;
        self.scopes = outer_scopes;
        self.current_return_ty = outer_return_ty;
        self.current_fn_name = outer_fn_name;
        self.current_source_context = outer_source_context;
        self.definition_paths = outer_definition_paths;
        self.current_owner = outer_owner;
        self.current_this = outer_this;
        self.smart_casts = outer_smart_casts;
        debug_assert!(self.loop_targets.is_empty());
        self.loop_targets = outer_loop_targets;
        lowered
    }
}
