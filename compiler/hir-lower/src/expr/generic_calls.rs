//! Generic call resolution, argument inference and concrete callable binding.

use super::*;

mod intrinsics;

impl Lowerer {
    pub(in crate::expr) fn finish_named_call_fallback(
        &mut self,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
        ordinary_failure: Option<Box<Lowerer>>,
        prelude_variant_failure: Option<Box<Lowerer>>,
        found: bool,
    ) -> Option<hir::Expr> {
        let name = &call.callee.text;
        if let Some((owner, index)) = self.contextual_imported_variant(name, expected) {
            return self.lower_imported_variant_construct(
                owner,
                index,
                &call.callee,
                CallSite {
                    type_args: &call.type_args,
                    args: &call.args,
                    span: call.span,
                },
                sink,
                expected,
            );
        }
        if let Some(target) = self.contextual_variant_ref(name, expected) {
            match self.probe_expr_layer(|state, sink| {
                state.lower_variant_construct(
                    target,
                    CallSite {
                        type_args: &call.type_args,
                        args: &call.args,
                        span: call.span,
                    },
                    sink,
                    expected,
                )
            }) {
                Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                Err(failure) => {
                    self.commit_layer_diagnostics(*failure);
                    return None;
                }
            }
        }
        if let Some(failure) = ordinary_failure {
            self.commit_layer_diagnostics(*failure);
            return None;
        }
        if let Some(enumeration) = self.exact_expected_enum(expected) {
            self.error(
                call.callee.span,
                format!(
                    "enum `{}` has no variant `{name}`",
                    self.enums[enumeration].name
                ),
            );
        } else if let Some(failure) = prelude_variant_failure {
            self.commit_layer_diagnostics(*failure);
        } else if matches!(
            self.lookup_type(name),
            crate::imports::lookup::LookupResult::Inaccessible(_)
        ) {
            match self.resolve_type_lookup(&call.callee) {
                Ok(Some(crate::imports::lookup::TypeLookupTarget::Current(
                    crate::namespace::TopLevelTypeTarget::Alias(alias),
                ))) => {
                    let _ = self.resolve_type_alias_id_reference(alias, &call.callee, false);
                }
                Ok(Some(crate::imports::lookup::TypeLookupTarget::Current(
                    crate::namespace::TopLevelTypeTarget::Nominal(_),
                ))) => self.error(
                    call.callee.span,
                    format!("type `{name}` is not accessible from this source location"),
                ),
                Ok(Some(crate::imports::lookup::TypeLookupTarget::Dependency(binding))) => {
                    let _ = self.resolve_imported_dependency_type_target(
                        &binding,
                        &call.callee,
                        !call.type_args.is_empty(),
                    );
                }
                Ok(None) | Err(()) => {}
            }
        } else if self.has_top_level_function_candidate(name) && !found {
            self.error(
                call.callee.span,
                format!("function `{name}` is not accessible here"),
            );
        } else {
            self.error(call.callee.span, format!("unknown function `{name}`; bare enum variants without an exact enum expected type must be qualified as `E.V` or given a type annotation"));
        }
        None
    }

    pub(in crate::expr) fn lower_local_function_layer(
        &mut self,
        name: &str,
        candidates: &[hir::LocalFunctionId],
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Result<Option<hir::Expr>, ()> {
        let functions: Vec<_> = candidates
            .iter()
            .map(|id| self.local_functions[*id].source_function())
            .collect();
        let owner_count = self.local_functions[candidates[0]].owner_type_param_count;
        let owner_type_args = self.ambient_type_args(owner_count);
        let explicit_type_args = self.resolve_call_type_args(&call.type_args).ok_or(())?;
        let resolved = match self.resolve_overload_outcome(
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
        ) {
            crate::overload::OverloadResolutionOutcome::NoApplicable => return Ok(None),
            crate::overload::OverloadResolutionOutcome::Blocked => return Ok(None),
            crate::overload::OverloadResolutionOutcome::Failed => return Err(()),
            crate::overload::OverloadResolutionOutcome::Resolved(resolved) => *resolved,
        };
        let function = resolved.function();
        let local_function = self.local_function_by_function[&function];
        let mut args = self
            .local_call_capture_args(local_function, call.span)
            .ok_or(())?;
        let callee = self.materialize_resolved_callee(&resolved);
        self.check_call_effects(callee, call.span);
        args.extend(resolved.args);
        Ok(Some(hir::Expr {
            kind: ExprKind::Call {
                callee: hir::CallableTarget::Local(callee),
                binding: None,
                receiver: hir::SourceCallReceiver::NoReceiver,
                args,
            },
            ty: resolved.return_ty,
            span: call.span,
            origin: self.expression_origin(call.span),
        }))
    }

    pub(in crate::expr) fn lower_top_level_function_layer(
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
        self.finish_resolved_top_level_function_call(call, resolved, sink)
    }

    pub(in crate::expr) fn finish_resolved_top_level_function_call(
        &mut self,
        call: &ast::CallExpr,
        resolved: crate::overload::ResolvedCallee,
        sink: &mut [hir::Statement],
    ) -> Option<hir::Expr> {
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
                binding: None,
                callee: callee.into(),
                receiver: resolved.source_receiver,
                args: resolved.args,
            },
            ty,
            span: call.span,
            origin: self.expression_origin(call.span),
        })
    }

    pub(crate) fn ambient_type_args(&mut self, count: usize) -> Vec<TypeId> {
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
            if let Some(&(parameter, _, _)) = self
                .constructor_params_in_scope
                .values()
                .find(|&&(_, _, candidate)| candidate == binding)
            {
                args.push(hir::Expr {
                    kind: ExprKind::ConstructorParam(parameter),
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
