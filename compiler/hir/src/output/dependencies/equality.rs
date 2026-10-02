//! Generated dependency calls are selected from actual executable expressions.

use super::*;

impl DependencyHirOutput {
    pub fn executable_derived_equalities(
        &self,
    ) -> Result<Vec<crate::ImportedDerivedEquality>, DependencyCallOccurrenceError> {
        let mut calls = std::collections::BTreeSet::new();
        self.output
            .local
            .module()
            .visit_executable_expressions(|occurrence| {
                if let concrete::ExprKind::Call {
                    callee: concrete::CallableTarget::DerivedEquality(target),
                    ..
                } = occurrence.expression.kind
                {
                    calls.insert(self.output.local.module().imported_derived_equalities[target].0);
                }
                Ok::<_, DependencyCallOccurrenceError>(())
            })
            .map_err(|error| match error {
                concrete::ExecutableExpressionVisitError::Structure(error) => {
                    DependencyCallOccurrenceError::Structure(error)
                }
                concrete::ExecutableExpressionVisitError::Visitor(error) => error,
            })?;
        Ok(calls.into_iter().collect())
    }
}
