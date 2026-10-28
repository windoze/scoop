//! Source metadata and local inputs used by resolution and preparation.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use scoop_identity::{ConeCoordinate, RequestedConeKind};
use scoop_manifest::{
    DependencyCoordinateKey as Key,
    package::{GitReference, GitSource, RegistryUrl},
};

use crate::{
    ExpandedDependency, ManifestOrigin, PmError, PmResult, Workspace, WorkspaceMember,
    cache::{FileStamp, PackageCache},
    lockfile::{LockedSource, Purpose},
    resolver::{
        Candidate, Choice, Requirement, ResolvePolicy, Solution, SourceRequest, reference_key,
    },
    sources::{
        git::{self, GitCheckout},
        registry::{self, RegistryClient, RegistryIndex, RegistryVersion},
    },
};

mod artifacts;
mod remote;
mod requirements;

#[derive(Clone, Debug)]
pub enum PackageInput {
    Source(Box<WorkspaceMember>),
    Prebuilt {
        path: PathBuf,
        summary: Box<scoop_slib::ArtifactManifestSummaryV1>,
    },
}

#[derive(Clone, Debug)]
pub struct CatalogOptions {
    pub lock_directory: PathBuf,
    pub target: scoop_lir::ValidatedLirTargetSelection,
    pub sysroot: PathBuf,
    pub artifact_roots: Vec<PathBuf>,
    pub default_registry: Option<RegistryUrl>,
    pub offline: bool,
    /// Metadata inspection must not materialize sources even from cached objects/archives.
    pub local_sources_only: bool,
}

#[derive(Clone)]
enum Context {
    Local,
    Git(GitCheckout),
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
enum SourceCacheKey {
    Path(PathBuf),
    Git {
        repository: String,
        commit: String,
        subdir: String,
    },
}

pub struct Catalog<'a> {
    cache: &'a PackageCache,
    http: RegistryClient<'a>,
    policy: ResolvePolicy,
    options: CatalogOptions,
    overrides: BTreeMap<Key, ExpandedDependency>,
    inputs: BTreeMap<PathBuf, FileStamp>,
    missing_inputs: BTreeSet<PathBuf>,
    packages: BTreeMap<(Key, Choice), PackageInput>,
    source_candidates: BTreeMap<SourceCacheKey, Candidate>,
    indexes: BTreeMap<(String, Key), RegistryIndex>,
    refreshable_indexes: BTreeSet<Key>,
    versions: BTreeMap<(Key, Choice), RegistryVersion>,
    git_refs: BTreeMap<(String, String), GitCheckout>,
    overlays: crate::workspace::ManifestOverlays,
}

impl<'a> Catalog<'a> {
    pub fn new(
        cache: &'a PackageCache,
        workspace: &Workspace,
        policy: &ResolvePolicy,
        options: CatalogOptions,
    ) -> Self {
        let overrides = workspace
            .document
            .overrides
            .iter()
            .map(|(key, request)| {
                (
                    key.clone(),
                    ExpandedDependency {
                        request: request.clone(),
                        origin: ManifestOrigin {
                            manifest: workspace.manifest.clone(),
                            span: request.span.clone(),
                        },
                        inherited_at: None,
                        base: workspace.root.clone(),
                        registry: workspace.document.registry.clone(),
                    },
                )
            })
            .collect();
        Self {
            overlays: workspace.overlays.clone(),
            cache,
            http: RegistryClient::new(cache),
            policy: policy.clone(),
            options,
            overrides,
            inputs: workspace.inputs.iter().cloned().collect(),
            missing_inputs: workspace.missing_inputs.clone(),
            packages: BTreeMap::new(),
            source_candidates: BTreeMap::new(),
            indexes: BTreeMap::new(),
            refreshable_indexes: BTreeSet::new(),
            versions: BTreeMap::new(),
            git_refs: BTreeMap::new(),
        }
    }

    pub fn root(&mut self, member: &WorkspaceMember, purpose: Purpose) -> PmResult<Candidate> {
        let source = LockedSource::Path {
            path: self.lock_path(&member.directory)?,
        };
        self.register_source(member.clone(), source, &Context::Local, purpose)
    }

    pub(crate) fn update_policy(&mut self, policy: &ResolvePolicy) {
        self.indexes
            .retain(|(_, key), _| self.policy.updating(key) || !policy.updating(key));
        self.refreshable_indexes.retain(|key| !policy.updating(key));
        self.policy = policy.clone();
    }

    pub(crate) fn can_refresh_index(&self, key: &Key) -> bool {
        self.refreshable_indexes.contains(key)
    }

    pub fn candidates(&mut self, requirement: &Requirement) -> PmResult<Vec<Candidate>> {
        if self.policy.strict && !self.policy.locked.contains_key(&requirement.key) {
            return Err(PmError::new(format!(
                "Cone.lock does not include {}; resolve without --locked",
                requirement.key
            )));
        }
        let result = match &requirement.source {
            SourceRequest::Path(path) => self.local(path, &Context::Local).map(|node| vec![node]),
            SourceRequest::Git { source, base } => {
                self.git(requirement, source, base).map(|node| vec![node])
            }
            SourceRequest::GitPath { checkout, subdir } => self
                .local(&checkout.root.join(subdir), &Context::Git(checkout.clone()))
                .map(|node| vec![node]),
            SourceRequest::Registry(registry) => self.registry(requirement, registry),
            SourceRequest::Artifact(path) => {
                let source = self
                    .lock_path(path)
                    .map(|path| LockedSource::Artifact { path });
                source
                    .and_then(|source| self.artifact(path, source))
                    .map(|node| vec![node])
            }
            SourceRequest::System
            | SourceRequest::ImplicitCore
            | SourceRequest::ExactArtifact(_) => self.search_artifacts(requirement),
        };
        result.map_err(|mut error| {
            error.origins.extend(requirement.origins.clone());
            error
        })
    }

    pub fn input(&self, candidate: &Candidate) -> &PackageInput {
        &self.packages[&(candidate.key(), candidate.choice())]
    }

    pub fn check_inputs(&self) -> PmResult<()> {
        for (path, stamp) in &self.inputs {
            stamp.unchanged(path)?;
        }
        Ok(())
    }

    pub(crate) fn input_stamps(&self) -> &BTreeMap<PathBuf, FileStamp> {
        &self.inputs
    }

    pub(crate) fn missing_inputs(&self) -> &BTreeSet<PathBuf> {
        &self.missing_inputs
    }

    fn observe_presence(&mut self, path: &Path) -> PmResult<bool> {
        match FileStamp::optional(path)? {
            Some(stamp) => {
                self.inputs.insert(path.to_owned(), stamp);
                Ok(true)
            }
            None => {
                self.missing_inputs.insert(path.to_owned());
                Ok(false)
            }
        }
    }

    pub fn materialize(&mut self, solution: &Solution) -> PmResult<()> {
        for candidate in solution.packages.values() {
            if let LockedSource::Registry { registry, .. } = &candidate.source {
                let version = &self.versions[&(candidate.key(), candidate.choice())];
                let cached = if self.options.local_sources_only {
                    registry::cached(self.cache, registry, &candidate.coordinate, version)?
                        .ok_or_else(|| {
                            PmError::new(format!(
                                "sources for {} are not cached; run scoop pm fetch",
                                candidate.coordinate
                            ))
                        })?
                } else {
                    registry::fetch(
                        &self.http,
                        registry,
                        &candidate.coordinate,
                        version,
                        self.options.offline,
                    )?
                };
                let member =
                    Workspace::packaged(cached.root, cached.manifest_text, cached.document)?;
                self.packages.insert(
                    (candidate.key(), candidate.choice()),
                    PackageInput::Source(Box::new(member)),
                );
            }
        }
        for candidate in solution.packages.values() {
            for requirement in &candidate.dependencies {
                if let Some(expected) = &requirement.artifact
                    && let PackageInput::Prebuilt { summary, .. } =
                        self.input(&solution.packages[&requirement.key])
                    && summary.dependency_record() != *expected
                {
                    return Err(PmError::new(format!(
                        "{} requires a different semantic fingerprint of {}; rebuild the upstream artifact from its source",
                        candidate.coordinate,
                        expected.coordinate()
                    )));
                }
            }
        }
        self.check_inputs()
    }

    fn local(&mut self, directory: &Path, context: &Context) -> PmResult<Candidate> {
        let git_subdir = match context {
            Context::Git(checkout) => Some(relative(directory, &checkout.root)?),
            Context::Local => None,
        };
        let directory =
            std::fs::canonicalize(directory).map_err(|error| PmError::io(directory, error))?;
        let cache_key = match context {
            Context::Local => SourceCacheKey::Path(directory.clone()),
            Context::Git(checkout) => SourceCacheKey::Git {
                repository: checkout.repository.clone(),
                commit: checkout.commit.clone(),
                subdir: git_subdir.clone().expect("Git context has a subdir"),
            },
        };
        if let Some(candidate) = self.source_candidates.get(&cache_key) {
            return Ok(candidate.clone());
        }
        let boundary = match context {
            Context::Git(checkout) => Some(checkout.root.as_path()),
            _ => None,
        };
        let workspace = Workspace::discover_cached(
            &directory,
            Some(&directory),
            self.cache,
            boundary,
            self.overlays.clone(),
        )?;
        self.inputs.extend(workspace.inputs);
        self.missing_inputs.extend(workspace.missing_inputs);
        let member = workspace
            .members
            .into_values()
            .find(|member| member.directory == directory)
            .ok_or_else(|| {
                PmError::new(format!(
                    "{} is a virtual workspace; select a Cone subdirectory",
                    directory.display()
                ))
            })?;
        let source = match context {
            Context::Git(checkout) => LockedSource::Git {
                git: checkout.repository.clone(),
                commit: checkout.commit.clone(),
                subdir: git_subdir.expect("Git context has a subdir"),
                references: Vec::new(),
            },
            Context::Local => LockedSource::Path {
                path: self.lock_path(&directory)?,
            },
        };
        let candidate = self.register_source(member, source, context, Purpose::Production)?;
        self.source_candidates.insert(cache_key, candidate.clone());
        Ok(candidate)
    }

    fn register_source(
        &mut self,
        member: WorkspaceMember,
        source: LockedSource,
        context: &Context,
        purpose: Purpose,
    ) -> PmResult<Candidate> {
        let dependencies = self.source_requirements(&member, context, purpose)?;
        let candidate = Candidate {
            coordinate: member.cone.coordinate.clone(),
            kind: member.cone.kind,
            source,
            dependencies,
            yanked: false,
        };
        self.packages.insert(
            (candidate.key(), candidate.choice()),
            PackageInput::Source(Box::new(member)),
        );
        Ok(candidate)
    }

    fn lock_path(&self, path: &Path) -> PmResult<String> {
        relative(path, &self.options.lock_directory)
    }
}

fn relative(path: &Path, base: &Path) -> PmResult<String> {
    let path = pathdiff::diff_paths(path, base).unwrap_or_else(|| path.to_owned());
    let text = path
        .to_str()
        .ok_or_else(|| PmError::new("portable Cone.lock source paths require UTF-8"))?;
    Ok(if text.is_empty() {
        ".".into()
    } else if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text.into()
    })
}

fn lexical(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                result.pop();
            }
            other => result.push(other.as_os_str()),
        }
    }
    result
}
