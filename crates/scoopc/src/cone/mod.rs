//! Cone（包/稳定 IR/分发）相关基础设施。
//!
//! 当前阶段已落地：
//! - T1101：`Cone.toml` 的最小解析（name/version/kind/deps）；
//! - T1102：source package 的加载规则（cone root → sources 列表 + main 入口定位）。

pub mod annotations;
pub mod archive;
pub mod consume;
pub mod graph;
pub mod manifest;
pub mod package;
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
pub use consume::{
    ConeArchiveApi, inject_cone_dependency_public_api, load_cone_archive_api,
    read_cone_api_scoopir_from_archive, read_cone_manifest_from_archive,
};
pub use graph::{
    CONSUMER_CONE_ID, SourceConeDependencyEdge, SourceConeDependencyKind, SourceConeGraph,
    SourceConeNode, SourceConeRole, SourceConeTrust,
};
pub use manifest::{
    CONE_TOML_FILE_NAME, ConeDependencySpec, ConeKind, ConeManifest, ConeNativeBuildConfig,
    ConeSection, discover_cone_manifest_path, discover_cone_root,
};
pub use package::{
    CONE_MAIN_FILE_NAME, CONE_SRC_DIR_NAME, ConeSourcePackage, load_cone_source_package,
    load_cone_source_package_for_platform,
};
pub use pre_specialize::{CONE_PRE_SPECIALIZE_FILE_NAME, ConePreSpecializeFile};
