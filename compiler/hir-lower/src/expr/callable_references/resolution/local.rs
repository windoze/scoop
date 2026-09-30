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
            &self.view.owner_parameters,
            &self.view.callable_parameters,
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
        if view.owner_parameters.len() != owner_type_args.len() {
            return Err(fail(
                &state,
                "receiver does not provide complete owner type arguments".into(),
            ));
        }
        let own_type_param_count = view.callable_parameters.len();
        if context.expected.is_none() && own_type_param_count != 0 {
            return Err(fail(
                &state,
                "generic callable references require an expected function type".into(),
            ));
        }
        if view.effects.attributes.safety == hir::Safety::Unsafe {
            return Err(fail(&state, "unsafe functions cannot be stored in a managed function type because safety is not part of function-type identity".into()));
        }
        let mut reference_params = view
            .value_parameters
            .iter()
            .map(|parameter| parameter.ty)
            .collect::<Vec<_>>();
        let mut forwarding_parameter_types = reference_params.clone();
        if let Some(receiver) = extension_receiver {
            forwarding_parameter_types.insert(0, receiver);
            if matches!(
                context.extension_mode,
                ReferenceExtensionMode::IncludeUnbound
            ) {
                reference_params.insert(0, receiver);
            }
        }
        let expected_type = context.expected.map_or_else(
            || {
                let parameters = reference_params
                    .iter()
                    .map(|&parameter| state.instantiate_ty(parameter, &owner_type_args))
                    .collect();
                let return_type = state.instantiate_ty(view.return_type, &owner_type_args);
                state.intern_function_type(view.effects.is_suspend, parameters, return_type)
            },
            |(ty, _)| *ty,
        );
        let type_args = state
            .solve_callable_reference_applicability(CallableReferenceApplicabilityInput {
                owner_parameters: &view.owner_parameters,
                callable_parameters: &view.callable_parameters,
                owner_arguments: &owner_type_args,
                bound_receiver,
                parameter_types: &reference_params,
                return_type: view.return_type,
                is_suspend: view.effects.is_suspend,
                expected_type,
            })
            .map_err(|constraint| {
                fail(
                    &state,
                    render_callable_constraint_failure(&state, &view, None, &[], &constraint),
                )
            })?;
        Ok(Some(ApplicableReference {
            state: Box::new(state),
            declaration: ReferenceDeclaration::Local(LocalReferenceDeclaration {
                candidate: candidate.clone(),
                view,
                forwarding_parameter_types,
            }),
            type_args,
            ty: expected_type,
            own_type_param_count,
        }))
    }
}
