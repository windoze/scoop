//! Versioned single-Cone compile cache contracts.

mod key;
mod receipt;
mod store;
mod validation;

pub use key::*;
pub use receipt::*;
pub use store::*;
pub use validation::*;

pub(crate) const COMPILE_CACHE_NAMESPACE: &str = "compile-v1";
