//! Cleanup transfers needed only while constructing coroutine frames.

use scoop_mir as mir;

#[derive(Clone, Debug, Default)]
pub(crate) struct CleanupContext(pub(crate) Vec<CleanupTransfer>);

impl CleanupContext {
    pub(super) fn with_transfer(&self, transfer: CleanupTransfer) -> Self {
        let mut chain = self.0.clone();
        chain.push(transfer);
        Self(chain)
    }
}

#[derive(Clone, Debug)]
pub(crate) enum CleanupTransfer {
    Fallthrough(mir::CoroutineCleanupFallthroughTarget),
    ReturnUnit,
    ReturnValue(mir::LocalId),
    Break(mir::CoroutineLoopExitTarget),
    Continue(mir::CoroutineLoopHeaderTarget),
    ManagedThrow {
        exception: mir::LocalId,
        unwind: Option<mir::CoroutineUnwindTarget>,
    },
}
