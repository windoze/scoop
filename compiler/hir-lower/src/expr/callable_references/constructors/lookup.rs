use super::*;
use crate::expr::named_calls::nominals::PreparedNominalPlans;
use crate::imports::lookup::calls::{NamedCallBinding, NamedCallTarget};

impl Lowerer {
    pub(in crate::expr::callable_references) fn named_reference_candidates(
        &mut self,
        bindings: Vec<NamedCallBinding>,
        name: &ast::Ident,
        span: Span,
    ) -> Result<Vec<ReferenceCandidate>, ()> {
        let call = ast::CallExpr {
            callee: name.clone(),
            type_args: Vec::new(),
            args: Vec::new(),
            span,
        };
        let mut candidates = Vec::new();
        let mut first_failure = None;
        for binding in bindings {
            if matches!(
                binding.target,
                NamedCallTarget::Function(_)
                    | NamedCallTarget::ImportedDependency(
                        hir::ImportedTarget::Function(_) | hir::ImportedTarget::GenericFunction(_)
                    )
            ) {
                candidates.push(ReferenceCandidate::named(binding));
                continue;
            }
            let mut state = self.clone();
            let result = state
                .named_nominal_plans(&binding, &call, None)
                .and_then(|plans| state.reference_constructor_plans(plans, name, span));
            match result {
                Ok(prepared) => {
                    *self = state;
                    candidates.extend(prepared);
                }
                Err(()) => {
                    first_failure.get_or_insert(state);
                }
            }
        }
        if candidates.is_empty()
            && let Some(failure) = first_failure
        {
            self.commit_layer_diagnostics(failure);
            return Err(());
        }
        Ok(candidates)
    }

    fn reference_constructor_plans(
        &mut self,
        plans: PreparedNominalPlans,
        name: &ast::Ident,
        span: Span,
    ) -> Result<Vec<ReferenceCandidate>, ()> {
        let mut candidates = Vec::new();
        match plans {
            PreparedNominalPlans::Nominal(plans, intrinsic) => {
                if intrinsic.is_some() {
                    self.error(
                        span,
                        format!("type `{}` has no ordinary constructor reference", name.text),
                    );
                    return Err(());
                }
                candidates.extend(plans.into_iter().map(|plan| {
                    ReferenceCandidate::Constructor(ConstructorReferenceCandidate {
                        source: ConstructorReferenceSource::Local(plan.view.target),
                        fixed: plan.fixed_alias.then_some(plan.expected).flatten(),
                    })
                }));
            }
            PreparedNominalPlans::Imported {
                owner,
                expected,
                fixed_alias,
            } => {
                let declarations = self.imported_reference_constructors(owner, name, span)?;
                candidates.extend(declarations.into_iter().map(|declaration| {
                    ReferenceCandidate::Constructor(ConstructorReferenceCandidate {
                        source: ConstructorReferenceSource::Imported(Box::new(declaration)),
                        fixed: fixed_alias.then_some(expected).flatten(),
                    })
                }));
            }
        }
        Ok(candidates)
    }

    pub(in crate::expr::callable_references) fn lower_lexical_constructor_reference(
        &mut self,
        nominal: crate::NominalTarget,
        name: &ast::Ident,
        span: Span,
        expected: Option<TypeId>,
    ) -> Result<Option<hir::Expr>, ()> {
        let call = ast::CallExpr {
            callee: name.clone(),
            type_args: Vec::new(),
            args: Vec::new(),
            span,
        };
        let plans = self.nominal_constructor_plans(nominal, &call, None, false)?;
        let candidates = self.reference_constructor_plans(plans, name, span)?;
        let expected = self.expected_function_signature(expected);
        let display = format!("constructor reference `::{}`", name.text);
        match self.resolve_reference_candidate_set(
            &candidates,
            None,
            ReferenceResolutionContext {
                expected: expected.as_ref(),
                name: &name.text,
                display: &display,
                span,
                extension_mode: ReferenceExtensionMode::Exclude,
            },
        ) {
            ReferenceResolutionOutcome::Resolved(resolved) => self
                .finish_named_callable_reference(resolved, span)
                .map(Some)
                .ok_or(()),
            ReferenceResolutionOutcome::NoApplicable => Ok(None),
            ReferenceResolutionOutcome::Blocked | ReferenceResolutionOutcome::Failed => Err(()),
        }
    }

    fn imported_reference_constructors(
        &mut self,
        owner: hir::SourceNominalId,
        name: &ast::Ident,
        span: Span,
    ) -> Result<Vec<hir::ImportedCallableDeclaration>, ()> {
        let dependencies = self
            .dependencies
            .as_ref()
            .expect("an imported constructor has a dependency catalog");
        let declaration = dependencies
            .nominal_declaration(owner)
            .expect("a resolved imported nominal has its declaration");
        if declaration.interface.declaration_details().modality()
            == hir::NominalInheritanceModalityV1::Abstract
        {
            self.error(
                span,
                format!("abstract class `{}` cannot be instantiated", name.text),
            );
            return Err(());
        }
        if let crate::CoreLoweringAuthority::Imported(core) = &self.core
            && owner
                == hir::SourceNominalId::GenericTemplate(
                    core.foreign_callbacks().callback().persistent(),
                )
        {
            self.error(
                span,
                "`ForeignCallback` values can only be produced by `foreignCallback`".into(),
            );
            return Err(());
        }
        let candidates = match dependencies.constructor_candidates(owner) {
            Ok(candidates) => candidates,
            Err(error) => {
                self.error(
                    span,
                    format!("invalid dependency constructor declaration: {error}"),
                );
                return Err(());
            }
        };
        if candidates.is_empty() {
            self.error(
                span,
                format!("type `{}` does not name a constructible type", name.text),
            );
            return Err(());
        }
        let candidates = candidates
            .into_iter()
            .filter(|candidate| self.imported_callable_is_accessible(candidate.interface(), None))
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            self.error(
                span,
                format!("constructor `{}` is not accessible here", name.text),
            );
            return Err(());
        }
        Ok(candidates)
    }
}
