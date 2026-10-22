//! Cancellation stays separate from source diagnostics at shared phase boundaries.

use std::convert::Infallible;

use scoop_ast::Diagnostic;

#[derive(Debug)]
pub(crate) enum PhaseFailure<C> {
    Diagnostics(Vec<Diagnostic>),
    Cancelled(C),
}

impl<C> PhaseFailure<C> {
    pub(super) fn check(check: &impl Fn() -> Result<(), C>) -> Result<(), Self> {
        check().map_err(Self::Cancelled)
    }
}

impl<C> From<Vec<Diagnostic>> for PhaseFailure<C> {
    fn from(diagnostics: Vec<Diagnostic>) -> Self {
        Self::Diagnostics(diagnostics)
    }
}

impl PhaseFailure<Infallible> {
    pub(super) fn into_diagnostics(self) -> Vec<Diagnostic> {
        match self {
            Self::Diagnostics(diagnostics) => diagnostics,
            Self::Cancelled(never) => match never {},
        }
    }
}

pub(crate) fn uninterrupted() -> Result<(), Infallible> {
    Ok(())
}
