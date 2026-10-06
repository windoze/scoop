//! Pairwise declaration forwarding for most-specific-candidate selection.

use scoop_hir as hir;

use super::candidates::{CallableView, NominalConstructorView};
use super::constraints::{Constraint, ConstraintOrigin, InferenceSession, TypeTerm};
use crate::Lowerer;

mod literals;
mod selection;
pub(crate) use literals::{NumericLiteralKind, prefer_literal_defaults};
pub(crate) use selection::ApplicableDeclaration;

#[cfg(test)]
mod tests;

/// Declaration binders and the mapped source-input types used by MSC.
/// Owner and callable groups remain distinct; no callable identity is needed
/// and constructor declarations never masquerade as functions.
#[derive(Clone, Copy)]
pub(crate) struct DeclarationForwardingView<'a> {
    owner_parameters: &'a [hir::TypeParamDecl],
    callable_parameters: &'a [hir::TypeParamDecl],
    parameter_types: &'a [hir::TypeId],
}

pub(crate) struct OwnedDeclarationForwarding {
    owner_parameters: Vec<hir::TypeParamDecl>,
    callable_parameters: Vec<hir::TypeParamDecl>,
    parameter_types: Vec<hir::TypeId>,
}

impl OwnedDeclarationForwarding {
    pub(crate) fn as_view(&self) -> DeclarationForwardingView<'_> {
        DeclarationForwardingView {
            owner_parameters: &self.owner_parameters,
            callable_parameters: &self.callable_parameters,
            parameter_types: &self.parameter_types,
        }
    }
}

impl<'a> DeclarationForwardingView<'a> {
    pub(crate) fn to_owned(self) -> OwnedDeclarationForwarding {
        OwnedDeclarationForwarding {
            owner_parameters: self.owner_parameters.to_vec(),
            callable_parameters: self.callable_parameters.to_vec(),
            parameter_types: self.parameter_types.to_vec(),
        }
    }

    pub(crate) fn parameter_groups(
        owner_parameters: &'a [hir::TypeParamDecl],
        callable_parameters: &'a [hir::TypeParamDecl],
        parameter_types: &'a [hir::TypeId],
    ) -> Self {
        Self {
            owner_parameters,
            callable_parameters,
            parameter_types,
        }
    }

    pub(crate) fn nominal_parameters(
        owner_parameters: &'a [hir::TypeParamDecl],
        parameter_types: &'a [hir::TypeId],
    ) -> Self {
        Self {
            owner_parameters,
            callable_parameters: &[],
            parameter_types,
        }
    }
}

impl CallableView {
    /// The mapped declaration parameters include an extension receiver, but
    /// never an ordinary instance receiver or inferred call-site arguments.
    pub(crate) fn forwarding<'a>(
        &'a self,
        parameter_types: &'a [hir::TypeId],
    ) -> DeclarationForwardingView<'a> {
        DeclarationForwardingView {
            owner_parameters: &self.signature.owner_parameters,
            callable_parameters: &self.signature.callable_parameters,
            parameter_types,
        }
    }
}

impl NominalConstructorView {
    pub(crate) fn forwarding<'a>(
        &'a self,
        parameter_types: &'a [hir::TypeId],
    ) -> DeclarationForwardingView<'a> {
        DeclarationForwardingView {
            owner_parameters: &self.signature.owner_parameters,
            callable_parameters: &[],
            parameter_types,
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
