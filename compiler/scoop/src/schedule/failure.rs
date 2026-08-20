use std::fmt;

use scoop_protocol::StructuredDiagnosticV1;

use super::BuildGraphExecutionError;
use crate::{
    BuildFailureClassification, ClassifyBuildFailure, CompletedNode, OrdinarySourceExecutionError,
};

/// A stopped graph retains warnings already produced by completed dependencies.
#[derive(Debug)]
pub struct BuildGraphExecutionFailure {
    cause: BuildGraphExecutionError,
    warnings: Vec<StructuredDiagnosticV1>,
}

impl BuildGraphExecutionFailure {
    pub(super) fn new(cause: BuildGraphExecutionError, completed: &[&CompletedNode]) -> Self {
        let mut warnings = completed
            .iter()
            .flat_map(|node| node.warnings().iter().cloned())
            .collect::<Vec<_>>();
        if let BuildGraphExecutionError::Ordinary(_, source) = &cause
            && let OrdinarySourceExecutionError::ProducedOutput {
                warnings: current, ..
            } = source.as_ref()
        {
            warnings.extend(current.iter().cloned());
        }
        Self { cause, warnings }
    }

    pub const fn cause(&self) -> &BuildGraphExecutionError {
        &self.cause
    }

    pub fn warnings(&self) -> &[StructuredDiagnosticV1] {
        &self.warnings
    }

    pub fn child_diagnostics(&self) -> Option<&[StructuredDiagnosticV1]> {
        self.cause.child_diagnostics()
    }
}

impl ClassifyBuildFailure for BuildGraphExecutionFailure {
    fn classification(&self) -> BuildFailureClassification {
        self.cause.classification()
    }
}

impl fmt::Display for BuildGraphExecutionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(formatter)
    }
}

impl std::error::Error for BuildGraphExecutionFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}
