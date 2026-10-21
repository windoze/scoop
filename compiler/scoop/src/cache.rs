//! Versioned single-Cone compile cache contracts.

mod artifact;
mod key;
mod manifest;
mod record;
mod single_file;
mod store;

pub use key::*;
pub use record::*;
pub(crate) use store::open_build_lock;
pub use store::*;

pub(crate) const COMPILE_CACHE_NAMESPACE: &str = "compile-v2";
