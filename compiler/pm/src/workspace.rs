use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use scoop_identity::{ConeCoordinate, RequestedConeKind};
use scoop_manifest::package::{DependencyRequest, PackageDocument, RegistryUrl};
use scoop_manifest::{DependencyCoordinateKey, NativeConfig, SourceSelection};

use crate::{ManifestOrigin, PmError, PmResult};

mod discover;
mod expand;
mod load;

use load::LoadedManifest;

#[derive(Clone, Debug)]
pub struct ExpandedDependency {
    pub request: DependencyRequest,
    pub origin: ManifestOrigin,
    pub inherited_at: Option<ManifestOrigin>,
    /// Relative source locators use the declaration's directory, not the member's.
    pub base: PathBuf,
    pub registry: Option<RegistryUrl>,
}

#[derive(Clone, Debug)]
pub struct ExpandedCone {
    pub coordinate: ConeCoordinate,
    pub kind: RequestedConeKind,
    pub dependencies: BTreeMap<DependencyCoordinateKey, ExpandedDependency>,
    pub test_dependencies: BTreeMap<DependencyCoordinateKey, ExpandedDependency>,
    pub sources: SourceSelection,
    pub test_sources: SourceSelection,
    pub native: NativeConfig,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct WorkspaceMember {
    pub relative_path: String,
    pub directory: PathBuf,
    pub manifest: PathBuf,
    pub cone: ExpandedCone,
}

#[derive(Debug)]
pub struct Workspace {
    pub root: PathBuf,
    pub manifest: PathBuf,
    pub members: BTreeMap<DependencyCoordinateKey, WorkspaceMember>,
    pub document: Arc<PackageDocument>,
    defaults: Vec<DependencyCoordinateKey>,
    invoking_member: Option<DependencyCoordinateKey>,
}

#[derive(Clone, Debug, Default)]
pub enum MemberSelection {
    #[default]
    Default,
    All,
    Packages(Vec<DependencyCoordinateKey>),
}

impl Workspace {
    pub fn discover(cwd: &Path, input: Option<&Path>) -> PmResult<Self> {
        discover::discover(cwd, input)
    }

    pub fn select(&self, selection: &MemberSelection) -> PmResult<Vec<&WorkspaceMember>> {
        let keys: BTreeSet<_> = match selection {
            MemberSelection::All => self.members.keys().cloned().collect(),
            MemberSelection::Packages(keys) => keys.iter().cloned().collect(),
            MemberSelection::Default => self
                .invoking_member
                .as_ref()
                .map(|key| vec![key.clone()])
                .unwrap_or_else(|| self.defaults.clone())
                .into_iter()
                .collect(),
        };
        keys.into_iter()
            .map(|key| {
                self.members.get(&key).ok_or_else(|| {
                    PmError::new(format!("{key} is not a member of {}", self.root.display()))
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
