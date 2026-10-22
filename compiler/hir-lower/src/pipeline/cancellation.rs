//! Cancellation stays separate from source diagnostics at shared phase boundaries.

use std::{cell::Cell, convert::Infallible, rc::Rc};

use scoop_ast::Diagnostic;

#[derive(Debug)]
pub(crate) enum PhaseFailure<C> {
    Diagnostics(Vec<Diagnostic>),
    Cancelled(C),
}

impl<C> PhaseFailure<C> {
    pub(crate) fn check(check: &impl Fn() -> Result<(), C>) -> Result<(), Self> {
        check().map_err(Self::Cancelled)
    }
}

impl<C> From<Vec<Diagnostic>> for PhaseFailure<C> {
    fn from(diagnostics: Vec<Diagnostic>) -> Self {
        Self::Diagnostics(diagnostics)
    }
}

impl PhaseFailure<Infallible> {
    pub(crate) fn into_diagnostics(self) -> Vec<Diagnostic> {
        match self {
            Self::Diagnostics(diagnostics) => diagnostics,
            Self::Cancelled(never) => match never {},
        }
    }
}

pub(crate) fn uninterrupted() -> Result<(), Infallible> {
    Ok(())
}

/// A cancelled query stays cancelled across lowerer clones and speculative rollback.
#[derive(Clone)]
pub(crate) struct FrontendCancellation {
    check: Rc<dyn Fn() -> bool>,
    cancelled: Rc<Cell<bool>>,
}

impl FrontendCancellation {
    pub(crate) fn new(check: impl Fn() -> bool + 'static) -> Self {
        Self {
            check: Rc::new(check),
            cancelled: Rc::new(Cell::new(false)),
        }
    }

    pub(crate) fn cancelled(&self) -> bool {
        if !self.cancelled.get() && (self.check)() {
            self.cancelled.set(true);
        }
        self.cancelled.get()
    }

    pub(crate) fn check(&self) -> Result<(), ()> {
        if self.cancelled() { Err(()) } else { Ok(()) }
    }
}
