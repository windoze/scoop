//! Versioned single-Cone compile cache contracts.

mod core_receipt;
mod key;
mod receipt;
mod store;
mod validation;

pub use core_receipt::*;
pub use key::*;
pub use receipt::*;
pub use store::*;
pub use validation::*;

pub(crate) const COMPILE_CACHE_NAMESPACE: &str = "compile-v1";
