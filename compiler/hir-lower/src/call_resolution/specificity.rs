//! Pairwise declaration forwarding for most-specific-candidate selection.

use scoop_hir as hir;

use super::candidates::{CallableView, NominalConstructorView};
use super::constraints::{Constraint, ConstraintOrigin, InferenceSession, TypeTerm};
use crate::Lowerer;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
pub(crate) struct ForwardingDeclaration<'a> {
    pub(crate) view: &'a CallableView,
    /// The complete M16 forwarding list. Extension receivers are prepended by
    /// the caller; ordinary receivers never enter MSC.
    pub(crate) parameter_types: &'a [hir::TypeId],
}

#[derive(Clone, Copy)]
pub(crate) struct NominalForwardingDeclaration<'a> {
    pub(crate) view: &'a NominalConstructorView,
    pub(crate) parameter_types: &'a [hir::TypeId],
}

/// Declaration binders and the mapped source-input types used by MSC.
/// Owner and callable groups remain distinct; no callable identity is needed
/// and constructor declarations never masquerade as functions.
#[derive(Clone, Copy)]
pub(crate) struct DeclarationForwardingView<'a> {
    owner_parameters: &'a [hir::TypeParamDecl],
    callable_parameters: &'a [hir::TypeParamDecl],
    parameter_types: &'a [hir::TypeId],
}

impl<'a> From<ForwardingDeclaration<'a>> for DeclarationForwardingView<'a> {
    fn from(declaration: ForwardingDeclaration<'a>) -> Self {
        Self {
            owner_parameters: &declaration.view.owner_parameters,
            callable_parameters: &declaration.view.callable_parameters,
            parameter_types: declaration.parameter_types,
        }
    }
}

impl<'a> From<NominalForwardingDeclaration<'a>> for DeclarationForwardingView<'a> {
    fn from(declaration: NominalForwardingDeclaration<'a>) -> Self {
        Self {
            owner_parameters: &declaration.view.owner_parameters,
            callable_parameters: &[],
            parameter_types: declaration.parameter_types,
        }
    }
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
        self.declaration_forwards(source.into(), target.into())
    }

    pub(crate) fn nominal_constructor_forwards(
        &self,
        source: NominalForwardingDeclaration<'_>,
        target: NominalForwardingDeclaration<'_>,
    ) -> bool {
        self.declaration_forwards(source.into(), target.into())
    }

    pub(crate) fn declaration_forwards(
        &self,
        source: DeclarationForwardingView<'_>,
        target: DeclarationForwardingView<'_>,
    ) -> bool {
        if source.parameter_types.len() != target.parameter_types.len() {
            return false;
        }
        let mut probe = self.clone();
        // `is_subtype(Type::Param(..), ..)` consumes declaration bounds from
        // this scope, so install the source skolems and their complete bounds.
        for parameter in source
            .owner_parameters
            .iter()
            .chain(source.callable_parameters)
        {
            probe
                .type_params_in_scope
                .retain(|candidate| candidate.id != parameter.id);
            probe.type_params_in_scope.push(parameter.clone());
        }

        let mut session = InferenceSession::new();
        session.add_environment(target.owner_parameters, target.callable_parameters);
        probe.add_declaration_bounds(
            &mut session,
            target
                .owner_parameters
                .iter()
                .chain(target.callable_parameters),
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
