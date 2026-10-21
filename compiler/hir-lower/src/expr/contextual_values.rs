//! Reuse independent expression and branch synthesis within one inference pass.

use scoop_ast::{Diagnostic, DiagnosticSeverity};
use scoop_hir::TypeId;

use crate::Lowerer;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExpressionContextRequirement {
    Independent,
    Required,
    Unresolved,
}

impl ExpressionContextRequirement {
    pub(super) fn from_required(required: bool) -> Self {
        if required {
            Self::Required
        } else {
            Self::Independent
        }
    }

    pub(super) fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::Required, _) | (_, Self::Required) => Self::Required,
            (Self::Unresolved, _) | (_, Self::Unresolved) => Self::Unresolved,
            _ => Self::Independent,
        }
    }
}

impl Lowerer {
    /// A complete context types the value directly. Otherwise syntax either
    /// postpones it or its successful independent synthesis is committed once.
    pub(crate) fn probe_contextual_value<T>(
        &mut self,
        requirement: ExpressionContextRequirement,
        expected: Option<TypeId>,
        failure: &mut Option<Vec<Diagnostic>>,
        lower: impl FnOnce(&mut Self, Option<TypeId>) -> Option<T>,
    ) -> Option<Option<T>> {
        if expected.is_some() || requirement == ExpressionContextRequirement::Independent {
            return lower(self, expected).map(Some);
        }
        if requirement == ExpressionContextRequirement::Required {
            return Some(None);
        }
        let mut attempt = Box::new(self.clone());
        let baseline = self.diagnostics.len();
        if let Some(value) = lower(&mut attempt, None)
            && attempt.diagnostics[baseline..]
                .iter()
                .all(|diagnostic| diagnostic.severity != DiagnosticSeverity::Error)
        {
            std::mem::swap(self, &mut attempt);
            Some(Some(value))
        } else {
            *failure = Some(attempt.diagnostics.split_off(baseline));
            Some(None)
        }
    }

    /// An unchanged failed synthesis is not repeated when there is no new
    /// context. With a hint, the ordinary lowering path owns diagnostics.
    pub(crate) fn finish_contextual_value<T>(
        &mut self,
        expected: Option<TypeId>,
        failure: &mut Option<Vec<Diagnostic>>,
        lower: impl FnOnce(&mut Self, Option<TypeId>) -> Option<T>,
    ) -> Option<T> {
        if let Some(failed) = failure.take()
            && expected.is_none()
        {
            self.diagnostics.extend(failed);
            return None;
        }
        lower(self, expected)
    }
}
