use super::*;
use inference::CandidateInput;

impl Lowerer {
    pub(super) fn editing_source_call_candidate(
        &self,
        call: &ast::RecoveredCall,
        candidate: &crate::CallableCandidate,
        receiver: Option<hir::TypeId>,
    ) -> Candidate {
        let function = candidate.function;
        if self.declaration_surface.rejects_function(function) {
            return Ok(None);
        }
        if self
            .declaration_surface
            .unavailable_signature(function)
            .is_some()
        {
            return Err(U::SignatureFailure);
        }
        let signature = &self.signatures[&function];
        if signature.parameter_pending {
            return Err(U::PendingInference);
        }
        let extension = self.extension_receivers.get(&function).copied();
        let receiver = match (extension, receiver) {
            (Some(declared), Some(actual)) => Some((declared, actual)),
            (Some(_), None) => return Ok(None),
            (None, _) => None,
        };
        let view = self.callable_view(candidate, extension.is_some());
        let owner_arguments = self.callable_candidate_owner_arguments(candidate);
        self.editing_signature_candidate(
            call,
            CandidateInput {
                signature: view.signature,
                type_arguments:
                    crate::call_resolution::applicability::DeclarationTypeArguments::Callable {
                        owner_arguments: &owner_arguments,
                    },
                receiver,
                argument_mode: ArgumentMode::Mixed,
                result: if signature.result_pending {
                    V::Unavailable(U::PendingInference)
                } else {
                    V::Known(signature.return_ty)
                },
                named_parameters: true,
                explicit_arguments: None,
            },
        )
    }
}
