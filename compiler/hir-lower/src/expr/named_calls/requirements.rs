//! Typed capability queries share the ordinary callable and constructor probes.

use super::*;
use hir::{MemberRequirementBindingValue as Binding, MemberRequirementContract as Contract};

mod calls;
mod constructors;
mod methods;
mod scope;

impl Lowerer {
    pub(crate) fn check_member_requirement(
        &mut self,
        id: hir::MemberRequirementId,
        contract: &Contract,
    ) -> Result<Binding, String> {
        if self.active_member_requirements.contains(&id) {
            return Err("recursive member-requirement predicate".into());
        }
        let key = contract.key(Ok::<_, std::convert::Infallible>).unwrap();
        if let Some(binding) = self.checked_member_requirements.get(&key) {
            return Ok(binding.clone());
        }
        if matches!(
            self.types[contract.subject],
            Type::Param(_) | Type::Companion(_)
        ) {
            let binding = self
                .type_params_in_scope
                .iter()
                .flat_map(|parameter| &parameter.requirements)
                .find(|requirement| requirement.contract == *contract)
                .map(|requirement| Binding::Forwarded(requirement.id))
                .ok_or_else(|| {
                    format!(
                        "type `{}` does not provide the exact member requirement",
                        self.type_name(contract.subject)
                    )
                })?;
            self.checked_member_requirements
                .insert(key, binding.clone());
            return Ok(binding);
        }
        self.active_member_requirements.insert(id);
        let result = self.build_member_requirement(contract);
        self.active_member_requirements.remove(&id);
        let implementation = result?;
        if contract.effects.gc_effect == hir::GcEffect::NoGc {
            self.check_requirement_no_gc_body(&implementation.body, &implementation.result)?;
        }
        let binding = Binding::Implementation {
            implementation: self
                .member_requirement_implementations
                .alloc(implementation),
            type_arguments: self.requirement_type_arguments(contract),
        };
        self.checked_member_requirements
            .insert(key, binding.clone());
        Ok(binding)
    }

    fn select_member_requirement(
        &mut self,
        contract: &Contract,
        receiver: Option<hir::Expr>,
        arguments: &[ast::CallArgument],
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Result<hir::Expr, String> {
        let (name, applicable) = match &contract.call {
            hir::RequiredCallKind::Method {
                name,
                type_arguments,
            } => (
                name.clone(),
                self.requirement_method_probes(
                    contract.subject,
                    receiver.expect("a method requirement has a receiver"),
                    name,
                    type_arguments,
                    arguments,
                    span,
                )?,
            ),
            hir::RequiredCallKind::Constructor => (
                self.type_name(contract.subject),
                self.requirement_constructor_probes(contract.subject, arguments, span)?,
            ),
        };
        let (mut probes, mut commits): (Vec<_>, Vec<_>) = applicable
            .into_iter()
            .map(|value| (value.probe, value.commit))
            .unzip();
        if probes.is_empty() {
            return Err(format!(
                "type `{}` has no applicable public member `{name}`",
                self.type_name(contract.subject)
            ));
        }
        let winner = self
            .select_named_function_like(&name, "required member", &probes, arguments, span)
            .ok_or_else(|| self.requirement_failure("ambiguous required member"))?;
        let probe = probes.swap_remove(winner);
        let commit = commits.swap_remove(winner);
        let effects = match &probe {
            NamedFunctionLikeProbe::Callable(probe) => probe.requirement_effects(),
            NamedFunctionLikeProbe::ImportedDependency(probe) => probe.requirement_effects(),
            NamedFunctionLikeProbe::Nominal(probe) => probe.requirement_effects(),
            NamedFunctionLikeProbe::ImportedDerivedEquality(probe) => hir::RequiredCallEffects {
                is_suspend: false,
                safety: if self.requires_unsafe_use(probe.owner) {
                    hir::Safety::Unsafe
                } else {
                    hir::Safety::Safe
                },
                gc_effect: hir::GcEffect::Managed,
            },
            NamedFunctionLikeProbe::IntrinsicStruct(_) => hir::RequiredCallEffects {
                is_suspend: false,
                safety: hir::Safety::Unsafe,
                gc_effect: hir::GcEffect::NoGc,
            },
            _ => unreachable!("requirement probes contain only members and constructors"),
        };
        if effects.is_suspend != contract.effects.is_suspend
            || effects.safety == hir::Safety::Unsafe && contract.effects.safety == hir::Safety::Safe
            || effects.gc_effect == hir::GcEffect::Managed
                && contract.effects.gc_effect == hir::GcEffect::NoGc
        {
            return Err(format!(
                "selected member `{name}` does not satisfy the required effects"
            ));
        }
        let expression = match probe {
            NamedFunctionLikeProbe::Callable(probe) => self
                .commit_named_callable(*probe, sink)
                .and_then(|resolved| self.finish_resolved_method_call(resolved, span, sink)),
            NamedFunctionLikeProbe::ImportedDependency(probe) => {
                self.commit_imported_dependency_callable(*probe, sink)
            }
            NamedFunctionLikeProbe::Nominal(probe) => self
                .commit_named_nominal(*probe, span, sink)
                .and_then(|resolved| self.finish_named_nominal(resolved, span)),
            NamedFunctionLikeProbe::ImportedDerivedEquality(probe) => {
                Some(probe.commit(self, sink))
            }
            NamedFunctionLikeProbe::IntrinsicStruct(_) => {
                let NamedFunctionCommit::Intrinsic(layer) = commit else {
                    unreachable!("an intrinsic constructor retains its typed result")
                };
                Some(self.commit_expr_layer(layer, sink))
            }
            _ => unreachable!("requirement probes contain only members and constructors"),
        }
        .ok_or_else(|| self.requirement_failure("required member call is invalid"))?;
        if !self.is_subtype(expression.ty, contract.result) {
            return Err(format!(
                "selected member `{name}` returns `{}`, required `{}`",
                self.type_name(expression.ty),
                self.type_name(contract.result)
            ));
        }
        Ok(self.adapt_to(expression, contract.result))
    }

    fn requirement_failure(&self, fallback: &str) -> String {
        self.diagnostics
            .last()
            .map_or_else(|| fallback.into(), |diagnostic| diagnostic.message.clone())
    }
}
