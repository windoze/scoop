//! CFG call order and deferred coroutine registrations.

use scoop_mir as mir;

use super::CleanupContext;
use crate::coroutine_registry::ProtocolMethod;

#[derive(Clone)]
pub(crate) struct CallSite {
    pub(crate) block: mir::BlockId,
    /// Position in the ordinary CFG statement list. Registrations precede
    /// the statement at this position and may share the same insertion point.
    pub(crate) statement: usize,
    pub(crate) pending: CleanupContext,
    pub(crate) kind: CallSiteKind,
}

#[derive(Clone)]
pub(crate) enum CallSiteKind {
    Call,
    Suspend(SuspendCall),
}

#[derive(Clone)]
pub(crate) struct SuspendCall {
    pub(crate) method: ProtocolMethod,
    pub(crate) receiver: Box<mir::Expr>,
    /// Unit has no destination; every other result has its typed CFG local.
    pub(crate) destination: Option<mir::LocalId>,
}
