//! Pairwise declaration forwarding for most-specific-candidate selection.

use scoop_hir as hir;

use super::candidates::CallableView;
use super::constraints::{Constraint, ConstraintOrigin, InferenceSession, TypeTerm};
use crate::Lowerer;

pub(crate) struct ForwardingDeclaration<'a> {
    pub(crate) view: &'a CallableView,
    /// The complete M16 forwarding list. Extension receivers are prepended by
    /// the caller; ordinary receivers never enter MSC.
    pub(crate) parameter_types: &'a [hir::TypeId],
}

impl Lowerer {
    /// Whether every value accepted by `source` can be forwarded to `target`.
    ///
    /// Both source parameter groups stay rigid skolems in a cloned semantic
    /// state. Both target groups receive fresh variables. The comparison thus
    /// depends only on declarations, never on the current call's inferred
    /// concrete arguments (including receiver owner arguments).
    pub(crate) fn callable_forwards(
        &self,
        source: ForwardingDeclaration<'_>,
        target: ForwardingDeclaration<'_>,
    ) -> bool {
        if source.parameter_types.len() != target.parameter_types.len() {
            return false;
        }
        let mut probe = self.clone();
        // `is_subtype(Type::Param(..), ..)` consumes declaration bounds from
        // this scope, so install the source skolems and their complete bounds.
        for parameter in source
            .view
            .owner_parameters
            .iter()
            .chain(&source.view.callable_parameters)
        {
            probe
                .type_params_in_scope
                .retain(|candidate| candidate.id != parameter.id);
            probe.type_params_in_scope.push(parameter.clone());
        }

        let mut session = InferenceSession::new();
        session.add_environment(
            &target.view.owner_parameters,
            &target.view.callable_parameters,
        );
        probe.add_declaration_bounds(
            &mut session,
            target
                .view
                .owner_parameters
                .iter()
                .chain(&target.view.callable_parameters),
        );
        for (&source, &target) in source.parameter_types.iter().zip(target.parameter_types) {
            session.push(
                Constraint::Subtype(TypeTerm::Rigid(source), TypeTerm::Type(target)),
                ConstraintOrigin::Specificity,
            );
        }
        probe.constraints_are_satisfiable(&session)
    }
}
