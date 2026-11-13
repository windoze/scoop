//! Shared closed registry for Scoop target and host toolchain profiles.
//!
//! This crate owns target resolution for both the orchestration parent and
//! the single-Cone compiler driver. Compiler stages receive only the typed
//! projections they consume and do not depend on this registry.

use std::fmt;

mod compiler_inputs;
pub use compiler_inputs::{CompileTimeInputs, build_file_input, compiler_inputs_current};

pub const VERSION: &str = env!("SCOOP_BUILD_VERSION");
include!(concat!(env!("OUT_DIR"), "/deployment.rs"));

mod c_bridge;
mod c_bridge_invocation;
pub use c_bridge_invocation::{
    CBridgeToolchainInvocationError, ValidatedCBridgeToolchainInvocation,
};
mod compiler;
mod debug;
pub use debug::{DebugFileKind, NativeDebugTools};
mod cxx;
mod final_link;
mod generators;
pub use generators::{GeneratorPrograms, record_generator_input};
mod linux_c;
mod linux_target;
mod windows_c;
pub use windows_c::resolve_windows_c_toolchain;
mod manifest;
mod windows_machine;
pub use manifest::{SOURCE_CATALOG_KEY, SOURCE_TARGETS, load_cone_manifest, parse_cone_manifest};
mod build_record;
mod native;
mod native_driver;
mod native_invocation;
mod paths;
pub use paths::system_library_root;
mod registry;
mod request;
mod runtime;
mod system_provider;
mod trusted_core;
mod unwind;
mod windows_native;
mod windows_resource;

pub use cxx::ValidatedCxxToolchain;
pub use final_link::{
    FinalLinkOptions, FinalOutputKind, LinkMode, LinuxFinalLinkProfile, ValidatedFinalLinkProfile,
    WindowsFinalLinkProfile,
};
pub use linux_c::resolve_linux_c_toolchain;
pub use native::{
    NativeSourceInput, NativeToolchain, PreparedNativeInputs, PreparedNativeObjects,
    compile_native_source, prepare_native_inputs, prepare_native_objects,
    selected_native_object_sources, strip_native_include_observations,
};
pub use paths::{
    artifact_candidate_path, configured_cache_root, configured_sysroot_root,
    default_profiling_runtime_index, default_runtime_index, development_runtime_root,
    development_workspace_root, host_executable_name, installed_executable, installed_host_tool,
    profile_map_path, profiling_runtime_archive, shared_commands_root, sysroot_library_path,
    target_library_root, versioned_cache_root,
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

#[derive(Clone, Debug)]
pub struct ToolchainError(pub String);

impl fmt::Display for ToolchainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ToolchainError {}
pub use build_record::inputs::{Input as NativeInputMetadata, native_include_search_paths};
pub use compiler::paired_compiler_machine_capability;
