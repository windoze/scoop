//! Stage-independent project, source-cone, and compilation-unit model.
//!
//! This base crate owns project membership, source-cone graph data, source
//! trust, dependency topology checks, and backend-neutral manifest settings. The
//! `scoopc` facade may provide filesystem/session adapters, but it must not
//! duplicate these definitions or make this crate depend on stage/fact/backend
//! crates.

#![forbid(unsafe_code)]

pub mod artifact_metadata;
pub mod graph;
pub mod graph_loader;
pub mod manifest;
pub mod manifest_loader;
pub mod opt;
pub mod package;
pub mod package_loader;
pub mod sysroot;

pub use artifact_metadata::{
    CONE_ARTIFACT_EFFECT_FACTS_FILE_NAME, CONE_ARTIFACT_FRONTEND_IMPORT_FILE_NAME,
    CONE_ARTIFACT_HIR_FACTS_FILE_NAME, CONE_ARTIFACT_INPUTS_FINGERPRINT_FILE_NAME,
    CONE_ARTIFACT_LIR_FACTS_FILE_NAME, CONE_ARTIFACT_LIR_PROGRAM_FILE_NAME,
    CONE_ARTIFACT_MANIFEST_FILE_NAME, CONE_ARTIFACT_MIR_FACTS_FILE_NAME,
    CONE_ARTIFACT_OBJS_DIR_NAME, CONE_ARTIFACT_OUTPUTS_FINGERPRINT_FILE_NAME,
    CONE_ARTIFACT_TYPE_STORE_FILE_NAME, ConeArtifactFingerprints, ConeArtifactManifest,
    ConeArtifactMetadataError, ConeArtifactSchemaVersions, compute_outputs_fingerprint,
    read_manifest_and_inputs_fingerprint, validate_object_file_name,
};
pub use graph::{
    CONSUMER_CONE_ID, ConeId, ConeInfo, SourceConeCompilationUnit, SourceConeDependencyEdge,
    SourceConeDependencyKind, SourceConeGraph, SourceConeInfo, SourceConeNode, SourceConeRole,
    SourceConeTrust, StableConeKey,
};
pub use graph_loader::{
    load_source_cone_graph_for_consumer_package, load_source_cone_graph_for_virtual_consumer,
};
pub use manifest::{
    CONE_TOML_FILE_NAME, ConeDependencySpec, ConeKind, ConeManifest, ConeNativeBuildConfig,
    ConeSection, ConeSelectEntry, ConeSelectWhen,
};
pub use manifest_loader::{
    cone_manifest_path_in_dir, discover_cone_manifest_path, discover_cone_root,
    load_cone_manifest_from_dir, load_cone_manifest_from_path,
};
pub use opt::{InvalidOptLevel, OptLevel};
pub use package::{CONE_MAIN_FILE_NAME, CONE_SRC_DIR_NAME, ConeSourcePackage};
pub use package_loader::{
    host_target_platform_id, load_cone_source_package, load_cone_source_package_for_platform,
    load_cone_source_package_for_platform_with_sysroot_root,
};
pub use scoopc_source::SourceFile;
pub use sysroot::{
    DEFAULT_AUTO_DEPENDENCY_CONES, SYSROOT_OVERLAY_ENV, SysrootSourceConePackage,
    SysrootSourceEntry, canonicalize_sysroot_root, collect_auto_sysroot_source_cone_packages,
    collect_auto_sysroot_source_entries, collect_merged_sysroot_entries, collect_sysroot_files,
    collect_sysroot_source_cone_packages, default_sysroot_path,
    select_auto_sysroot_source_cone_packages, sysroot_source_cone_names,
};
