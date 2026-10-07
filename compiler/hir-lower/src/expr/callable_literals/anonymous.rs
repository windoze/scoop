use super::*;

impl Lowerer {
    pub(in crate::expr) fn lower_anonymous_function(
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
                if !self.is_subtype(expected, ty) {
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
            && !self.is_subtype(explicit, expected)
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
        let outer_return_inference = self.return_inference.take();
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
        let function_number = self.next_anonymous_function;
        self.next_anonymous_function += 1;
        self.current_fn_name = format!("$anonymous.{function_number}");
        self.set_source_context(hir::SourceContextSubject::LexicalCallable {
            root: self.current_definition_root(),
            path: definition_path.clone(),
            role: scoop_identity::LexicalCallableRole::AnonymousFunctionBody,
        });
        self.current_return_ty = known_return.unwrap_or(self.unit);
        self.return_inference = known_return.is_none().then(ReturnInference::default);
        self.push_suspension_context(if is_suspend {
            SuspensionContext::SuspendFunction
        } else {
            SuspensionContext::Forbidden(ForbiddenSuspendContext::Function)
        });
        self.push_safety_context(hir::Safety::Safe);
        self.push_scope();
        let outer_default_template = std::mem::replace(&mut self.lowering_default_template, false);

        let lowered = (|| {
            let mut params = Vec::with_capacity(source_params.len());
            for (index, (parameter, ty)) in source_params.iter().zip(&parameter_types).enumerate() {
                if self.scopes.is_declared_here(&parameter.name.text) {
                    self.error(
                        parameter.name.span,
                        format!("duplicate parameter `{}`", parameter.name.text),
                    );
                    return None;
                }
                let local = self.alloc_parameter_local(
                    parameter.name.text.clone(),
                    *ty,
                    index,
                    parameter.name.span,
                );
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
                && self
                    .statements_control_outcomes(&statements)
                    .can_fall_through()
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
            if self.types_equal(return_ty, self.unit)
                && self
                    .statements_control_outcomes(&statements)
                    .can_fall_through()
            {
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
            let closure_local = self.alloc_synthetic_local(
                "$closure".to_string(),
                function_ty,
                false,
                scoop_identity::SyntheticLocalRole::Temporary,
            );
            let mut abi_params = Vec::with_capacity(params.len() + 1);
            abi_params.push(hir::Param {
                name: "$closure".to_string(),
                ty: function_ty,
                local: closure_local,
            });
            abi_params.extend(params);
            let type_params = self.type_params_in_scope.clone();
            let access = self.local_declaration_access();
            let function = self.functions.alloc(hir::Function {
                signature: hir::CallableSignature {
                    context_parameters: Vec::new(),
                    release_callability: Default::default(),
                    name: self.current_fn_name.clone(),
                    is_suspend,
                    modifiers: hir::CallableModifiers::default(),
                    params: abi_params,
                    return_ty,
                    attributes: hir::FunctionAttributes::default(),
                    span,
                },

                access,
                genericity: hir::FunctionGenericity::Plain,
                kind: hir::FunctionKind::User(hir::Body {
                    locals: std::mem::take(&mut self.locals),
                    statements,
                }),
                method: None,
            });
            if !type_params.is_empty() {
                self.register_generic(function, type_params.clone());
            }
            self.function_files.insert(function, self.current_file);
            let captures = self.finish_current_captures(literal_origin);
            let id = self.anonymous_functions.alloc(hir::AnonymousFunction {
                definition: hir::LexicalFunctionDefinition::Source {
                    function,
                    root: self.current_definition_root(),
                },
                definition_path: definition_path.clone(),
                function_type,
                owner_type_param_count: type_params.len(),
                body_type_arguments: hir::CallableBodyTypeArguments::Lexical,
                captures,
                span,
            });
            Some(hir::Expr {
                kind: ExprKind::AnonymousFunction(id),
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
        self.return_inference = outer_return_inference;
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
