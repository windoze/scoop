use super::*;
mod anonymous;
mod expectation;
mod parameters;
mod returns;
mod synthesized;
pub(crate) use expectation::ExpressionExpectation;
use expectation::LambdaExpectation;

impl Lowerer {
    pub(super) fn lower_lambda(
        &mut self,
        label: Option<&ast::Ident>,
        is_suspend: bool,
        parameters: Option<&[ast::LambdaParam]>,
        body: &ast::Block,
        span: Span,
        expected: Option<LambdaExpectation>,
    ) -> Option<hir::Expr> {
        self.with_pattern_transaction(|state| {
            state.lower_lambda_inner(
                label,
                is_suspend,
                parameters,
                body.span,
                span,
                expected,
                |state, expected| state.lower_lambda_body(body, expected),
            )
        })
    }

    /// A lambda is one lowering owner: its complete parameter header, binding
    /// plans, body, captures and generated entities commit together. Callers
    /// enter through `lower_lambda`, which supplies the single owner-level
    /// transaction around this implementation.
    #[allow(clippy::too_many_arguments)]
    fn lower_lambda_inner(
        &mut self,
        label: Option<&ast::Ident>,
        is_suspend: bool,
        parameters: Option<&[ast::LambdaParam]>,
        body_span: Span,
        span: Span,
        expected_signature: Option<LambdaExpectation>,
        lower_body: impl FnOnce(&mut Self, Option<TypeId>) -> Option<crate::stmt::ValueBlock>,
    ) -> Option<hir::Expr> {
        if let Some(signature) = &expected_signature
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
                Some(signature) if signature.parameter_types.len() == 1 => vec![None],
                Some(signature) if signature.parameter_types.is_empty() => Vec::new(),
                Some(signature) => {
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
        if let Some(signature) = &expected_signature
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
        let nested_scope = self.scopes.nested_callable();
        let outer_scopes = std::mem::replace(&mut self.scopes, nested_scope);
        let outer_return_ty = self.current_return_ty;
        let outer_return_inference = self.return_inference.take();
        let outer_callable_label = std::mem::replace(
            &mut self.current_callable_label,
            label.map(|name| name.text.clone()),
        );
        let outer_fn_name = std::mem::take(&mut self.current_fn_name);
        let outer_loop_targets = std::mem::take(&mut self.loop_targets);
        let outer_source_context = self.current_source_context;
        let outer_owner = self.current_owner;
        let outer_this = self.current_this.take();
        let outer_initialization = if expected_signature
            .as_ref()
            .is_some_and(|signature| signature.receiver.is_some())
        {
            self.initialization_context.take()
        } else {
            None
        };
        let outer_receivers = self.receiver_scopes.len();
        let outer_smart_casts = self.smart_casts.clone();
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
            let receiver_ty = expected_signature
                .as_ref()
                .and_then(|signature| signature.receiver);
            let receiver = receiver_ty.map(|ty| {
                self.bind_callable_receiver(ty, label.map(|label| label.text.clone()), span)
            });
            if receiver.is_none() {
                self.push_receiver_scope(label.map(|label| label.text.clone()), None);
            }
            let parameters = self.lower_lambda_parameters(
                &source_parameters,
                expected_signature
                    .as_ref()
                    .map(|signature| signature.parameter_types.as_slice()),
                span,
            )?;
            let mut prefix = parameters.prefix;
            let mut abi_params = parameters.abi;
            if let Some(receiver) = receiver {
                abi_params.insert(0, receiver);
            }
            let parameter_types = parameters.types;

            let expected_return = expected_signature
                .as_ref()
                .and_then(|signature| signature.return_type);
            self.current_return_ty = expected_return.unwrap_or(self.unit);
            self.return_inference = expected_return.is_none().then(ReturnInference::default);
            let mut value_block = lower_body(self, expected_return)?;
            let return_ty =
                self.finish_lambda_returns(&mut value_block, expected_return, body_span)?;
            if !self.check_local_type_result(return_ty, body_span, &self.scopes.clone(), true) {
                return None;
            }
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
                        span: body_span,
                        kind: hir::StatementKind::Return {
                            value: self.unit_expression(body_span),
                        },
                    });
                } else {
                    let value = self.adapt_to(value, return_ty);
                    prefix.push(hir::Statement {
                        span: value.span,
                        kind: hir::StatementKind::Return { value },
                    });
                }
            }
            let function_type = self.intern_receiver_function_type(
                is_suspend,
                receiver_ty,
                parameter_types,
                return_ty,
            );
            let function_ty = self.function_types[function_type].canonical_type;
            let closure_local = self.alloc_synthetic_local(
                "$closure".to_string(),
                function_ty,
                false,
                scoop_identity::SyntheticLocalRole::Temporary,
            );
            let mut params = Vec::with_capacity(abi_params.len() + 1);
            params.push(hir::Param {
                name: "$closure".to_string(),
                ty: function_ty,
                local: closure_local,
            });
            params.extend(abi_params);
            let type_params = self.type_params_in_scope.clone();
            let access = self.local_declaration_access();
            let annotation_locals = source_parameters
                .iter()
                .flatten()
                .any(|p| !p.annotations.is_empty())
                .then(|| self.locals.clone());
            let function = self.functions.alloc(hir::Function {
                signature: hir::CallableSignature {
                    context_parameters: Vec::new(),
                    release_callability: Default::default(),
                    name: self.current_fn_name.clone(),
                    is_suspend,
                    modifiers: hir::CallableModifiers::default(),
                    params,
                    return_ty,
                    attributes: hir::FunctionAttributes::default(),
                    span,
                },

                access,
                genericity: hir::FunctionGenericity::Plain,
                kind: hir::FunctionKind::User(hir::Body {
                    locals: std::mem::take(&mut self.locals).into_arena(),
                    statements: prefix,
                }),
                method: None,
            });
            if !type_params.is_empty() {
                self.register_generic(function, type_params.clone());
            }
            self.function_files.insert(function, self.current_file);
            for (index, parameter) in source_parameters.iter().enumerate() {
                if let Some(ty) = parameter.and_then(|parameter| parameter.ty.as_ref()) {
                    self.annotate_type_site(
                        hir::SourceAnnotationTarget::Parameter {
                            callable: hir::SourceAnnotationCallable::Function(function),
                            index: index as u32,
                        },
                        hir::AnnotationTypeSlot::Value,
                        ty,
                    );
                }
            }
            if let Some(locals) = annotation_locals {
                let empty = std::mem::replace(&mut self.locals, locals);
                for (index, parameter) in source_parameters.iter().enumerate() {
                    let Some(parameter) = parameter else { continue };
                    self.bind_source_annotations(
                        hir::SourceAnnotationTarget::Parameter {
                            callable: hir::SourceAnnotationCallable::Function(function),
                            index: index as u32,
                        },
                        &parameter.annotations,
                    );
                }
                self.locals = empty;
            }
            let captures = self.finish_current_captures(literal_origin);
            let id = self.lambdas.alloc(hir::Lambda {
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
        self.return_inference = outer_return_inference;
        self.current_callable_label = outer_callable_label;
        self.current_fn_name = outer_fn_name;
        self.current_source_context = outer_source_context;
        self.definition_paths = outer_definition_paths;
        self.current_owner = outer_owner;
        self.current_this = outer_this;
        if let Some(initialization) = outer_initialization {
            self.initialization_context = Some(initialization);
        }
        self.receiver_scopes.truncate(outer_receivers);
        self.smart_casts = outer_smart_casts;
        debug_assert!(self.loop_targets.is_empty());
        self.loop_targets = outer_loop_targets;
        lowered
    }
}
