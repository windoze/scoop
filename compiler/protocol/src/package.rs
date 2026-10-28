//! Same-host package preparation data, independent of package-manager implementation.

use std::collections::BTreeMap;

use scoop_identity::{ConeCoordinate, RequestedConeKind};
use scoop_manifest::{DependencyCoordinateKey, ParsedConeManifest, package::format};
use scoop_slib::ArtifactManifestSummaryV1;
use serde::{Deserialize, Serialize};

use crate::{CompilationPurpose, HostPathCarrier};

mod diagnostic;
mod members;
mod overlays;
mod summary;
mod validate;

pub use members::{PackageMemberV1, PackageMembersV1};
pub use overlays::{ManifestOverlayV1, ManifestOverlaysV1};

pub const PACKAGE_PROTOCOL_VERSION: u32 = 2;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageDiagnosticV1 {
    pub version: u32,
    #[serde(with = "diagnostic")]
    pub diagnostic: crate::StructuredDiagnosticV1,
}

impl PackageDiagnosticV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate::version(self.version)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrepareRequestV1 {
    pub version: u32,
    pub cwd: HostPathCarrier,
    pub input: Option<HostPathCarrier>,
    pub members: PackageMemberSelectionV1,
    pub purpose: CompilationPurpose,
    pub target: String,
    pub sysroot: HostPathCarrier,
    pub artifact_roots: Vec<HostPathCarrier>,
    pub package_cache: Option<HostPathCarrier>,
    pub lockfile: Option<HostPathCarrier>,
    pub locked: bool,
    pub offline: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "selection", rename_all = "snake_case", deny_unknown_fields)]
pub enum PackageMemberSelectionV1 {
    Default,
    All,
    Packages { keys: Vec<DependencyCoordinateKey> },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedWorkspaceV1 {
    pub version: u32,
    pub workspace: HostPathCarrier,
    pub lockfile: HostPathCarrier,
    pub resolutions: Vec<PreparedResolutionV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedResolutionV1 {
    pub root: String,
    #[serde(with = "format::coordinate")]
    pub coordinate: ConeCoordinate,
    pub purpose: CompilationPurpose,
    pub target: String,
    pub nodes: Vec<PreparedNodeV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedNodeV1 {
    pub cone: PreparedConeV1,
    #[serde(with = "format::coordinates")]
    pub dependencies: Vec<ConeCoordinate>,
    #[serde(with = "format::coordinates")]
    pub test_dependencies: Vec<ConeCoordinate>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "input", rename_all = "snake_case", deny_unknown_fields)]
pub enum PreparedConeV1 {
    Source {
        config: Box<ExpandedConeInputV1>,
    },
    Prebuilt {
        artifact: HostPathCarrier,
        #[serde(with = "summary")]
        summary: Box<ArtifactManifestSummaryV1>,
    },
}

impl PreparedConeV1 {
    pub fn coordinate(&self) -> &ConeCoordinate {
        match self {
            Self::Source { config } => config.manifest.semantic().coordinate(),
            Self::Prebuilt { summary, .. } => summary.cone().coordinate(),
        }
    }

    pub fn kind(&self) -> RequestedConeKind {
        match self {
            Self::Source { config } => config.manifest.semantic().requested_kind(),
            Self::Prebuilt { summary, .. } => match summary.cone().kind() {
                scoop_slib::ConeKind::Library => RequestedConeKind::Library,
                scoop_slib::ConeKind::Executable => RequestedConeKind::Executable,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpandedConeInputV1 {
    pub input_root: HostPathCarrier,
    pub root: HostPathCarrier,
    pub manifest_path: HostPathCarrier,
    /// Original configuration text is retained solely for diagnostics.
    pub manifest_text: String,
    pub manifest: ParsedConeManifest,
    pub metadata: BTreeMap<String, String>,
}

impl ExpandedConeInputV1 {
    pub fn from_manifest(
        manifest: &scoop_manifest::LoadedConeManifest,
    ) -> Result<Self, crate::HostPathError> {
        Ok(Self {
            input_root: HostPathCarrier::from_path(manifest.input_root())?,
            root: HostPathCarrier::from_path(manifest.real_root())?,
            manifest_path: HostPathCarrier::from_path(manifest.manifest_path())?,
            manifest_text: manifest.source_text().to_owned(),
            manifest: manifest.parsed().clone().into_bound(),
            metadata: manifest.metadata().clone(),
        })
    }
}

#[cfg(test)]
mod tests;
