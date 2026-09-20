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

mod source_parameter_contracts;
pub use source_parameter_contracts::SourceParameterContractError;
mod nominal_dispatch_binding;
pub use nominal_dispatch_binding::*;
mod protected_declaration_binding;
pub use protected_declaration_binding::*;
mod nominal_nested_binding;
pub use nominal_nested_binding::*;
mod nominal_parameter_binding;
pub use nominal_parameter_binding::*;

mod parameter_binding;
pub use parameter_binding::*;

mod nominal_constructor_binding;
pub use nominal_constructor_binding::*;

mod nominal_member_binding;
pub use nominal_member_binding::*;

mod nominal_binding;
pub use nominal_binding::*;

mod inheritance_binding;
pub use inheritance_binding::*;

mod declarations;
pub use declarations::*;

mod default_access;
pub use default_access::*;

mod default_references;
pub use default_references::*;

mod default_template;
pub use default_template::*;
