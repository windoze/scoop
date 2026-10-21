use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Shared only by an analysis revision or by one outstanding request.
#[derive(Debug, Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cancelled;

impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn check(&self) -> Result<(), Cancelled> {
        if self.0.load(Ordering::Relaxed) {
            Err(Cancelled)
        } else {
            Ok(())
        }
    }
}
