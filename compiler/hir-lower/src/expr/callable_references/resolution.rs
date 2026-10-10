//! Candidate layering and expected-type-driven callable-reference resolution.

use super::*;
use crate::call_resolution::specificity::{ApplicableDeclaration, OwnedDeclarationForwarding};
use crate::imports::lookup::calls::{
    ExtensionCallTarget, NamedCallBinding, NamedCallOrigin, NamedCallTarget,
};

mod diagnostics;
mod imported;
mod local;

use imported::ImportedReferenceDeclaration;
use local::LocalReferenceDeclaration;

#[derive(Clone)]
pub(super) enum ReferenceCandidate {
    Local(crate::CallableCandidate, Option<hir::Expr>),
    Dependency(hir::DirectImportedTargetBinding),
    Member(Box<hir::ImportedCallableDeclaration>, hir::Expr),
}

impl ReferenceCandidate {
    pub(super) fn named(binding: NamedCallBinding) -> Self {
        match (binding.target, binding.origin) {
            (NamedCallTarget::Function(function), _) => Self::Local(
                crate::CallableCandidate::function(function, Vec::new()),
                None,
            ),
            (NamedCallTarget::ImportedDependency(_), NamedCallOrigin::Dependency(binding)) => {
                Self::Dependency(binding)
            }
            _ => unreachable!("reference lookup returns function declarations"),
        }
    }

    pub(super) fn extension(target: ExtensionCallTarget) -> Self {
        match target {
            ExtensionCallTarget::Current(function) => Self::Local(
                crate::CallableCandidate::function(function, Vec::new()),
                None,
            ),
            ExtensionCallTarget::Dependency(binding) => Self::Dependency(binding),
        }
    }
}

enum ReferenceDeclaration {
    Local(LocalReferenceDeclaration),
    Imported(ImportedReferenceDeclaration),
}

struct ApplicableReference {
    state: Box<Lowerer>,
    declaration: ReferenceDeclaration,
    type_args: Vec<TypeId>,
    ty: TypeId,
    own_type_param_count: usize,
    receiver: Option<hir::Expr>,
}

struct ReferenceFailure {
    signature: String,
    layer: &'static str,
    reason: String,
}

impl ReferenceDeclaration {
    fn forwarding(&self, state: &mut Lowerer) -> OwnedDeclarationForwarding {
        match self {
            Self::Local(declaration) => declaration.forwarding(),
            Self::Imported(declaration) => declaration.forwarding(state),
        }
    }

    fn commit(
        self,
        state: &mut Lowerer,
        type_args: &[TypeId],
        receiver: Option<&hir::Expr>,
        ty: TypeId,
    ) -> Result<hir::CallableReferenceTarget, String> {
        match self {
            Self::Local(declaration) => Ok(declaration.commit(state, type_args, receiver)),
            Self::Imported(declaration) => declaration.commit(state, type_args, receiver, ty),
        }
    }
}

impl Lowerer {
    pub(super) fn is_declared_type_name(&self, name: &ast::Ident) -> bool {
        self.lexical_nested_nominal_target(&name.text).is_some()
            || self.top_level_type_target(&name.text).is_some()
            || matches!(
                self.lookup_expression_qualifier(name),
                crate::imports::lookup::calls::ExpressionQualifierLookup::Unique(
                    crate::imports::lookup::calls::ExpressionQualifierTarget::DependencyType(_)
                )
            )
    }

    pub(super) fn expected_function_signature(
        &self,
        expected: Option<TypeId>,
    ) -> Option<(TypeId, hir::FunctionType)> {
        expected.and_then(|ty| match self.types[ty] {
            Type::Function(id) => Some((ty, self.function_types[id].clone())),
            _ => None,
        })
    }

    pub(super) fn resolve_reference_candidates(
        &mut self,
        candidates: &[hir::FunctionId],
        owner_type_args: &[TypeId],
        context: ReferenceResolutionContext<'_>,
    ) -> ReferenceResolutionOutcome {
        let candidates = candidates
            .iter()
            .copied()
            .map(|function| {
                ReferenceCandidate::Local(
                    crate::CallableCandidate::function(function, owner_type_args.to_vec()),
                    None,
                )
            })
            .collect::<Vec<_>>();
        self.resolve_reference_candidate_set(&candidates, None, context)
    }

    pub(super) fn resolve_reference_candidate_set(
        &mut self,
        candidates: &[ReferenceCandidate],
        receiver: Option<&hir::Expr>,
        context: ReferenceResolutionContext<'_>,
    ) -> ReferenceResolutionOutcome {
        let rejected = |candidate: &ReferenceCandidate| {
            matches!(candidate,
            ReferenceCandidate::Local(candidate, _) if self.declaration_surface.rejects_function(candidate.function))
        };
        let suppressed = candidates.iter().any(rejected);
        let candidates = candidates
            .iter()
            .filter(|candidate| !rejected(candidate))
            .collect::<Vec<_>>();
        if candidates.is_empty() && suppressed {
            return ReferenceResolutionOutcome::Blocked;
        }
        let mut applicable = Vec::new();
        let mut failures = Vec::new();
        for candidate in candidates {
            let views = match candidate {
                ReferenceCandidate::Local(_, Some(view)) | ReferenceCandidate::Member(_, view) => {
                    vec![Some(view.clone())]
                }
                _ => receiver
                    .map(|receiver| {
                        self.smart_cast_receiver_views(receiver)
                            .into_iter()
                            .map(Some)
                            .collect()
                    })
                    .unwrap_or_else(|| vec![None]),
            };
            for view in views {
                let probe_context = ReferenceResolutionContext {
                    extension_mode: match (context.extension_mode, &view) {
                        (ReferenceExtensionMode::Bound(_), Some(view)) => {
                            ReferenceExtensionMode::Bound(view.ty)
                        }
                        (mode, _) => mode,
                    },
                    ..context
                };
                let result = match candidate {
                    ReferenceCandidate::Local(candidate, _) => {
                        self.probe_local_reference(candidate, probe_context)
                    }
                    ReferenceCandidate::Dependency(_) | ReferenceCandidate::Member(..) => self
                        .probe_imported_reference(
                            candidate,
                            view.as_ref().map(|value| value.ty),
                            probe_context,
                        ),
                };
                match result {
                    Ok(Some(mut candidate)) => {
                        candidate.receiver = view;
                        applicable.push(candidate);
                        break;
                    }
                    Ok(None) => continue,
                    Err(failure) => failures.push(failure),
                }
            }
        }
        let selected = match applicable.len() {
            0 => {
                self.reference_failures_diagnostic(context, &failures);
                return if suppressed {
                    ReferenceResolutionOutcome::Failed
                } else {
                    ReferenceResolutionOutcome::NoApplicable
                };
            }
            1 => 0,
            _ if context.expected.is_none() => {
                self.reference_ambiguity_diagnostic(
                    context,
                    &applicable.iter().collect::<Vec<_>>(),
                );
                return ReferenceResolutionOutcome::Failed;
            }
            _ => {
                // Re-resolve declaration types into one comparison arena. Probe-local
                // type IDs and inferred arguments must never enter another probe's MSC.
                let mut comparison = self.clone();
                let declarations = applicable
                    .iter()
                    .map(|candidate| candidate.declaration.forwarding(&mut comparison))
                    .collect::<Vec<_>>();
                let declarations = declarations
                    .iter()
                    .zip(&applicable)
                    .map(|(declaration, candidate)| ApplicableDeclaration {
                        declaration: declaration.as_view(),
                        parameterized: candidate.own_type_param_count != 0,
                        defaults: 0,
                        vararg: false,
                    })
                    .collect::<Vec<_>>();
                let pool = comparison.most_specific_declarations(&declarations);
                if let [winner] = pool.as_slice() {
                    *winner
                } else {
                    let ambiguous = pool
                        .iter()
                        .map(|&index| &applicable[index])
                        .collect::<Vec<_>>();
                    self.reference_ambiguity_diagnostic(context, &ambiguous);
                    return ReferenceResolutionOutcome::Failed;
                }
            }
        };
        let selected = applicable.swap_remove(selected);
        *self = *selected.state;
        match selected.declaration.commit(
            self,
            &selected.type_args,
            selected.receiver.as_ref(),
            selected.ty,
        ) {
            Ok(target) => ReferenceResolutionOutcome::Resolved(ResolvedReference {
                target,
                type_args: selected.type_args,
                ty: selected.ty,
            }),
            Err(message) => {
                self.error(context.span, message);
                ReferenceResolutionOutcome::Failed
            }
        }
    }
}
