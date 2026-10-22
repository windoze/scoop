use super::*;

impl Lowerer {
    pub(crate) fn with_recovered_local_function<R>(
        &mut self,
        header: ast::FunctionHeaderRef<'_>,
        query: impl FnOnce(&mut Self, hir::FunctionId) -> R,
    ) -> Option<R> {
        let outer_type_params = self.type_params_in_scope.clone();
        let before = self.diagnostics.len();
        let result = (|| {
            let local = self.prepare_local_function(header)?;
            if self.diagnostics.len() != before {
                return None;
            }
            let function = self.local_functions[local].source_function();
            self.with_local_function_scope(local, |state, _| {
                state.bind_local_parameters(function);
                state.lower_context_entry(function);
                if state.diagnostics.len() != before {
                    return None;
                }
                Some(query(state, function))
            })
        })();
        self.type_params_in_scope = outer_type_params;
        result
    }
}
