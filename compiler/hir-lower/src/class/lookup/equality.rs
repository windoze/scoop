use super::*;

impl Lowerer {
    pub(crate) fn equality_method_candidates(
        &mut self,
        receiver: TypeId,
    ) -> Vec<crate::CallableCandidate> {
        let parameters = self.equality_parameter_types(receiver);
        self.methods_by_operator(receiver, hir::OperatorKind::Equals)
            .into_iter()
            .filter(|candidate| {
                if self.functions[candidate.function].method_type_param_count() != 0 {
                    return false;
                }
                let arguments = match &candidate.owner {
                    crate::CallableCandidateOwner::Method(owner) => {
                        self.method_owner_arguments(*owner).to_vec()
                    }
                    crate::CallableCandidateOwner::Function { owner_arguments } => {
                        owner_arguments.clone()
                    }
                };
                let signature = self.instantiated_signature(candidate.function, &arguments, &[]);
                matches!(signature.params.as_slice(), [other]
                    if parameters.iter().any(|parameter| self.types_equal(other.ty, *parameter)))
            })
            .collect()
    }
}
