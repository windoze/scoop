//! Shared closed registry for Scoop target and host toolchain profiles.
//!
//! This crate owns target resolution for both the orchestration parent and
//! the single-Cone compiler driver. Compiler stages receive only the typed
//! projections they consume and do not depend on this registry.

use std::fmt;

mod compiler_inputs;
pub use compiler_inputs::{CompileTimeInputs, environment_inputs_current};

pub const VERSION: &str = env!("SCOOP_BUILD_VERSION");
pub const DARWIN_DEPLOYMENT_BASELINE: &str = env!("SCOOP_MACOS_BASELINE");

mod c_bridge;
mod c_bridge_invocation;
pub use c_bridge_invocation::{
    CBridgeToolchainInvocationError, ValidatedCBridgeToolchainInvocation,
};
mod compiler;
mod cxx;
mod final_link;
mod generators;
pub use generators::{GeneratorPrograms, record_generator_input};
mod linux_c;
mod linux_target;
mod manifest;
pub use manifest::{SOURCE_TARGETS, load_cone_manifest, parse_cone_manifest};
mod build_record;
mod native;
mod native_record;
mod object_file;
mod paths;
pub use object_file::{object_path, store_object};
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
    NativeSourceInput, NativeToolchain, PreparedNativeInputs, PreparedNativeObjects,
    compile_native_source, prepare_native_inputs, prepare_native_objects,
    strip_native_include_observations,
};
pub use paths::{
    artifact_candidate_path, configured_cache_root, configured_sysroot_root, default_runtime_index,
    development_runtime_root, development_workspace_root, installed_executable,
    sysroot_library_path, target_library_root,
};
pub use registry::host_target_triple;
pub use registry::{CToolchainOptions, ResolvedTargetProfile, select_target};
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
pub use native_record::inputs::{Input as NativeInputMetadata, native_include_search_paths};

pub use native_record::{cached_native_tool_inputs, native_tool_configuration};
