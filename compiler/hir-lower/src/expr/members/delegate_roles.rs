//! Delegate roles use the ordinary mixed local/dependency callable partition.

use super::*;
use crate::call_resolution::named::NamedFunctionLikeProbe;
use crate::expr::named_calls::imported_dependency::{ImportedMemberReceiver, ImportedProbeCall};
use crate::imports::lookup::calls::ExtensionCallTarget;
use crate::overload::{LoweredOverloadCall, NamedCallReceiver};
use crate::properties::{DelegateCallEffect, DelegateRoleCall, ResolvedDelegateRoleCall};

impl Lowerer {
    pub(crate) fn resolve_delegate_member_role(
        &mut self,
        receiver: hir::Expr,
        role: hir::PropertyDelegateOperatorKind,
        name: &ast::Ident,
        arguments: &[hir::Expr],
    ) -> DelegateRoleCall {
        let mut context = self.clone();
        let imported = match context.imported_member_call_candidates(
            receiver.ty,
            name,
            RequiredCallableModifiers {
                property_delegate_operator: Some(role),
                ..Default::default()
            },
            MemberCallKind::Ordinary,
        ) {
            Ok(candidates) => candidates,
            Err(failure) => {
                self.commit_layer_diagnostics(*failure);
                return DelegateRoleCall::Failed;
            }
        };
        let mut probes = Vec::new();
        let mut suppressed = false;
        for candidate in context.methods_by_name(receiver.ty, &name.text) {
            if context.signatures[&candidate.function]
                .modifiers
                .property_delegate_operator
                != Some(role)
            {
                continue;
            }
            if context
                .declaration_surface
                .rejects_function(candidate.function)
            {
                suppressed = true;
                continue;
            }
            if let Ok(probe) = context.probe_lowered_named_callable(
                &name.text,
                candidate,
                NamedCallReceiver::Member(receiver.clone()),
                lowered_call(arguments, name.span),
            ) {
                probes.push(NamedFunctionLikeProbe::Callable(Box::new(probe)));
            }
        }
        for candidate in imported {
            if let Ok(probe) = context.probe_imported_member_callable(
                candidate,
                ImportedMemberReceiver::Value(receiver.clone()),
                name,
                ImportedProbeCall::lowered(arguments, name.span),
                None,
                false,
            ) {
                probes.push(NamedFunctionLikeProbe::ImportedDependency(Box::new(probe)));
            }
        }
        if probes.is_empty() && suppressed {
            return DelegateRoleCall::Failed;
        }
        match context.finish_delegate_role_partition(name, "member", probes) {
            DelegateRoleCall::Resolved(resolved) => {
                *self = context;
                DelegateRoleCall::Resolved(resolved)
            }
            DelegateRoleCall::Failed => {
                self.commit_layer_diagnostics(context);
                DelegateRoleCall::Failed
            }
            DelegateRoleCall::NoApplicable => DelegateRoleCall::NoApplicable,
        }
    }

    pub(crate) fn resolve_delegate_extension_role(
        &mut self,
        candidates: &[ExtensionCallTarget],
        receiver: hir::Expr,
        name: &ast::Ident,
        arguments: &[hir::Expr],
    ) -> DelegateRoleCall {
        let mut probes = Vec::new();
        let mut suppressed = false;
        for candidate in candidates {
            let probe = match candidate {
                ExtensionCallTarget::Current(function) => {
                    if self.declaration_surface.rejects_function(*function) {
                        suppressed = true;
                        continue;
                    }
                    self.probe_lowered_named_callable(
                        &name.text,
                        crate::CallableCandidate::function(*function, Vec::new()),
                        NamedCallReceiver::Extension(receiver.clone()),
                        lowered_call(arguments, name.span),
                    )
                    .map(|probe| NamedFunctionLikeProbe::Callable(Box::new(probe)))
                }
                ExtensionCallTarget::Dependency(binding) => self
                    .probe_imported_delegate_extension_callable(
                        binding,
                        receiver.clone(),
                        name,
                        ImportedProbeCall::lowered(arguments, name.span),
                    )
                    .map(|probe| NamedFunctionLikeProbe::ImportedDependency(Box::new(probe))),
            };
            if let Ok(probe) = probe {
                probes.push(probe);
            }
        }
        if probes.is_empty() && suppressed {
            return DelegateRoleCall::Failed;
        }
        self.finish_delegate_role_partition(name, "extension", probes)
    }

    fn finish_delegate_role_partition(
        &mut self,
        name: &ast::Ident,
        layer: &str,
        mut probes: Vec<NamedFunctionLikeProbe>,
    ) -> DelegateRoleCall {
        if probes.is_empty() {
            return DelegateRoleCall::NoApplicable;
        }
        let Some(winner) =
            self.select_lowered_named_function_like(&name.text, layer, &probes, name.span)
        else {
            return DelegateRoleCall::Failed;
        };
        let (expression, effect) = match probes.swap_remove(winner) {
            NamedFunctionLikeProbe::Callable(probe) => {
                let mut sink = Vec::new();
                let Some(resolved) = self.commit_named_callable(*probe, &mut sink) else {
                    return DelegateRoleCall::Failed;
                };
                debug_assert!(sink.is_empty(), "delegate arguments are already lowered");
                let callable = self.materialize_resolved_callee(&resolved);
                let expression = if resolved.receiver.is_some() {
                    self.finish_resolved_method_call(resolved, name.span, &mut sink)
                } else {
                    self.check_call_effects(callable, name.span);
                    Some(hir::Expr {
                        kind: hir::ExprKind::Call {
                            binding: None,
                            callee: (callable).into(),
                            receiver: resolved.source_receiver,
                            args: resolved.args,
                        },
                        ty: resolved.return_ty,
                        span: name.span,
                        origin: self.expression_origin(name.span),
                    })
                };
                (expression, DelegateCallEffect::Current(callable))
            }
            NamedFunctionLikeProbe::ImportedDependency(probe) => {
                let effect = DelegateCallEffect::Imported(probe.safety());
                (self.commit_imported_lowered_callable(*probe), effect)
            }
            _ => unreachable!("delegate partitions contain only callable declarations"),
        };
        match expression {
            Some(expression) => {
                DelegateRoleCall::Resolved(ResolvedDelegateRoleCall { expression, effect })
            }
            None => DelegateRoleCall::Failed,
        }
    }
}

fn lowered_call(arguments: &[hir::Expr], span: ast::Span) -> LoweredOverloadCall {
    LoweredOverloadCall {
        explicit_type_args: Vec::new(),
        args: arguments.to_vec(),
        span,
        expected_result: None,
    }
}
