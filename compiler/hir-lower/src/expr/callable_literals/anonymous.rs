use super::*;

mod signature;
use signature::AnonymousSignature;

impl Lowerer {
    pub(super) fn bind_anonymous_parameters(
        &mut self,
        source_params: &[ast::Param],
        parameter_types: &[TypeId],
    ) -> Option<Vec<hir::Param>> {
        let mut params = Vec::with_capacity(source_params.len());
        for (index, (parameter, ty)) in source_params.iter().zip(parameter_types).enumerate() {
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
        Some(params)
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::expr) fn lower_anonymous_function(
        &mut self,
        label: Option<&ast::Ident>,
        source_receiver: Option<&ast::TypeRef>,
        is_suspend: bool,
        source_params: &[ast::Param],
        source_return_ty: Option<&ast::TypeRef>,
        body: &ast::Block,
        span: Span,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let AnonymousSignature {
            receiver: receiver_ty,
            parameters: parameter_types,
            result: known_return,
        } = self.anonymous_signature(
            source_receiver,
            is_suspend,
            source_params,
            source_return_ty,
            span,
            expected,
        )?;

        self.with_callable_literal_scope(
            scope::LiteralKind::AnonymousFunction,
            label,
            is_suspend,
            receiver_ty.is_some(),
            span,
            |state, literal_origin, definition_path| {
                state.current_return_ty = known_return.unwrap_or(state.unit);
                state.return_inference = known_return.is_none().then(ReturnInference::default);
                let mut params =
                    Vec::with_capacity(source_params.len() + usize::from(receiver_ty.is_some()));
                params.extend(state.bind_literal_receiver(receiver_ty, label, span));
                params.extend(state.bind_anonymous_parameters(source_params, &parameter_types)?);

                let diagnostics_before = state.diagnostics.len();
                let mut statements = state.lower_block(body);
                let return_ty = if let Some(known) = known_return {
                    known
                } else {
                    let inference = state
                        .return_inference
                        .take()
                        .expect("return inference is active");
                    if inference.saw_bare && !inference.value_types.is_empty() {
                        state.error(
                            body.span,
                            "anonymous function mixes bare and value returns".to_string(),
                        );
                        return None;
                    }
                    if inference.value_types.is_empty() {
                        state.unit
                    } else {
                        state.least_upper_bound(&inference.value_types)
                    }
                };
                state.current_return_ty = return_ty;
                if state.diagnostics.len() == diagnostics_before
                    && !state.types_equal(return_ty, state.unit)
                    && state
                        .statements_control_outcomes(&statements)
                        .can_fall_through()
                {
                    let found = state.type_name(return_ty);
                    state.error(
                        body.span,
                        format!(
                            "anonymous function returning {found} may complete without returning a value"
                        ),
                    );
                    return None;
                }
                if known_return.is_none() {
                    statements = state.adapt_inferred_returns(statements, return_ty);
                }
                if state.types_equal(return_ty, state.unit)
                    && state
                        .statements_control_outcomes(&statements)
                        .can_fall_through()
                {
                    statements.push(hir::Statement {
                        kind: hir::StatementKind::Return {
                            value: state.unit_expression(body.span),
                        },
                        span: body.span,
                    });
                }

                let function_type = state.intern_receiver_function_type(
                    is_suspend,
                    receiver_ty,
                    parameter_types.clone(),
                    return_ty,
                );
                let function_ty = state.function_types[function_type].canonical_type;
                let closure_local = state.alloc_synthetic_local(
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
                let type_params = state.type_params_in_scope.clone();
                let access = state.local_declaration_access();
                let annotation_locals = source_params
                    .iter()
                    .any(|p| !p.annotations.is_empty())
                    .then(|| state.locals.clone());
                let function = state.functions.alloc(hir::Function {
                    signature: hir::CallableSignature {
                        context_parameters: Vec::new(),
                        release_callability: Default::default(),
                        name: state.current_fn_name.clone(),
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
                        locals: std::mem::take(&mut state.locals).into_arena(),
                        statements,
                    }),
                    method: None,
                });
                if !type_params.is_empty() {
                    state.register_generic(function, type_params.clone());
                }
                state.record_function_file(function, state.current_file);
                state.annotate_callable_types(
                    hir::SourceAnnotationCallable::Function(function),
                    source_receiver,
                    source_return_ty,
                );
                for (index, parameter) in source_params.iter().enumerate() {
                    state.annotate_type_site(
                        hir::SourceAnnotationTarget::Parameter {
                            callable: hir::SourceAnnotationCallable::Function(function),
                            index: index as u32,
                        },
                        hir::AnnotationTypeSlot::Value,
                        &parameter.ty,
                    );
                }
                if let Some(locals) = annotation_locals {
                    let empty = std::mem::replace(&mut state.locals, locals);
                    state.annotate_parameters(
                        hir::SourceAnnotationCallable::Function(function),
                        source_params,
                    );
                    state.locals = empty;
                }
                let captures = state.finish_current_captures(literal_origin);
                let id = state.anonymous_functions.alloc(hir::AnonymousFunction {
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
                    kind: ExprKind::AnonymousFunction(id),
                    ty: function_ty,
                    span,
                    origin: literal_origin,
                })
            },
        )
    }
}
