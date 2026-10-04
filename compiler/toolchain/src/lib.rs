//! Shared closed registry for Scoop target and host toolchain profiles.
//!
//! This crate owns target resolution for both the orchestration parent and
//! the single-Cone compiler driver. Compiler stages receive only the typed
//! projections they consume and do not depend on this registry.

use std::fmt;

mod c_bridge;
mod compiler;
mod final_link;
mod linux_c;
mod paths;
mod registry;
mod system_provider;
mod trusted_core;

pub use final_link::ValidatedFinalLinkProfile;
pub use linux_c::resolve_linux_c_toolchain;
pub use paths::{configured_sysroot_root, development_runtime_root, development_workspace_root};
pub use registry::host_target_triple;
pub use registry::{ResolvedTargetProfile, ValidatedRuntimeBuildProfile};
pub use system_provider::{
    LIBSYSTEM_INSTALL_NAME, NativeExport, SystemExportKind, SystemProvider, SystemStubFile,
    TextStubInterface, read_text_stubs, write_link_stub,
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
