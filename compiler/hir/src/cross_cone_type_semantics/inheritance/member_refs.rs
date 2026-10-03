//! Protected member identities used by inheritance and MIR callable selection.
use crate::SourceNominalId;
use crate::cross_cone_type_semantics::wire;

mod errors;
mod references;
mod table;
pub use errors::*;
pub use references::*;
pub use table::*;
