use super::*;
use crate::call_resolution::applicability::CallableReferenceApplicabilityInput;
use crate::call_resolution::candidates::{CallableView, ReceiverShape};
use crate::call_resolution::diagnostics::{
    callable_layer_name, callable_source_signature, render_callable_constraint_failure,
};
use crate::call_resolution::specificity::DeclarationForwardingView;

pub(super) struct LocalReferenceDeclaration {
    pub candidate: crate::CallableCandidate,
    pub view: CallableView,
    forwarding_parameter_types: Vec<TypeId>,
}

impl LocalReferenceDeclaration {
    pub(super) fn forwarding(&self) -> OwnedDeclarationForwarding {
        DeclarationForwardingView::parameter_groups(
            &self.view.signature.owner_parameters,
            &self.view.signature.callable_parameters,
            &self.forwarding_parameter_types,
        )
        .to_owned()
    }

    pub(super) fn commit(
        self,
        state: &mut Lowerer,
        type_args: &[TypeId],
        receiver: Option<&hir::Expr>,
    ) -> hir::CallableReferenceTarget {
        let callee = state.materialize_candidate_callable(&self.candidate, type_args);
        match receiver {
            Some(receiver) if matches!(self.view.receiver, ReceiverShape::Extension(_)) => {
                hir::CallableReferenceTarget::BoundExtension {
                    receiver: Box::new(receiver.clone()),
                    callee: callee.into(),
                }
            }
            Some(receiver) => hir::CallableReferenceTarget::BoundMember {
                receiver: Box::new(receiver.clone()),
                callee: state.materialize_method_callee(self.candidate.source, callee, type_args),
            },
            None => hir::CallableReferenceTarget::Named(callee.into()),
        }
    }
}

impl Lowerer {
    pub(super) fn probe_local_reference(
        &self,
        candidate: &crate::CallableCandidate,
        context: ReferenceResolutionContext<'_>,
    ) -> Result<Option<ApplicableReference>, ReferenceFailure> {
        let mut state = self.clone();
        let function = candidate.function;
        let owner_type_args = state.callable_candidate_owner_arguments(candidate);
        let extension_receiver = state.extension_receivers.get(&function).copied();
        let bound_receiver = match context.extension_mode {
            ReferenceExtensionMode::Exclude if extension_receiver.is_some() => return Ok(None),
            ReferenceExtensionMode::Bound(_) if extension_receiver.is_none() => return Ok(None),
            ReferenceExtensionMode::Bound(receiver) => {
                extension_receiver.map(|declared| (declared, receiver))
            }
            ReferenceExtensionMode::Exclude | ReferenceExtensionMode::IncludeUnbound => None,
        };
        let view = state.callable_view(candidate, extension_receiver.is_some());
        let fail = |state: &Lowerer, reason: String| ReferenceFailure {
            signature: callable_source_signature(state, context.name, &view),
            layer: callable_layer_name(state, std::slice::from_ref(&view)),
            reason,
        };
        let own_type_param_count = view.signature.callable_parameters.len();
        let forwarding_parameter_types = extension_receiver
            .into_iter()
            .chain(
                view.signature
                    .value_parameters
                    .iter()
                    .map(|parameter| parameter.ty),
            )
            .collect();
        let application = state
            .solve_callable_reference_applicability(CallableReferenceApplicabilityInput {
                signature: &view.signature,
                owner_arguments: &owner_type_args,
                bound_receiver,
                unbound_receiver: matches!(
                    context.extension_mode,
                    ReferenceExtensionMode::IncludeUnbound
                )
                .then_some(extension_receiver)
                .flatten(),
                effects: view.effects,
                expected_type: context.expected.map(|(ty, _)| *ty),
            })
            .map_err(|failure| {
                fail(
                    &state,
                    failure.describe(|constraint| {
                        render_callable_constraint_failure(&state, &view, None, &[], constraint)
                    }),
                )
            })?;
        Ok(Some(ApplicableReference {
            state: Box::new(state),
            declaration: ReferenceDeclaration::Local(LocalReferenceDeclaration {
                candidate: candidate.clone(),
                view,
                forwarding_parameter_types,
            }),
            type_args: application.type_args,
            ty: application.ty,
            own_type_param_count,
        }))
    }
}
