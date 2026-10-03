//! Shared closed registry for Scoop target and host toolchain profiles.
//!
//! This crate owns target resolution for both the orchestration parent and
//! the single-Cone compiler driver. Compiler stages receive only the typed
//! projections they consume and do not depend on this registry.

use std::fmt;

mod c_bridge;
mod compiler;
mod final_link;
mod registry;
mod system_provider;
mod trusted_core;

pub use final_link::ValidatedFinalLinkProfile;
pub use registry::{ResolvedTargetProfile, ValidatedRuntimeBuildProfile};
pub use system_provider::{
    LIBSYSTEM_INSTALL_NAME, SystemExportKind, SystemProvider, SystemStubFile,
};
pub use trusted_core::TrustedCoreSlotLayoutV1;

#[derive(Debug)]
pub struct ToolchainError(pub String);

impl fmt::Display for ToolchainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ToolchainError {}
pub use compiler::paired_compiler_machine_capability;
