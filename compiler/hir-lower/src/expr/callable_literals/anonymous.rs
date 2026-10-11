use super::*;

mod signature;
use signature::AnonymousSignature;

impl Lowerer {
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
        let outer_initialization = if receiver_ty.is_some() {
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
            let mut params =
                Vec::with_capacity(source_params.len() + usize::from(receiver_ty.is_some()));
            if let Some(ty) = receiver_ty {
                params.push(self.bind_callable_receiver(
                    ty,
                    label.map(|label| label.text.clone()),
                    span,
                ));
            } else {
                self.push_receiver_scope(label.map(|label| label.text.clone()), None);
            }
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
                    kind: hir::StatementKind::Return {
                        value: self.unit_expression(body.span),
                    },
                    span: body.span,
                });
            }

            let function_type = self.intern_receiver_function_type(
                is_suspend,
                receiver_ty,
                parameter_types.clone(),
                return_ty,
            );
            let function_ty = self.function_types[function_type].canonical_type;
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
            let annotation_locals = source_params
                .iter()
                .any(|p| !p.annotations.is_empty())
                .then(|| self.locals.clone());
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
                    locals: std::mem::take(&mut self.locals).into_arena(),
                    statements,
                }),
                method: None,
            });
            if !type_params.is_empty() {
                self.register_generic(function, type_params.clone());
            }
            self.function_files.insert(function, self.current_file);
            if let Some(locals) = annotation_locals {
                let empty = std::mem::replace(&mut self.locals, locals);
                self.annotate_parameters(
                    hir::SourceAnnotationCallable::Function(function),
                    source_params,
                );
                self.locals = empty;
            }
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
