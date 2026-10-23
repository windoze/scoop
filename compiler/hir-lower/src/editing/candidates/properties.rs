use super::*;
use crate::expr::SuccessfulExprLayer;
use crate::properties::ExtensionPropertyCandidateOutcome;

pub(super) struct RankedValue {
    pub(super) view: SuccessfulExprLayer,
    pub(super) rank: usize,
}

impl Lowerer {
    pub(super) fn editing_member_property(
        &self,
        name: &ast::Ident,
        receiver: &hir::Expr,
    ) -> Option<SuccessfulExprLayer> {
        self.probe_expr_layer(|state, _| state.member_property_read(receiver.clone(), name))
            .ok()
    }

    pub(super) fn editing_extension_property_value(
        &self,
        name: &ast::Ident,
        receiver: &hir::Expr,
        rank: usize,
    ) -> Result<Option<RankedValue>, U> {
        let mut layers = self.named_extension_property_layers(&name.text);
        Self::retain_highest_rank_origins(&mut layers);
        let layer = layers
            .into_iter()
            .find(|layer| layer.kind.call_rank() == rank);
        let Some(layer) = layer else {
            return Ok(None);
        };
        if !layer.unavailable.is_empty() {
            return Err(U::SignatureFailure);
        }
        if layer.candidates.is_empty() {
            return Ok(None);
        }
        self.editing_extension_property_read(receiver.clone(), name, &layer.candidates)
            .map(|view| view.map(|view| RankedValue { view, rank }))
    }

    pub(super) fn editing_extension_property_read(
        &self,
        receiver: hir::Expr,
        name: &ast::Ident,
        candidates: &[crate::imports::lookup::calls::ExtensionPropertyTarget],
    ) -> Result<Option<SuccessfulExprLayer>, U> {
        let mut state = self.clone();
        let mut sink = Vec::new();
        match state
            .resolve_extension_property_candidates_outcome(receiver, name, candidates, &mut sink)
        {
            ExtensionPropertyCandidateOutcome::Resolved(property) => {
                Ok(Some(SuccessfulExprLayer {
                    state: Box::new(state),
                    expression: property.read,
                    sink,
                }))
            }
            ExtensionPropertyCandidateOutcome::NoApplicable
            | ExtensionPropertyCandidateOutcome::NoCandidate => Ok(None),
            ExtensionPropertyCandidateOutcome::Failed => Err(U::BodyFailure),
        }
    }
}
