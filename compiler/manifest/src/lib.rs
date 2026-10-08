//! Strict `Cone.toml` parsing and deterministic current-Cone source discovery.
//!
//! Locator paths remain request-local. Successful semantic values contain only
//! canonical Cone coordinates, requested output kind, exact dependency
//! coordinates, source identities, source text, and content digests.

mod discovery;
mod file_snapshot;
mod input;
mod native;
mod root;
mod selection;
mod semantic;
mod single_file;
mod stable_file;

pub use discovery::{
    DiscoveredManifestSources, DiscoveredSource, DiscoveryIoOperation, SourceDiscoveryError,
    SourceDiscoveryErrorKind, SourceDisplayLocator, discover_manifest_sources,
};
pub use input::{
    CurrentConeInput, CurrentConeOperandError, CurrentConeOperandErrorKind,
    classify_current_cone_operand,
};
pub use native::{NativeCompileFlag, NativeConfig, NativeIncludeFlag, NativeSourceLanguage};
pub use root::{
    LoadedConeManifest, ManifestRootError, ManifestRootErrorKind, ManifestRootIoOperation,
    ManifestRootLocator, load_cone_manifest,
};
pub use scoop_identity::RequestedConeKind;
pub use selection::{ConditionalSourcePath, ConeRelativePath, SourceSelection, TargetPredicate};
pub use semantic::{
    ConeManifestSemantic, ConeManifestSpans, DependencyCoordinateKey, DependencyLocator,
    DependencyLocatorTable, DependencyManifestSpans, HostPathLocator, ManifestDiagnosticSpans,
    ManifestParseError, ManifestParseErrorKind, ManifestSpan, ParsedConeManifest,
    parse_cone_manifest,
};
pub use single_file::{
    SingleFileInputError, SingleFileInputErrorKind, SingleFileInputIoOperation, SingleFileLocator,
    load_single_file_source,
};

pub use file_snapshot::{ImmutableInputSnapshot, SnapshotFileError, SnapshotIoOperation};
