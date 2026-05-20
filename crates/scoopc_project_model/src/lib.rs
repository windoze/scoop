//! Stage-independent project, source-cone, and compilation-unit model.
//!
//! This base crate owns project membership, source-cone graph data, source
//! trust, dependency topology checks, and backend-neutral manifest settings. The
//! `scoopc` facade may provide filesystem/session adapters, but it must not
//! duplicate these definitions or make this crate depend on stage/fact/backend
//! crates.

#![forbid(unsafe_code)]

pub mod graph;
pub mod manifest;
pub mod opt;
pub mod package;

pub use graph::{
    CONSUMER_CONE_ID, ConeId, ConeInfo, SourceConeDependencyEdge, SourceConeDependencyKind,
    SourceConeGraph, SourceConeInfo, SourceConeNode, SourceConeRole, SourceConeTrust,
    StableConeKey,
};
pub use manifest::{
    CONE_TOML_FILE_NAME, ConeDependencySpec, ConeKind, ConeManifest, ConeNativeBuildConfig,
    ConeSection, ConeSelectEntry, ConeSelectWhen,
};
pub use opt::{InvalidOptLevel, OptLevel};
pub use package::{CONE_MAIN_FILE_NAME, CONE_SRC_DIR_NAME, ConeSourcePackage};
