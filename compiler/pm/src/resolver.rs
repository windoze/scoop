//! Single-version resolution. Metadata loading is separate from PubGrub decisions.

use std::{collections::BTreeMap, path::PathBuf};

use scoop_identity::{ConeCoordinate, RequestedConeKind};
use scoop_manifest::{
    DependencyCoordinateKey as Key, VersionRequirement,
    package::{GitSource, RegistryUrl},
};

use crate::{
    ManifestOrigin, PmResult,
    lockfile::{LockedNode, LockedResolution, LockedSource, Purpose},
    sources::git::GitCheckout,
};

mod choice;
mod locked;
mod provider;
mod report;
pub(crate) use choice::Choice;
pub use locked::try_locked;

#[derive(Clone, Debug)]
pub enum SourceRequest {
    Path(PathBuf),
    Git {
        source: GitSource,
        base: PathBuf,
    },
    GitPath {
        checkout: GitCheckout,
        subdir: String,
    },
    Registry(RegistryUrl),
    Artifact(PathBuf),
    System,
    ExactArtifact(ConeCoordinate),
    ImplicitCore,
}

#[derive(Clone, Debug)]
pub struct Requirement {
    pub key: Key,
    pub versions: Vec<VersionRequirement>,
    pub source: SourceRequest,
    pub origins: Vec<ManifestOrigin>,
    pub test: bool,
    pub artifact: Option<scoop_slib::DependencyRecord>,
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub coordinate: ConeCoordinate,
    pub kind: RequestedConeKind,
    pub source: LockedSource,
    pub dependencies: Vec<Requirement>,
    pub yanked: bool,
}

impl Candidate {
    pub fn key(&self) -> Key {
        Key::from_coordinate(&self.coordinate)
    }

    pub(crate) fn choice(&self) -> Choice {
        Choice::new(&self.coordinate, &self.source)
    }
}

#[derive(Clone, Debug, Default)]
pub struct ResolvePolicy {
    pub locked: BTreeMap<Key, LockedNode>,
    pub strict: bool,
    /// None preserves every locked preference; Some(empty) updates all packages.
    pub update: Option<Vec<Key>>,
    /// Registry version or full Git commit, checked by the source loader as well.
    pub precise: Option<(Key, String)>,
}

impl ResolvePolicy {
    pub fn updating(&self, key: &Key) -> bool {
        self.update
            .as_ref()
            .is_some_and(|keys| keys.is_empty() || keys.contains(key))
    }

    pub(crate) fn is_locked(&self, candidate: &Candidate) -> bool {
        self.locked.get(&candidate.key()).is_some_and(|locked| {
            Choice::new(&locked.coordinate, &locked.source) == candidate.choice()
        })
    }

    fn permitted(&self, candidate: &Candidate) -> bool {
        let locked = self.is_locked(candidate);
        if self.strict && !locked {
            return false;
        }
        if candidate.yanked && (!locked || self.updating(&candidate.key())) {
            return false;
        }
        match &self.precise {
            Some((key, value)) if *key == candidate.key() => match &candidate.source {
                LockedSource::Registry { .. } => candidate.coordinate.version() == value,
                LockedSource::Git { commit, .. } => commit == value,
                _ => false,
            },
            _ => true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Solution {
    pub root: Key,
    pub packages: BTreeMap<Key, Candidate>,
}

impl Solution {
    pub fn lock(
        &self,
        root: String,
        purpose: Purpose,
        target: String,
    ) -> PmResult<LockedResolution> {
        let mut nodes: BTreeMap<_, _> = self
            .packages
            .iter()
            .map(|(key, candidate)| {
                let edges = |test| {
                    candidate
                        .dependencies
                        .iter()
                        .filter(|edge| edge.test == test)
                        .map(|edge| self.packages[&edge.key].coordinate.clone())
                        .collect()
                };
                let mut source = candidate.source.clone();
                if let LockedSource::Git { references, .. } = &mut source {
                    references.clear();
                }
                (
                    key.clone(),
                    LockedNode {
                        coordinate: candidate.coordinate.clone(),
                        kind: candidate.kind,
                        source,
                        dependencies: edges(false),
                        test_dependencies: edges(true),
                    },
                )
            })
            .collect();
        for candidate in self.packages.values() {
            for dependency in &candidate.dependencies {
                if let SourceRequest::Git { source, .. } = &dependency.source
                    && let LockedSource::Git { references, .. } = &mut nodes
                        .get_mut(&dependency.key)
                        .expect("selected edge has a node")
                        .source
                {
                    references.push(reference_key(&source.reference));
                }
            }
        }
        for node in nodes.values_mut() {
            if let LockedSource::Git { references, .. } = &mut node.source {
                references.sort();
                references.dedup();
            }
        }
        let mut resolution = LockedResolution {
            root,
            purpose,
            target,
            root_coordinate: self.packages[&self.root].coordinate.clone(),
            nodes: nodes.into_values().collect(),
        };
        resolution.validate()?;
        resolution.canonicalize();
        Ok(resolution)
    }
}

pub fn reference_key(reference: &scoop_manifest::package::GitReference) -> String {
    use scoop_manifest::package::GitReference;
    match reference {
        GitReference::DefaultBranch => "default".into(),
        GitReference::Branch(value) => format!("branch:{value}"),
        GitReference::Tag(value) => format!("tag:{value}"),
        GitReference::Revision(value) => format!("rev:{value}"),
    }
}

/// The production caller supplies the source catalog's metadata loader. It is
/// invoked only for requirements of candidates actually considered by the solver.
pub fn solve(
    root: Candidate,
    policy: &ResolvePolicy,
    load: impl FnMut(&Requirement) -> PmResult<Vec<Candidate>>,
) -> PmResult<Solution> {
    provider::solve(root, policy, load)
}

#[cfg(test)]
mod tests;
