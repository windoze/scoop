//! Package declarations before workspace inheritance and dependency resolution.

use std::{collections::BTreeMap, ops::Range, path::PathBuf};

use crate::{
    ConeRelativePath, DependencyCoordinateKey, ManifestParseError, ManifestParseErrorKind,
    NativeConfig, RequestedConeKind, SourceSelection, VersionRequirement,
};

mod dependency;
pub mod format;
mod parse;
mod raw;
mod registry;

pub use parse::parse_package_manifest;
pub use registry::RegistryUrl;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ManifestValue<T> {
    pub value: T,
    pub span: Range<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum InheritableValue {
    Value(ManifestValue<String>),
    Workspace { span: Range<usize> },
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConeDeclaration {
    pub span: Range<usize>,
    pub group: InheritableValue,
    pub name: ManifestValue<String>,
    pub version: InheritableValue,
    #[serde(with = "crate::serde_record::RequestedKind")]
    pub kind: RequestedConeKind,
    pub kind_span: Range<usize>,
    pub metadata: ConeMetadata,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConeMetadata {
    pub license: Option<InheritableValue>,
    pub repository: Option<InheritableValue>,
    pub description: Option<ManifestValue<String>>,
    pub readme: Option<ManifestValue<String>>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DependencyDeclaration {
    Workspace { span: Range<usize> },
    Request(DependencyRequest),
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DependencyRequest {
    pub requirement: Option<VersionRequirement>,
    pub source: PackageSource,
    pub span: Range<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PackageSource {
    Default,
    Path(PathBuf),
    Git(GitSource),
    Registry(RegistryUrl),
    Artifact(PathBuf),
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GitSource {
    pub repository: String,
    pub reference: GitReference,
    pub subdir: ConeRelativePath,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GitReference {
    DefaultBranch,
    Branch(String),
    Tag(String),
    Revision(String),
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceDefinition {
    pub members: Vec<ManifestValue<ConeRelativePath>>,
    pub default_members: Option<Vec<ManifestValue<ConeRelativePath>>>,
    pub cone: BTreeMap<String, ManifestValue<String>>,
    pub dependencies: BTreeMap<DependencyCoordinateKey, DependencyRequest>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PublishConfig {
    pub include: Option<Vec<ConeRelativePath>>,
    pub exclude: Vec<ConeRelativePath>,
    pub files: Vec<PublishFile>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PublishFile {
    pub from: PathBuf,
    pub to: ConeRelativePath,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PackageDocument {
    pub schema_span: Range<usize>,
    pub cone: Option<ConeDeclaration>,
    pub workspace: Option<WorkspaceDefinition>,
    pub registry: Option<RegistryUrl>,
    pub dependencies: BTreeMap<DependencyCoordinateKey, DependencyDeclaration>,
    pub test_dependencies: BTreeMap<DependencyCoordinateKey, DependencyDeclaration>,
    pub overrides: BTreeMap<DependencyCoordinateKey, DependencyRequest>,
    pub sources: SourceSelection,
    pub test_sources: SourceSelection,
    pub native: NativeConfig,
    #[serde(default)]
    pub windows: crate::WindowsConfig,
    pub publish: PublishConfig,
}

fn invalid(message: impl Into<String>, span: Range<usize>) -> ManifestParseError {
    ManifestParseError::new(
        ManifestParseErrorKind::InvalidPackage(message.into()),
        Some(span),
    )
}

fn path(value: &str) -> Result<PathBuf, &'static str> {
    if value.is_empty() || value.contains('\0') {
        Err("path must not be empty or contain NUL")
    } else {
        Ok(PathBuf::from(value))
    }
}

#[cfg(test)]
mod tests;
