//! Generic call resolution, argument inference and concrete callable binding.

use super::*;

mod intrinsics;

impl Lowerer {
    /// A bare call `f(args)`: the candidate layers are, in order,
    /// the current host's methods (inside a member function, where
    /// `f(...)` means `this.f(...)`), extensions callable on a lexical `this`,
    /// top-level functions declared on the call site's own side of the
    /// core/user boundary, and the other side (the implicitly imported
    /// layer). The first layer containing an applicable candidate wins whole
    /// (M16 DESIGN 2.2). The import layering is relative to
    /// the call site's file: for a user-file call that is user
    /// top-level → core, for a core-file call core → user. An applicable user
    /// declaration therefore shadows core overloads without breaking the core
    /// library's own internal calls. A single candidate and an overload set
    /// both enter the same M16 resolver.
    pub(super) fn lower_function_call(
        &mut self,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let name = call.callee.text.clone();
        let mut first_failure = None;

        // Layer 1: the nearest lexical block containing local functions of
        // this name. Declarations enter it only as they are encountered.
        let local_candidates = self.local_function_scopes.lookup(&name);
        if !local_candidates.is_empty() {
            match self.probe_expr_layer(|state, layer_sink| {
                state.lower_local_function_layer(
                    &name,
                    &local_candidates,
                    call,
                    layer_sink,
                    expected,
                )
            }) {
                Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                Err(failure) => first_failure = Some(failure),
            }
        }

        // Layer 2: members of the current host.
        let members = self
            .current_this_ty()
            .map(|host_ty| self.methods_by_name(host_ty, &name))
            .unwrap_or_default();
        if !members.is_empty() {
            match self.probe_expr_layer(|state, layer_sink| {
                let receiver = state
                    .lower_current_this(call.callee.span)
                    .expect("a member callable body always has a lexical `this`");
                state.finish_overloaded_method_call(
                    members,
                    &name,
                    receiver,
                    CallSite {
                        type_args: &call.type_args,
                        args: &call.args,
                        span: call.span,
                    },
                    layer_sink,
                    expected,
                )
            }) {
                Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                Err(failure) => first_failure.get_or_insert(failure),
            };
        }

        // Reading an implicit `this` property can register a closure capture.
        // Keep that mutation inside the candidate state until its invoke wins.
        let property_candidate = {
            let mut state = self.clone();
            state
                .bare_member_fallback(&call.callee)
                .map(|property| (state, property))
        };
        if let Some((property_state, property)) = &property_candidate
            && let Some(layer) = property_state.clone().probe_property_member_invoke(
                property.clone(),
                CallSite {
                    type_args: &call.type_args,
                    args: &call.args,
                    span: call.span,
                },
                expected,
                false,
            )
        {
            match layer {
                Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                Err(failure) => {
                    first_failure.get_or_insert(failure);
                }
            }
        }

        // An extension body has a lexical `this` just like a member body.
        // If no real member wins, another visible extension may use it as
        // the implicit receiver before ordinary top-level functions.
        if self.current_this_ty().is_some() {
            for same_side in [true, false] {
                let extensions = self.extension_candidates_on_side(&name, same_side);
                if !extensions.is_empty() {
                    match self.probe_expr_layer(|state, layer_sink| {
                        let receiver = state
                            .lower_current_this(call.callee.span)
                            .expect("a lexical receiver has a `this` value");
                        state.finish_extension_call(
                            &extensions,
                            &name,
                            receiver,
                            CallSite {
                                type_args: &call.type_args,
                                args: &call.args,
                                span: call.span,
                            },
                            layer_sink,
                            expected,
                        )
                    }) {
                        Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                        Err(failure) => {
                            first_failure.get_or_insert(failure);
                        }
                    }
                }
                if let Some((property_state, property)) = &property_candidate
                    && let Some(layer) = property_state.probe_property_extension_invoke(
                        property.clone(),
                        CallSite {
                            type_args: &call.type_args,
                            args: &call.args,
                            span: call.span,
                        },
                        expected,
                        false,
                        same_side,
                    )
                {
                    match layer {
                        Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                        Err(failure) => {
                            first_failure.get_or_insert(failure);
                        }
                    }
                }
            }
        }

        // Final two layers, relative to the call site's file: the
        // declarations on the call site's own side of the core/user
        // boundary come first, the other side is the implicitly
        // imported layer.
        let top_level_layers = self.top_level_candidate_layers(&name);
        if top_level_layers.is_empty() && first_failure.is_none() {
            self.error(
                call.callee.span,
                format!("unknown function `{}`", call.callee.text),
            );
            return None;
        }
        for candidates in top_level_layers {
            match self.probe_expr_layer(|state, layer_sink| {
                state.lower_top_level_function_layer(&name, &candidates, call, layer_sink, expected)
            }) {
                Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                Err(failure) => {
                    first_failure.get_or_insert(failure);
                }
            }
        }

        self.commit_layer_diagnostics(*first_failure.expect("at least one callable layer failed"));
        None
    }

    fn lower_local_function_layer(
        &mut self,
        name: &str,
        candidates: &[hir::LocalFunctionId],
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let functions: Vec<_> = candidates
            .iter()
            .map(|id| self.local_functions[*id].function)
            .collect();
        let owner_count = self.local_functions[candidates[0]].owner_type_param_count;
        let owner_type_args = self.ambient_type_args(owner_count);
        let explicit_type_args = self.resolve_call_type_args(&call.type_args)?;
        let resolved = self.resolve_overload(
            name,
            &functions,
            &owner_type_args,
            crate::overload::OverloadCall {
                explicit_type_args: &explicit_type_args,
                arg_exprs: &call.args,
                span: call.span,
                expected_result: expected,
            },
            sink,
        )?;
        let function = resolved.function();
        let local_function = self.local_function_by_function[&function];
        let captures = self.local_call_capture_args(local_function, call.span)?;
        let callee = self.materialize_resolved_callee(&resolved);
        self.check_call_effects(callee, call.span);
        Some(hir::Expr {
            kind: ExprKind::LocalFunctionCall {
                local_function,
                callee,
                captures,
                args: resolved.args,
            },
            ty: resolved.return_ty,
            span: call.span,
            origin: self.expression_origin(call.span),
        })
    }

    fn lower_top_level_function_layer(
        &mut self,
        name: &str,
        candidates: &[hir::FunctionId],
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let explicit_type_args = self.resolve_call_type_args(&call.type_args)?;
        let resolved = self.resolve_overload(
            name,
            candidates,
            &[],
            crate::overload::OverloadCall {
                explicit_type_args: &explicit_type_args,
                arg_exprs: &call.args,
                span: call.span,
                expected_result: expected,
            },
            sink,
        )?;
        let function = resolved.function();
        if let Some(core) = self.foreign_callback_core {
            if function == core.register {
                return self
                    .lower_foreign_callback_registration(core, function, call, resolved, sink);
            }
            let operation = if function == core.retain {
                Some(hir::ForeignCallbackOperation::Retain)
            } else if function == core.release {
                Some(hir::ForeignCallbackOperation::Release)
            } else if function == core.query_state {
                Some(hir::ForeignCallbackOperation::State)
            } else if function == core.failure {
                Some(hir::ForeignCallbackOperation::Failure)
            } else {
                None
            };
            if let Some(operation) = operation {
                return self.lower_foreign_callback_call(core, function, operation, call, resolved);
            }
        }
        if self.ffi_core.is_some_and(|core| {
            function == core.address_of || function == core.size_of || function == core.align_of
        }) {
            return self.lower_pointer_top_level_intrinsic(function, call, resolved);
        }
        let ty = resolved.return_ty;
        let callee = self.materialize_resolved_callee(&resolved);
        self.check_call_effects(callee, call.span);
        Some(hir::Expr {
            kind: ExprKind::Call {
                callee,
                args: resolved.args,
            },
            ty,
            span: call.span,
            origin: self.expression_origin(call.span),
        })
    }

    pub(super) fn ambient_type_args(&mut self, count: usize) -> Vec<TypeId> {
        self.type_params_in_scope[..count]
            .iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>()
            .into_iter()
            .map(|parameter| self.intern_type(Type::Param(parameter)))
            .collect()
    }

    pub(crate) fn resolve_call_type_args(&mut self, refs: &[ast::TypeRef]) -> Option<Vec<TypeId>> {
        refs.iter()
            .map(|type_ref| self.resolve_type_ref(type_ref))
            .collect()
    }

    pub(super) fn local_call_capture_args(
        &mut self,
        local_function: hir::LocalFunctionId,
        span: Span,
    ) -> Option<Vec<hir::Expr>> {
        let captures: Vec<_> = self.local_functions[local_function]
            .captures
            .iter()
            .map(|capture| (capture.binding, capture.name.clone(), capture.ty))
            .collect();
        let mut args = Vec::with_capacity(captures.len());
        for (binding, name, ty) in captures {
            if let Some((local, _)) = self
                .locals
                .iter()
                .find(|(_, candidate)| candidate.binding == binding)
            {
                args.push(hir::Expr {
                    kind: ExprKind::Local(local),
                    ty,
                    span,
                    origin: self.expression_origin(span),
                });
                continue;
            }
            args.push(self.lower_capture_binding(binding, &name, span)?);
        }
        Some(args)
    }
}
