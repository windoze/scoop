use super::*;
use crate::call_resolution::candidates::{ValueParameter, ValueParameterCalling};

impl Lowerer {
    pub(super) fn editing_direct_value_candidates(
        &mut self,
        call: &ast::RecoveredCall,
        value: &hir::Expr,
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<CallCandidates, EditingCancelled> {
        if matches!(
            self.types[value.ty],
            hir::Type::Function(_) | hir::Type::FunPtr(_)
        ) {
            return self.editing_value_call_candidates(call, value.clone(), is_cancelled);
        }
        let mut result = Candidates::default();
        for receiver in self.smart_cast_receiver_views(value) {
            let candidates = self.editing_direct_member_candidates(
                call,
                receiver.ty,
                "invoke",
                true,
                false,
                is_cancelled,
            )?;
            result.values.extend(candidates.values);
            result.unavailable = result.unavailable.or(candidates.unavailable);
        }
        Ok(result.finish())
    }

    pub(super) fn editing_value_call_candidates(
        &mut self,
        call: &ast::RecoveredCall,
        value: hir::Expr,
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<CallCandidates, EditingCancelled> {
        if matches!(self.types[value.ty], hir::Type::FunPtr(_)) {
            return Ok(V::Known(Vec::new()));
        }
        if let hir::Type::Function(function) = self.types[value.ty] {
            let mut candidates = Candidates::default();
            candidates.add(self.editing_function_value_candidate(call, function, None));
            return Ok(candidates.finish());
        }
        self.editing_member_call_candidates(call, value, "invoke", true, is_cancelled)
    }

    pub(super) fn editing_receiver_function_candidates(
        &self,
        call: &ast::RecoveredCall,
        receiver: &hir::Expr,
    ) -> CallCandidates {
        let ast::RecoveredCallTarget::Member { name, .. } = &call.target else {
            return V::Known(Vec::new());
        };
        let mut state = self.clone();
        let Some(value) = state.lower_var(name, &mut Vec::new(), None) else {
            return V::Known(Vec::new());
        };
        let hir::Type::Function(function) = state.types[value.ty] else {
            return V::Known(Vec::new());
        };
        if state.function_types[function].receiver.is_none() {
            return V::Known(Vec::new());
        }
        let mut candidates = Candidates::default();
        candidates.add(state.editing_function_value_candidate(call, function, Some(receiver.ty)));
        candidates.finish()
    }

    fn editing_function_value_candidate(
        &self,
        call: &ast::RecoveredCall,
        function: hir::FunctionTypeId,
        receiver: Option<hir::TypeId>,
    ) -> Candidate {
        let function = &self.function_types[function];
        let signature = DeclarationSignature::<()> {
            owner_parameters: Vec::new(),
            callable_parameters: Vec::new(),
            value_parameters: function
                .parameters()
                .skip(usize::from(receiver.is_some()))
                .map(|&ty| ValueParameter {
                    // Positional-only function types have no declaration names.
                    name: String::new(),
                    calling: ValueParameterCalling::Required,
                    ty,
                })
                .collect(),
            return_type: function.return_type,
        };
        let result = V::Known(signature.return_type);
        self.editing_signature_candidate(
            call,
            inference::CandidateInput {
                signature,
                type_arguments:
                    crate::call_resolution::applicability::DeclarationTypeArguments::Callable {
                        owner_arguments: &[],
                    },
                receiver: function.receiver.zip(receiver),
                argument_mode: ArgumentMode::PositionalOnly,
                result,
                named_parameters: false,
                explicit_arguments: None,
            },
        )
    }
}
