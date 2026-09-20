//! Independent source-side transcripts for replaying type semantics.
//! Identity resolution alone never grants semantic or lookup authority.

mod inventory;
pub use inventory::*;
mod foundation;
pub use foundation::*;

mod binding_keys;
mod dispatch_binding;
pub use dispatch_binding::*;
mod slot_binding;
pub use slot_binding::*;
mod constructor_binding;
pub use constructor_binding::*;

mod property_binding;
pub use property_binding::*;

mod protected_binding;
pub use protected_binding::*;

mod parameter_binding;
pub use parameter_binding::*;

mod nominal_binding;
pub use nominal_binding::*;
