use super::*;

impl Lowerer {
    pub(crate) fn equality_method_candidates(
        &mut self,
        receiver: TypeId,
    ) -> Vec<crate::CallableCandidate> {
        self.methods_by_operator(receiver, hir::OperatorKind::Equals)
            .into_iter()
            .filter(|candidate| self.functions[candidate.function].method_type_param_count() == 0)
            .collect()
    }
}
