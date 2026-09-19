//! Protected source declarations retain ordinary source signature constituents
//! without acquiring the public lookup interface's authority.

use super::wire;

mod decode;
mod errors;
mod nested;
mod payload;
mod property;
mod records;
mod semantics;
mod slot_refs;
mod source_callable;
mod source_use;
mod support_callables;
mod support_properties;
mod support_source_use;
#[cfg(test)]
pub(in crate::cross_cone_type_semantics) mod tests;

pub use decode::*;
pub use errors::*;
pub use nested::*;
pub use payload::*;
pub use property::*;
pub use records::*;
pub use semantics::*;
pub use slot_refs::*;
pub use source_callable::*;
pub use source_use::*;
pub use support_callables::*;
pub use support_properties::*;
pub use support_source_use::*;
