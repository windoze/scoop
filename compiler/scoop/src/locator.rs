use std::path::{Path, PathBuf};
use std::rc::Rc;

use scoop_identity::{ConeCoordinate, ConeIdentity, RequestedConeKind};
use scoop_manifest::{
    DependencyCoordinateKey, DependencyLocator, LoadedConeManifest, ManifestRootLocator,
};
use scoop_slib::{ArtifactFingerprint, ArtifactManifestSummaryV1, ConeKind, ConeSourceForm};

use crate::{ArtifactCacheRoot, ArtifactSearchRoot, BuildArtifact};

mod error;
pub use error::{DependencyLocatorError, LocatorIoOperation};

mod default_core;
pub(crate) use default_core::locate_default_core;
mod default_library;
pub(crate) use default_library::locate_build_dependency;

pub(crate) fn artifact_candidate_path(root: &Path, coordinate: &ConeCoordinate) -> PathBuf {
    scoop_toolchain::artifact_candidate_path(root, coordinate)
}

#[derive(Debug)]
pub(crate) enum LocatedDependencyClaim {
    Source(Box<ManifestSourceProjection>),
    Prebuilt(Box<PrebuiltArtifactProjection>),
}

#[derive(Debug)]
pub(crate) struct ManifestSourceProjection {
    manifest: LoadedConeManifest,
}

impl ManifestSourceProjection {
    pub(crate) const fn manifest(&self) -> &LoadedConeManifest {
        &self.manifest
    }

    pub(crate) fn into_manifest(self) -> LoadedConeManifest {
        self.manifest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrebuiltArtifactCandidate {
    artifact: Rc<BuildArtifact>,
}

impl PrebuiltArtifactCandidate {
    pub(crate) fn resolved_path(&self) -> &Path {
        self.artifact.path()
    }

    pub(crate) fn summary(&self) -> &ArtifactManifestSummaryV1 {
        self.artifact.summary()
    }

    pub(crate) fn into_artifact(self) -> Rc<BuildArtifact> {
        self.artifact
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrebuiltArtifactProjection {
    coordinate: ConeCoordinate,
    artifact_fingerprint: ArtifactFingerprint,
    candidates: NonEmptyPrebuiltArtifactCandidates,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NonEmptyPrebuiltArtifactCandidates {
    first: PrebuiltArtifactCandidate,
    rest: Vec<PrebuiltArtifactCandidate>,
}

impl PrebuiltArtifactProjection {
    pub(crate) fn from_prepared(path: PathBuf, summary: ArtifactManifestSummaryV1) -> Self {
        Self {
            coordinate: summary.cone().coordinate().clone(),
            artifact_fingerprint: summary.artifact_fingerprint(),
            candidates: NonEmptyPrebuiltArtifactCandidates {
                first: PrebuiltArtifactCandidate {
                    artifact: BuildArtifact::from_result(path, summary),
                },
                rest: Vec::new(),
            },
        }
    }

    pub(crate) const fn coordinate(&self) -> &ConeCoordinate {
        &self.coordinate
    }

    pub(crate) const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.artifact_fingerprint
    }

    pub(crate) const fn first_candidate(&self) -> &PrebuiltArtifactCandidate {
        &self.candidates.first
    }

    pub(crate) fn candidate_count(&self) -> usize {
        1 + self.candidates.rest.len()
    }

    pub(crate) fn candidate_iter(&self) -> impl Iterator<Item = &PrebuiltArtifactCandidate> {
        std::iter::once(&self.candidates.first).chain(&self.candidates.rest)
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        ConeCoordinate,
        ArtifactFingerprint,
        PrebuiltArtifactCandidate,
        Vec<PrebuiltArtifactCandidate>,
    ) {
        (
            self.coordinate,
            self.artifact_fingerprint,
            self.candidates.first,
            self.candidates.rest,
        )
    }

    pub(crate) fn merge_same_artifact(&mut self, other: Self) {
        for candidate in other.candidate_iter().cloned() {
            if !self
                .candidate_iter()
                .any(|existing| existing.resolved_path() == candidate.resolved_path())
            {
                self.candidates.rest.push(candidate);
            }
        }
        self.candidates
            .rest
            .sort_by(|left, right| left.resolved_path().cmp(right.resolved_path()));
    }
}

pub(crate) fn locate_manifest_dependency(
    parent: &LoadedConeManifest,
    key: &DependencyCoordinateKey,
    search_roots: &[ArtifactSearchRoot],
    target: scoop_lir::ValidatedLirTargetSelection,
    cache: &ArtifactCacheRoot,
) -> Result<LocatedDependencyClaim, DependencyLocatorError> {
    let production = parent
        .parsed()
        .semantic()
        .dependency_iter()
        .find(|(candidate, _)| *candidate == key);
    let (coordinate, locator) = if let Some((_, coordinate)) = production {
        let locator = parent
            .parsed()
            .locators()
            .get(key)
            .ok_or_else(|| DependencyLocatorError::MissingLocatorProjection(key.clone()))?;
        (coordinate, locator)
    } else {
        let (_, declaration) = parent
            .parsed()
            .tests()
            .dependency_iter()
            .find(|(candidate, _)| *candidate == key)
            .ok_or_else(|| DependencyLocatorError::UndeclaredDependency(key.clone()))?;
        (declaration.coordinate(), declaration.locator())
    };

    match locator {
        DependencyLocator::SourcePath(path) => locate_source_dependency(
            parent,
            coordinate,
            resolve_from_manifest(parent, path.as_path()),
            cache,
        )
        .map(|source| LocatedDependencyClaim::Source(Box::new(source))),
        DependencyLocator::ArtifactPath(path) => locate_artifact_dependency(
            resolve_from_manifest(parent, path.as_path()),
            coordinate,
            target,
            cache,
        ),
        DependencyLocator::SearchRoots => {
            locate_from_search_roots(coordinate, search_roots, target, cache)
                .map(|prebuilt| LocatedDependencyClaim::Prebuilt(Box::new(prebuilt)))
        }
    }
}

fn locate_artifact_dependency(
    path: PathBuf,
    coordinate: &ConeCoordinate,
    target: scoop_lir::ValidatedLirTargetSelection,
    cache: &ArtifactCacheRoot,
) -> Result<LocatedDependencyClaim, DependencyLocatorError> {
    let candidate = probe_artifact_candidate(path, coordinate, target, cache)?;
    Ok(LocatedDependencyClaim::Prebuilt(Box::new(
        PrebuiltArtifactProjection {
            coordinate: coordinate.clone(),
            artifact_fingerprint: candidate.summary().artifact_fingerprint(),
            candidates: NonEmptyPrebuiltArtifactCandidates {
                first: candidate,
                rest: Vec::new(),
            },
        },
    )))
}

fn resolve_from_manifest(parent: &LoadedConeManifest, locator: &Path) -> PathBuf {
    if locator.is_absolute() {
        locator.to_path_buf()
    } else {
        parent.real_root().join(locator)
    }
}

fn locate_source_dependency(
    parent: &LoadedConeManifest,
    expected: &ConeCoordinate,
    path: PathBuf,
    cache: &ArtifactCacheRoot,
) -> Result<ManifestSourceProjection, DependencyLocatorError> {
    let manifest = load_dependency_manifest(expected, path, cache)?;
    if manifest.real_root() == parent.real_root() {
        return Err(DependencyLocatorError::SelfSourceLocator {
            coordinate: expected.clone(),
            root: manifest.real_root().to_path_buf(),
        });
    }
    Ok(ManifestSourceProjection { manifest })
}

pub(crate) fn load_dependency_manifest(
    expected: &ConeCoordinate,
    path: PathBuf,
    cache: &ArtifactCacheRoot,
) -> Result<LoadedConeManifest, DependencyLocatorError> {
    let locator = ManifestRootLocator::from_path(path).map_err(DependencyLocatorError::Manifest)?;
    let manifest = cache
        .load_manifest(&locator)
        .map_err(DependencyLocatorError::Manifest)?;
    if manifest.coordinate() != expected {
        return Err(DependencyLocatorError::CoordinateMismatch {
            expected: Box::new(expected.clone()),
            actual: Box::new(manifest.coordinate().clone()),
            path: manifest.manifest_path().to_path_buf(),
        });
    }
    if manifest.parsed().semantic().requested_kind() != RequestedConeKind::Library {
        return Err(DependencyLocatorError::ExecutableDependency {
            coordinate: expected.clone(),
            path: manifest.manifest_path().to_path_buf(),
        });
    }
    Ok(manifest)
}

pub(crate) fn locate_from_search_roots(
    coordinate: &ConeCoordinate,
    search_roots: &[ArtifactSearchRoot],
    target: scoop_lir::ValidatedLirTargetSelection,
    cache: &ArtifactCacheRoot,
) -> Result<PrebuiltArtifactProjection, DependencyLocatorError> {
    let mut checked = Vec::new();
    let mut resolved = Vec::new();
    for root in search_roots {
        let candidate = artifact_candidate_path(root.as_path(), coordinate);

        checked.push(candidate.clone());
        match std::fs::symlink_metadata(&candidate) {
            Ok(_) => {
                let path = canonicalize(&candidate)?;
                if !resolved.contains(&path) {
                    resolved.push(path);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(DependencyLocatorError::Io {
                    operation: LocatorIoOperation::Inspect,
                    path: candidate,
                    source,
                });
            }
        }
    }
    if resolved.is_empty() {
        return Err(DependencyLocatorError::ArtifactNotFound {
            coordinate: coordinate.clone(),
            checked,
        });
    }

    resolved.sort();
    let mut paths = resolved.into_iter();
    let Some(first_path) = paths.next() else {
        return Err(DependencyLocatorError::ArtifactNotFound {
            coordinate: coordinate.clone(),
            checked,
        });
    };
    let first = probe_artifact_candidate(first_path, coordinate, target, cache)?;
    let expected_fingerprint = first.summary().artifact_fingerprint();
    let mut rest = Vec::new();
    for path in paths {
        rest.push(probe_artifact_candidate(path, coordinate, target, cache)?);
    }
    if rest
        .iter()
        .any(|candidate| candidate.summary().artifact_fingerprint() != expected_fingerprint)
    {
        let candidates = std::iter::once(first)
            .chain(rest)
            .map(|candidate| {
                (
                    candidate.resolved_path().to_path_buf(),
                    candidate.summary().artifact_fingerprint(),
                )
            })
            .collect();
        return Err(DependencyLocatorError::AmbiguousArtifact {
            coordinate: coordinate.clone(),
            candidates,
        });
    }
    Ok(PrebuiltArtifactProjection {
        coordinate: coordinate.clone(),
        artifact_fingerprint: expected_fingerprint,
        candidates: NonEmptyPrebuiltArtifactCandidates { first, rest },
    })
}

fn probe_artifact_candidate(
    path: PathBuf,
    expected: &ConeCoordinate,
    target: scoop_lir::ValidatedLirTargetSelection,
    cache: &ArtifactCacheRoot,
) -> Result<PrebuiltArtifactCandidate, DependencyLocatorError> {
    let resolved_path = canonicalize(&path)?;
    let artifact = cache.load_artifact(&resolved_path, target)?;
    validate_artifact_shape(expected, &resolved_path, artifact.summary())?;

    Ok(PrebuiltArtifactCandidate { artifact })
}

fn validate_artifact_shape(
    expected: &ConeCoordinate,
    path: &Path,
    summary: &ArtifactManifestSummaryV1,
) -> Result<(), DependencyLocatorError> {
    if summary.cone().coordinate() != expected {
        return Err(DependencyLocatorError::CoordinateMismatch {
            expected: Box::new(expected.clone()),
            actual: Box::new(summary.cone().coordinate().clone()),
            path: path.to_path_buf(),
        });
    }
    if summary.cone().identity() == ConeIdentity::SINGLE_FILE {
        return Err(DependencyLocatorError::ReservedArtifact {
            coordinate: expected.clone(),
            path: path.to_path_buf(),
        });
    }
    if summary.cone().kind() != ConeKind::Library {
        return Err(DependencyLocatorError::ExecutableDependency {
            coordinate: expected.clone(),
            path: path.to_path_buf(),
        });
    }
    if summary.cone().source_form() != ConeSourceForm::Manifest {
        return Err(DependencyLocatorError::InvalidArtifactSourceForm {
            coordinate: expected.clone(),
            path: path.to_path_buf(),
            actual: summary.cone().source_form(),
        });
    }
    Ok(())
}

fn canonicalize(path: &Path) -> Result<PathBuf, DependencyLocatorError> {
    std::fs::canonicalize(path).map_err(|source| DependencyLocatorError::Io {
        operation: LocatorIoOperation::Canonicalize,
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests;
