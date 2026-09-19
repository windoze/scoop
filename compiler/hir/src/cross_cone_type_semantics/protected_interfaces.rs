//! Protected source declarations retain ordinary source signature constituents
//! without acquiring the public lookup interface's authority.

use super::wire;

mod decode;
mod errors;
mod payload;
mod property;
mod records;
mod semantics;
mod slot_refs;
mod source_use;
#[cfg(test)]
mod tests;

pub use decode::*;
pub use errors::*;
pub use payload::*;
pub use property::*;
pub use records::*;
pub use semantics::*;
pub use slot_refs::*;
pub use source_use::*;
