//! Structural verifier for LIR fact products.

use std::error::Error;
use std::fmt;

use crate::LirFacts;

/// Result type returned by LIR fact verification.
pub type Result<T> = std::result::Result<T, VerifyError>;

/// Structural errors detected before LIR facts are handed to later stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    CallableCountMismatch { expected: usize, actual: usize },
    EmptyCallableRoot { key: String },
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CallableCountMismatch { expected, actual } => write!(
                f,
                "LIR summary reports {expected} callables but inventory contains {actual}"
            ),
            Self::EmptyCallableRoot { key } => {
                write!(f, "LIR callable `{key}` has an empty root FQN")
            }
        }
    }
}

impl Error for VerifyError {}

/// Verify facts that are already grouped by the LIR stage.
pub fn verify_lir_facts(facts: &LirFacts) -> Result<()> {
    verify_summary_counts(facts)?;
    verify_callable_inventory(facts)?;
    Ok(())
}

fn verify_summary_counts(facts: &LirFacts) -> Result<()> {
    let actual = facts.callables.len();
    if facts.summary.callable_count != actual {
        return Err(VerifyError::CallableCountMismatch {
            expected: facts.summary.callable_count,
            actual,
        });
    }
    Ok(())
}

fn verify_callable_inventory(facts: &LirFacts) -> Result<()> {
    for (key, callable) in &facts.callables {
        if callable.root_fqn().is_empty() {
            return Err(VerifyError::EmptyCallableRoot {
                key: key.as_str().to_string(),
            });
        }
    }
    Ok(())
}
