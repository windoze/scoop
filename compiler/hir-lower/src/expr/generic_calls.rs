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
        prior_ordinary_failure: Option<Box<Lowerer>>,
    ) -> Option<hir::Expr> {
        let name = call.callee.text.clone();
        let mut ordinary_failure = prior_ordinary_failure;

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
                Err(failure) => ordinary_failure = Some(failure),
            }
        }

        if let Some(receiver_ty) = self.initializing_receiver_type() {
            let members = self.methods_by_name(receiver_ty, &name);
            if !members.is_empty() {
                self.error(
                    call.span,
                    "initializing receiver cannot escape before construction completes".into(),
                );
                return None;
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
                    false,
                )
            }) {
                Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                Err(failure) => ordinary_failure.get_or_insert(failure),
            };
        }

        // Reading an implicit `this` property can register a closure capture.
        // Keep that mutation inside the candidate state until its invoke wins.
        let property_candidate = {
            let mut state = self.clone();
            if state.initialization_context.is_none()
                || state.initializing_receiver_has_field(&call.callee.text)
            {
                state
                    .bare_member_fallback(&call.callee)
                    .map(|property| (state, property))
            } else {
                None
            }
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
                    ordinary_failure.get_or_insert(failure);
                }
            }
        }

        // An extension body has a lexical `this` just like a member body.
        // If no real member wins, another visible extension may use it as
        // the implicit receiver before ordinary top-level functions.
        if self.current_this_ty().is_some() {
            for layer in crate::imports::LOOKUP_LAYERS {
                let extensions = self.extension_candidates_in_layer(&name, layer);
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
                            false,
                        )
                    }) {
                        Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                        Err(failure) => {
                            ordinary_failure.get_or_insert(failure);
                        }
                    }
                }
                let mut extension_property_state = self.clone();
                let mut extension_property_sink = Vec::new();
                let receiver = extension_property_state
                    .lower_current_this(call.callee.span)
                    .expect("a lexical receiver has a `this` value");
                match extension_property_state.resolve_extension_property_in_layer(
                    receiver,
                    &call.callee,
                    layer,
                    &mut extension_property_sink,
                    true,
                ) {
                    crate::properties::ExtensionPropertyResolution::Resolved(property) => {
                        if let Some(layer) = extension_property_state.probe_property_member_invoke(
                            property.read.clone(),
                            CallSite {
                                type_args: &call.type_args,
                                args: &call.args,
                                span: call.span,
                            },
                            expected,
                            false,
                        ) {
                            match layer {
                                Ok(mut layer) => {
                                    let mut setup = extension_property_sink.clone();
                                    setup.append(&mut layer.sink);
                                    layer.sink = setup;
                                    return Some(self.commit_expr_layer(layer, sink));
                                }
                                Err(failure) => {
                                    ordinary_failure.get_or_insert(failure);
                                }
                            }
                        }
                        if let Some(layer) = extension_property_state
                            .probe_property_extension_invoke(
                                property.read,
                                CallSite {
                                    type_args: &call.type_args,
                                    args: &call.args,
                                    span: call.span,
                                },
                                expected,
                                false,
                                layer,
                            )
                        {
                            match layer {
                                Ok(mut layer) => {
                                    let mut setup = extension_property_sink;
                                    setup.append(&mut layer.sink);
                                    layer.sink = setup;
                                    return Some(self.commit_expr_layer(layer, sink));
                                }
                                Err(failure) => {
                                    ordinary_failure.get_or_insert(failure);
                                }
                            }
                        }
                    }
                    crate::properties::ExtensionPropertyResolution::Failed => {
                        if extension_property_state.diagnostics.len() > self.diagnostics.len() {
                            ordinary_failure.get_or_insert(Box::new(extension_property_state));
                        }
                    }
                    crate::properties::ExtensionPropertyResolution::NoCandidate => {}
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
                        crate::imports::LOOKUP_LAYERS[crate::imports::LOOKUP_LAYERS.len() - 1],
                    )
                {
                    match layer {
                        Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                        Err(failure) => {
                            ordinary_failure.get_or_insert(failure);
                        }
                    }
                }
            }
        }

        // Final top-level layers (DESIGN 2.3): exact imports shadow the
        // current package, which shadows star imports, which shadow the
        // implicitly imported core surface. Each layer is probed
        // transactionally; the first with an applicable candidate wins.
        let top_level = self
            .functions_by_name
            .get(&name)
            .cloned()
            .unwrap_or_default();
        let mut found_top_level_candidate = false;
        for layer in crate::imports::LOOKUP_LAYERS {
            let candidates = top_level
                .iter()
                .copied()
                .filter(|function| self.function_is_accessible(*function, None))
                .filter(|function| self.function_lookup_layer(*function) == Some(layer))
                .collect::<Vec<_>>();
            if !candidates.is_empty() {
                found_top_level_candidate = true;
                match self.probe_expr_layer(|state, layer_sink| {
                    state.lower_top_level_function_layer(
                        &name,
                        &candidates,
                        call,
                        layer_sink,
                        expected,
                    )
                }) {
                    Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                    Err(failure) => {
                        ordinary_failure.get_or_insert(failure);
                    }
                }
            }

            let property = self
                .properties_by_name
                .get(&name)
                .into_iter()
                .flatten()
                .copied()
                .find(|property| {
                    self.access_domain_allows(&self.properties[*property].access.lookup.0, None)
                        && self.property_lookup_layer(*property) == Some(layer)
                });
            let Some(property) = property else {
                continue;
            };
            let mut candidate = self.clone();
            let property_ty = candidate.properties[property].ty;
            if !candidate.type_exposes_invoke(property_ty, false) {
                continue;
            }
            found_top_level_candidate = true;
            match self.probe_expr_layer(|state, layer_sink| {
                let callee = state.lower_property_read(
                    property,
                    None,
                    None,
                    property_ty,
                    call.callee.span,
                )?;
                state.lower_value_invoke(
                    callee,
                    CallSite {
                        type_args: &call.type_args,
                        args: &call.args,
                        span: call.span,
                    },
                    layer_sink,
                    expected,
                    false,
                )
            }) {
                Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                Err(failure) => {
                    ordinary_failure.get_or_insert(failure);
                }
            }
        }

        // Star-imported variant constructors (spec 4.2: `import pkg.E.*`)
        // form the layer right before the implicit core prelude. Probe
        // every same-name target in isolated states so only one
        // applicable winner can commit; a layer without an applicable
        // candidate falls through to the prelude layer.
        let star_variants = self.star_variant_refs(&name).to_vec();
        let mut star_failure = None;
        let mut unit_only_star = Vec::new();
        let mut star_successes = Vec::new();
        for target in &star_variants {
            if self.resolved_variant_style(*target) == VariantStyle::Unit {
                unit_only_star.push(*target);
                continue;
            }
            match self.probe_expr_layer(|state, layer_sink| {
                state.lower_variant_construct(
                    *target,
                    CallSite {
                        type_args: &call.type_args,
                        args: &call.args,
                        span: call.span,
                    },
                    layer_sink,
                    expected,
                )
            }) {
                Ok(layer) => star_successes.push((*target, layer)),
                Err(failure) => {
                    star_failure.get_or_insert(failure);
                }
            }
        }
        match star_successes.len() {
            1 => {
                let (_, layer) = star_successes.pop().expect("one star-layer winner");
                return Some(self.commit_expr_layer(layer, sink));
            }
            2.. => {
                let targets = star_successes
                    .iter()
                    .map(|(target, _)| *target)
                    .collect::<Vec<_>>();
                self.ambiguous_prelude_variant(&call.callee, &targets);
                return None;
            }
            0 => {}
        }

        // Ordinary core-prelude variant constructors form their own layer
        // after source declarations. Probe every same-name target in an
        // isolated state so only one applicable winner can commit.
        let prelude = self.core_prelude_variant_refs(&name).to_vec();
        let mut prelude_successes = Vec::new();
        let mut prelude_failure = None;
        let mut unit_only_prelude = Vec::new();
        for target in prelude {
            if self.resolved_variant_style(target) == VariantStyle::Unit {
                unit_only_prelude.push(target);
                continue;
            }
            match self.probe_expr_layer(|state, layer_sink| {
                state.lower_variant_construct(
                    target,
                    CallSite {
                        type_args: &call.type_args,
                        args: &call.args,
                        span: call.span,
                    },
                    layer_sink,
                    expected,
                )
            }) {
                Ok(layer) => prelude_successes.push((target, layer)),
                Err(failure) => {
                    prelude_failure.get_or_insert(failure);
                }
            }
        }
        match prelude_successes.len() {
            1 => {
                let (_, layer) = prelude_successes.pop().expect("one prelude winner");
                return Some(self.commit_expr_layer(layer, sink));
            }
            2.. => {
                let targets = prelude_successes
                    .iter()
                    .map(|(target, _)| *target)
                    .collect::<Vec<_>>();
                self.ambiguous_prelude_variant(&call.callee, &targets);
                return None;
            }
            0 => {}
        }

        // The final layer is contextual and considers only the exact enum
        // application supplied by the expected type.
        let mut contextual_failure = None;
        if let Some(target) = self.contextual_variant_ref(&name, expected) {
            if self.resolved_variant_style(target) == VariantStyle::Unit {
                self.error(
                    call.span,
                    format!(
                        "unit variant `{}` of `{}` does not take arguments; use `{}` without parentheses",
                        call.callee.text,
                        self.enums[target.enumeration()].name,
                        call.callee.text
                    ),
                );
                return None;
            }
            match self.probe_expr_layer(|state, layer_sink| {
                state.lower_variant_construct(
                    target,
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
                    contextual_failure = Some(failure);
                }
            }
        }
        if let Some(failure) = contextual_failure {
            self.commit_layer_diagnostics(*failure);
            return None;
        }
        if let Some(enumeration) = self.exact_expected_enum(expected) {
            if let Some(failure) = ordinary_failure {
                self.commit_layer_diagnostics(*failure);
                return None;
            }
            self.error(
                call.callee.span,
                format!(
                    "enum `{}` has no variant `{}`",
                    self.enums[enumeration].name, call.callee.text
                ),
            );
            return None;
        }
        if let Some(failure) = ordinary_failure {
            self.commit_layer_diagnostics(*failure);
            return None;
        }
        if let Some(failure) = star_failure {
            self.commit_layer_diagnostics(*failure);
            return None;
        }
        if let Some(failure) = prelude_failure {
            self.commit_layer_diagnostics(*failure);
            return None;
        }
        if let Some(target) = unit_only_star.first() {
            let enumeration = target.enumeration();
            self.error(
                call.span,
                format!(
                    "unit variant `{}` of `{}` does not take arguments; use `{}` without parentheses",
                    call.callee.text, self.enums[enumeration].name, call.callee.text
                ),
            );
            return None;
        }
        if let Some(target) = unit_only_prelude.first() {
            let enumeration = target.enumeration();
            self.error(
                call.span,
                format!(
                    "unit variant `{}` of `{}` does not take arguments; use `{}` without parentheses",
                    call.callee.text, self.enums[enumeration].name, call.callee.text
                ),
            );
            return None;
        }
        if !found_top_level_candidate {
            let message = if self
                .functions_by_name
                .get(&name)
                .is_some_and(|candidates| !candidates.is_empty())
            {
                format!("function `{}` is not accessible here", call.callee.text)
            } else if self
                .properties_by_name
                .get(&name)
                .is_some_and(|candidates| {
                    !candidates.is_empty()
                        && candidates.iter().all(|property| {
                            !self.access_domain_allows(
                                &self.properties[*property].access.lookup.0,
                                None,
                            )
                        })
                })
            {
                format!("property `{}` is not accessible here", call.callee.text)
            } else if self.lexical_nested_nominal_target(&name).is_none()
                && self.source_type_alias_named(&name).is_some()
                && self.type_alias_is_accessible(&name)
            {
                format!("typealias `{name}` does not name a constructible type")
            } else {
                format!(
                    "unknown function `{}`; bare enum variants without an exact enum expected type must be qualified as `E.V` or given a type annotation",
                    call.callee.text
                )
            };
            self.error(call.callee.span, message);
            return None;
        }
        unreachable!("a discovered callable candidate either succeeded or recorded a failure")
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
                argument_protocol: crate::overload::CallArgumentProtocol::Ordinary,
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
                argument_protocol: crate::overload::CallArgumentProtocol::Ordinary,
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

    pub(crate) fn resolve_call_type_args(
        &mut self,
        refs: &[ast::CallTypeArgument],
    ) -> Option<Vec<ResolvedCallTypeArgument>> {
        refs.iter()
            .map(|argument| match argument {
                ast::CallTypeArgument::Explicit(type_ref) => {
                    self.resolve_type_ref(type_ref)
                        .map(|ty| ResolvedCallTypeArgument::Explicit {
                            ty,
                            span: type_ref.span,
                        })
                }
                ast::CallTypeArgument::Infer { span } => {
                    Some(ResolvedCallTypeArgument::Infer { span: *span })
                }
            })
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
