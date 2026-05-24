//! Cone operation layer.
//!
//! This crate owns the post-typecheck cone publishing/consumption operations:
//! `.cone` archive read/write, ScoopIR public-API export, annotation classes,
//! visibility tables, pre-specialize index, and downstream consume/inject. It
//! sits above all stage crates and is consumed by the `scoopc` facade.

#![forbid(unsafe_code)]

pub mod annotations;
pub mod archive;
pub mod artifact;
pub mod consume;
pub mod pre_specialize;
pub mod scoopir;
pub mod visibility;

pub use annotations::{
    CONE_ANNOTATION_CLASSES_FILE_NAME, ConeAnnotationClassEntry, ConeAnnotationClassesFile,
    collect_cone_preserved_annotation_classes_for_cone_sources, parse_annotation_classes_file,
};
pub use archive::{
    CONE_API_SCOOPIR_FILE_NAME, CONE_SOURCES_SHA256_FILE_NAME, list_cone_archive_entries,
    read_cone_archive_entry, try_read_cone_archive_entry, write_cone_archive_v0,
};
pub use artifact::{
    CONE_ARTIFACT_EFFECT_FACTS_FILE_NAME, CONE_ARTIFACT_HIR_FACTS_FILE_NAME,
    CONE_ARTIFACT_INPUTS_FINGERPRINT_FILE_NAME, CONE_ARTIFACT_LIR_FACTS_FILE_NAME,
    CONE_ARTIFACT_LIR_PROGRAM_FILE_NAME, CONE_ARTIFACT_MANIFEST_FILE_NAME,
    CONE_ARTIFACT_MIR_FACTS_FILE_NAME, CONE_ARTIFACT_OBJS_DIR_NAME,
    CONE_ARTIFACT_OUTPUTS_FINGERPRINT_FILE_NAME, ConeArtifact, ConeArtifactError,
    ConeArtifactFingerprints, ConeArtifactManifest, ConeArtifactObject, ConeArtifactSchemaVersions,
    ConeArtifactStageProducts,
};
pub use consume::{
    ConeArchiveApi, inject_cone_dependency_public_api, load_cone_archive_api,
    read_cone_api_scoopir_from_archive, read_cone_manifest_from_archive,
};
pub use pre_specialize::{CONE_PRE_SPECIALIZE_FILE_NAME, ConePreSpecializeFile};
