//! Shared closed registry for Scoop target and host toolchain profiles.
//!
//! This crate owns target resolution for both the orchestration parent and
//! the single-Cone compiler driver. Compiler stages receive only the typed
//! projections they consume and do not depend on this registry.

use std::fmt;

mod c_bridge;
mod compiler;
mod cxx;
mod final_link;
mod linux_c;
mod native;
mod paths;
mod registry;
mod request;
mod runtime;
mod system_provider;
mod trusted_core;
mod unwind;

pub use cxx::ValidatedCxxToolchain;
pub use final_link::{
    FinalLinkOptions, LinkMode, LinuxFinalLinkProfile, ValidatedFinalLinkProfile,
};
pub use linux_c::resolve_linux_c_toolchain;
pub use native::{
    NativeSourceInput, NativeToolchain, PreparedNativeInputs, compile_native_source,
    prepare_native_inputs,
};
pub use paths::{configured_sysroot_root, development_runtime_root, development_workspace_root};
pub use registry::host_target_triple;
pub use registry::{CToolchainOptions, ResolvedTargetProfile};
pub use runtime::ValidatedRuntimeBuildProfile;
pub use system_provider::{
    LIBSYSTEM_INSTALL_NAME, NativeExport, PreviousExport, SystemExportKind, SystemProvider,
    SystemStubFile, TextStubInterface, read_text_stubs, write_link_stub,
};
pub use trusted_core::TrustedCoreSlotLayoutV1;
pub use unwind::{runtime_unwind_include, selected_unwind_prefix};

#[derive(Debug)]
pub struct ToolchainError(pub String);

impl fmt::Display for ToolchainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ToolchainError {}
pub use compiler::paired_compiler_machine_capability;
