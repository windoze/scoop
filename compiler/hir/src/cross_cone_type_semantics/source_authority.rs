//! Independent source-side transcripts for replaying type semantics.
//! Identity resolution alone never grants semantic or lookup authority.

mod inventory;
pub use inventory::*;
mod foundation;
pub use foundation::*;

mod binding_keys;
mod dispatch_binding;
pub use dispatch_binding::*;
