//! Non-callable name selection. Selection is independent of expected types
//! and does not materialize getters, singleton receivers or enum applications.

mod diagnostics;
mod materialization;
mod model;
mod selection;

pub(crate) use model::{NamedPropertyReceiver, NonValueTarget, ValueOrigin, ValueTarget};
