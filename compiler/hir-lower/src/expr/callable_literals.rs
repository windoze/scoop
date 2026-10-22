use super::*;
mod anonymous;
mod expectation;
mod parameters;
mod recovered;
mod returns;
mod scope;
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
        let lower = |state: &mut Self| {
            state.lower_lambda_inner(
                label,
                is_suspend,
                parameters,
                body.span,
                span,
                expected,
                |state, expected| state.lower_lambda_body(body, expected),
            )
        };
        if self.observes_current_body() {
            // The editing query owns this state, including the arenas for
            // checked receivers in a lambda whose later body fails. Rejected
            // overload probes still discard their entire private state.
            lower(self)
        } else {
            self.with_pattern_transaction(lower)
        }
    }

    /// A lambda is one lowering owner: its complete parameter header, binding
    /// plans, body, captures and generated entities commit together in ordinary
    /// compilation. `lower_lambda` supplies that owner-level transaction or the
    /// editing query's private state, which never publishes a failed body.
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
        let source_parameters = self.lambda_source_parameters(
            is_suspend,
            parameters,
            span,
            expected_signature.as_ref(),
        )?;

        let has_receiver = expected_signature
            .as_ref()
            .is_some_and(|signature| signature.receiver.is_some());
        self.with_callable_literal_scope(
            scope::LiteralKind::Lambda,
            label,
            is_suspend,
            has_receiver,
            span,
            |state, literal_origin, definition_path| {
                let receiver_ty = expected_signature
                    .as_ref()
                    .and_then(|signature| signature.receiver);
                let receiver = state.bind_literal_receiver(receiver_ty, label, span);
                let parameters = state.lower_lambda_parameters(
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
                state.current_return_ty = expected_return.unwrap_or(state.unit);
                state.return_inference = expected_return.is_none().then(ReturnInference::default);
                let mut value_block = lower_body(state, expected_return)?;
                let return_ty =
                    state.finish_lambda_returns(&mut value_block, expected_return, body_span)?;
                if !state.check_local_type_result(return_ty, body_span, &state.scopes.clone(), true)
                {
                    return None;
                }
                state.current_return_ty = return_ty;
                prefix.append(&mut value_block.statements);
                if let Some(value) = value_block.value.take() {
                    if state.types_equal(return_ty, state.unit) {
                        if !matches!(value.kind, ExprKind::UnitLiteral) {
                            prefix.push(hir::Statement {
                                span: value.span,
                                kind: hir::StatementKind::Expr(value),
                            });
                        }
                        prefix.push(hir::Statement {
                            span: body_span,
                            kind: hir::StatementKind::Return {
                                value: state.unit_expression(body_span),
                            },
                        });
                    } else {
                        let value = state.adapt_to(value, return_ty);
                        prefix.push(hir::Statement {
                            span: value.span,
                            kind: hir::StatementKind::Return { value },
                        });
                    }
                }
                let function_type = state.intern_receiver_function_type(
                    is_suspend,
                    receiver_ty,
                    parameter_types,
                    return_ty,
                );
                let function_ty = state.function_types[function_type].canonical_type;
                let closure_local = state.alloc_synthetic_local(
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
                let type_params = state.type_params_in_scope.clone();
                let access = state.local_declaration_access();
                let annotation_locals = source_parameters
                    .iter()
                    .flatten()
                    .any(|p| !p.annotations.is_empty())
                    .then(|| state.locals.clone());
                let function = state.functions.alloc(hir::Function {
                    signature: hir::CallableSignature {
                        context_parameters: Vec::new(),
                        release_callability: Default::default(),
                        name: state.current_fn_name.clone(),
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
                        locals: std::mem::take(&mut state.locals).into_arena(),
                        statements: prefix,
                    }),
                    method: None,
                });
                if !type_params.is_empty() {
                    state.register_generic(function, type_params.clone());
                }
                state.record_function_file(function, state.current_file);
                for (index, parameter) in source_parameters.iter().enumerate() {
                    if let Some(ty) = parameter.and_then(|parameter| parameter.ty.as_ref()) {
                        state.annotate_type_site(
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
                    let empty = std::mem::replace(&mut state.locals, locals);
                    for (index, parameter) in source_parameters.iter().enumerate() {
                        let Some(parameter) = parameter else { continue };
                        state.bind_source_annotations(
                            hir::SourceAnnotationTarget::Parameter {
                                callable: hir::SourceAnnotationCallable::Function(function),
                                index: index as u32,
                            },
                            &parameter.annotations,
                        );
                    }
                    state.locals = empty;
                }
                let captures = state.finish_current_captures(literal_origin);
                let id = state.lambdas.alloc(hir::Lambda {
                    definition: hir::LexicalFunctionDefinition::Source {
                        function,
                        root: state.current_definition_root(),
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
            },
        )
    }
}
