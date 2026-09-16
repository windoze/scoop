//! Versioned single-Cone compile cache contracts.

mod key;
mod receipt;
mod store;

pub use key::*;
pub use receipt::*;
pub use store::*;

pub(crate) const COMPILE_CACHE_NAMESPACE: &str = "compile-v1";
