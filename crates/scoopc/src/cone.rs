//! Cone/project facade adapters.
//!
//! Stage-independent project/cone data lives in `scoop_project_model`; the
//! post-typecheck `.cone` operation layer (archive read/write, ScoopIR export,
//! annotation classes, visibility tables, pre-specialize index, downstream
//! consume/inject) lives in `scoopc_cone`. This module is a single-file façade
//! that re-exports both for backward compatibility with `scoopc::cone::*`.

pub use scoop_project_model::{
    CONE_MAIN_FILE_NAME, CONE_SRC_DIR_NAME, CONE_TOML_FILE_NAME, CONSUMER_CONE_ID,
    ConeDependencySpec, ConeId, ConeInfo, ConeKind, ConeManifest, ConeNativeBuildConfig,
    ConeSection, ConeSelectEntry, ConeSelectWhen, ConeSourcePackage, SourceConeCompilationUnit,
    SourceConeDependencyEdge, SourceConeDependencyKind, SourceConeGraph, SourceConeInfo,
    SourceConeNode, SourceConeRole, SourceConeTrust, StableConeKey, discover_cone_manifest_path,
    discover_cone_root, load_cone_manifest_from_dir, load_cone_manifest_from_path,
    load_cone_source_package, load_cone_source_package_for_platform,
    load_source_cone_graph_for_consumer_package, load_source_cone_graph_for_virtual_consumer,
};
pub use scoopc_cone::{
    CONE_ANNOTATION_CLASSES_FILE_NAME, CONE_API_SCOOPIR_FILE_NAME,
    CONE_ARTIFACT_INPUTS_FINGERPRINT_FILE_NAME, CONE_ARTIFACT_MANIFEST_FILE_NAME,
    CONE_ARTIFACT_OBJS_DIR_NAME, CONE_ARTIFACT_OUTPUTS_FINGERPRINT_FILE_NAME,
    CONE_ARTIFACT_TYPE_STORE_FILE_NAME, CONE_PRE_SPECIALIZE_FILE_NAME,
    CONE_SOURCES_SHA256_FILE_NAME, ConeAnnotationClassEntry, ConeAnnotationClassesFile,
    ConeArchiveApi, ConeArtifact, ConeArtifactError, ConeArtifactFingerprints,
    ConeArtifactFrontendImport, ConeArtifactManifest, ConeArtifactObject,
    ConeArtifactStageProducts, ConePreSpecializeFile, build_cached_cone_import_from_artifact,
    build_frontend_import_for_typechecked_cone,
    collect_cone_preserved_annotation_classes_for_cone_sources, import_upstream_artifacts,
    inject_cone_artifact_frontend_import, inject_cone_dependency_public_api,
    list_cone_archive_entries, load_cone_archive_api, parse_annotation_classes_file,
    read_cone_api_scoopir_from_archive, read_cone_archive_entry, read_cone_manifest_from_archive,
    try_read_cone_archive_entry, write_cone_archive_v0,
};

pub mod annotations {
    pub use scoopc_cone::annotations::*;
}
pub mod archive {
    pub use scoopc_cone::archive::*;
}
pub mod consume {
    pub use scoopc_cone::consume::*;
}
pub mod pre_specialize {
    pub use scoopc_cone::pre_specialize::*;
}
pub mod scoopir {
    pub use scoopc_cone::scoopir::*;
}
pub mod visibility {
    pub use scoopc_cone::visibility::*;
}
pub mod graph {
    pub use scoop_project_model::graph::*;
    pub use scoop_project_model::graph_loader::*;
}
pub mod manifest {
    pub use scoop_project_model::manifest::*;
    pub use scoop_project_model::manifest_loader::*;
}
pub mod package {
    pub use scoop_project_model::package::*;
    pub use scoop_project_model::package_loader::*;
}
