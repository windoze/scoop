//! LLVM backend crate shell.
//!
//! During P9-T03 the implementation sources are owned under this crate path,
//! while the current monolithic pipeline still compiles them through the
//! `scoopc` facade to avoid a temporary Cargo dependency cycle. P9-T06 switches
//! the backend to direct `scoopc_lir` inputs and removes this facade dependency.

#[cfg(feature = "llvm")]
pub use scoopc::llvm::*;

pub mod stackmap {
    pub use scoopc::stackmap::*;
}
