use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use scoop_identity::{ConeCoordinate, ConeIdentity, RequestedConeKind};
use scoop_manifest::{
    DependencyCoordinateKey, DependencyLocator, LoadedConeManifest, ManifestRootError,
    ManifestRootLocator,
};
use scoop_slib::{
    ArtifactFingerprint, ArtifactManifestSummaryError, ArtifactManifestSummaryV1, ConeKind,
    ConeSourceForm, read_artifact_manifest_summary,
};

use crate::{ArtifactCacheRoot, ArtifactSearchRoot};

mod default_core;
pub(crate) use default_core::locate_default_core;
mod default_library;
pub(crate) use default_library::locate_build_dependency;

pub(crate) fn artifact_candidate_path(root: &Path, coordinate: &ConeCoordinate) -> PathBuf {
    root.join(coordinate.group())
        .join(coordinate.name())
        .join(coordinate.version())
        .join("cone.slib")
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
    resolved_path: PathBuf,
    summary: Box<ArtifactManifestSummaryV1>,
}

impl PrebuiltArtifactCandidate {
    pub(crate) fn resolved_path(&self) -> &Path {
        &self.resolved_path
    }

    pub(crate) const fn summary(&self) -> &ArtifactManifestSummaryV1 {
        &self.summary
    }

    pub(crate) fn into_parts(self) -> (PathBuf, ArtifactManifestSummaryV1) {
        (self.resolved_path, *self.summary)
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
                .any(|existing| existing.resolved_path == candidate.resolved_path)
            {
                self.candidates.rest.push(candidate);
            }
        }
        self.candidates
            .rest
            .sort_by(|left, right| left.resolved_path.cmp(&right.resolved_path));
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
        ),
        DependencyLocator::SearchRoots => {
            locate_from_search_roots(coordinate, search_roots, target)
                .map(|prebuilt| LocatedDependencyClaim::Prebuilt(Box::new(prebuilt)))
        }
    }
}

fn locate_artifact_dependency(
    path: PathBuf,
    coordinate: &ConeCoordinate,
    target: scoop_lir::ValidatedLirTargetSelection,
) -> Result<LocatedDependencyClaim, DependencyLocatorError> {
    let candidate = probe_artifact_candidate(path, coordinate, target)?;
    Ok(LocatedDependencyClaim::Prebuilt(Box::new(
        PrebuiltArtifactProjection {
            coordinate: coordinate.clone(),
            artifact_fingerprint: candidate.summary.artifact_fingerprint(),
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
    let first = probe_artifact_candidate(first_path, coordinate, target)?;
    let expected_fingerprint = first.summary.artifact_fingerprint();
    let mut rest = Vec::new();
    for path in paths {
        rest.push(probe_artifact_candidate(path, coordinate, target)?);
    }
    if rest
        .iter()
        .any(|candidate| candidate.summary.artifact_fingerprint() != expected_fingerprint)
    {
        let candidates = std::iter::once(first)
            .chain(rest)
            .map(|candidate| {
                (
                    candidate.resolved_path,
                    candidate.summary.artifact_fingerprint(),
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
) -> Result<PrebuiltArtifactCandidate, DependencyLocatorError> {
    let resolved_path = canonicalize(&path)?;
    let mut file = File::open(&resolved_path).map_err(|source| DependencyLocatorError::Io {
        operation: LocatorIoOperation::Open,
        path: resolved_path.clone(),
        source,
    })?;
    let metadata = file
        .metadata()
        .map_err(|source| DependencyLocatorError::Io {
            operation: LocatorIoOperation::Inspect,
            path: resolved_path.clone(),
            source,
        })?;
    if !metadata.is_file() {
        return Err(DependencyLocatorError::ArtifactNotRegularFile(
            resolved_path,
        ));
    }
    let expected_length = usize::try_from(metadata.len())
        .map_err(|_| DependencyLocatorError::LengthOverflow(resolved_path.clone()))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(expected_length)
        .map_err(|_| DependencyLocatorError::Allocation(resolved_path.clone()))?;
    let read_bound = metadata
        .len()
        .checked_add(1)
        .ok_or_else(|| DependencyLocatorError::LengthOverflow(resolved_path.clone()))?;
    file.by_ref()
        .take(read_bound)
        .read_to_end(&mut bytes)
        .map_err(|source| DependencyLocatorError::Io {
            operation: LocatorIoOperation::Read,
            path: resolved_path.clone(),
            source,
        })?;
    let after = file
        .metadata()
        .map_err(|source| DependencyLocatorError::Io {
            operation: LocatorIoOperation::Inspect,
            path: resolved_path.clone(),
            source,
        })?;
    if bytes.len() != expected_length
        || after.len() != metadata.len()
        || after.modified().ok() != metadata.modified().ok()
    {
        return Err(DependencyLocatorError::ArtifactChangedDuringRead(
            resolved_path,
        ));
    }
    let summary = read_artifact_manifest_summary(&bytes, target).map_err(|source| {
        DependencyLocatorError::Summary {
            path: resolved_path.clone(),
            source,
        }
    })?;
    validate_artifact_shape(expected, &resolved_path, &summary)?;

    Ok(PrebuiltArtifactCandidate {
        resolved_path,
        summary: Box::new(summary),
    })
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocatorIoOperation {
    Inspect,
    Canonicalize,
    Open,
    Read,
}

impl fmt::Display for LocatorIoOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Inspect => "inspect",
            Self::Canonicalize => "canonicalize",
            Self::Open => "open",
            Self::Read => "read",
        })
    }
}

#[derive(Debug)]
pub enum DependencyLocatorError {
    UndeclaredDependency(DependencyCoordinateKey),
    MissingLocatorProjection(DependencyCoordinateKey),
    Manifest(ManifestRootError),
    Io {
        operation: LocatorIoOperation,
        path: PathBuf,
        source: std::io::Error,
    },
    ArtifactNotRegularFile(PathBuf),
    LengthOverflow(PathBuf),
    ArtifactChangedDuringRead(PathBuf),
    Allocation(PathBuf),
    Summary {
        path: PathBuf,
        source: ArtifactManifestSummaryError,
    },

    CoordinateMismatch {
        expected: Box<ConeCoordinate>,
        actual: Box<ConeCoordinate>,
        path: PathBuf,
    },
    ExecutableDependency {
        coordinate: ConeCoordinate,
        path: PathBuf,
    },
    InvalidArtifactSourceForm {
        coordinate: ConeCoordinate,
        path: PathBuf,
        actual: ConeSourceForm,
    },
    ReservedArtifact {
        coordinate: ConeCoordinate,
        path: PathBuf,
    },
    SelfSourceLocator {
        coordinate: ConeCoordinate,
        root: PathBuf,
    },
    ArtifactNotFound {
        coordinate: ConeCoordinate,
        checked: Vec<PathBuf>,
    },
    AmbiguousArtifact {
        coordinate: ConeCoordinate,
        candidates: Vec<(PathBuf, ArtifactFingerprint)>,
    },
}

impl fmt::Display for DependencyLocatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UndeclaredDependency(key) => {
                write!(formatter, "manifest does not declare dependency {key}")
            }
            Self::MissingLocatorProjection(key) => {
                write!(
                    formatter,
                    "manifest dependency {key} has no locator projection"
                )
            }
            Self::Manifest(error) => error.fmt(formatter),
            Self::Io {
                operation,
                path,
                source,
            } => write!(formatter, "cannot {operation} {}: {source}", path.display()),
            Self::ArtifactNotRegularFile(path) => {
                write!(
                    formatter,
                    "artifact {} is not a regular file",
                    path.display()
                )
            }
            Self::LengthOverflow(path) => write!(
                formatter,
                "artifact {} length cannot be represented",
                path.display()
            ),
            Self::ArtifactChangedDuringRead(path) => {
                write!(
                    formatter,
                    "artifact {} changed while it was read",
                    path.display()
                )
            }
            Self::Allocation(path) => {
                write!(
                    formatter,
                    "cannot allocate artifact bytes for {}",
                    path.display()
                )
            }
            Self::Summary { path, source } => {
                write!(
                    formatter,
                    "invalid artifact summary {}: {source}",
                    path.display()
                )
            }

            Self::CoordinateMismatch {
                expected,
                actual,
                path,
            } => write!(
                formatter,
                "dependency at {} declares {actual}, expected {expected}",
                path.display()
            ),
            Self::ExecutableDependency { coordinate, path } => write!(
                formatter,
                "dependency {coordinate} at {} is executable; dependencies must be libraries",
                path.display()
            ),
            Self::InvalidArtifactSourceForm {
                coordinate,
                path,
                actual,
            } => write!(
                formatter,
                "dependency {coordinate} at {} has invalid source form {actual:?}",
                path.display()
            ),
            Self::ReservedArtifact { coordinate, path } => write!(
                formatter,
                "dependency {coordinate} at {} uses a reserved artifact identity",
                path.display()
            ),
            Self::SelfSourceLocator { coordinate, root } => write!(
                formatter,
                "dependency {coordinate} resolves to its declaring source root {}",
                root.display()
            ),
            Self::ArtifactNotFound {
                coordinate,
                checked,
            } => write!(
                formatter,
                "artifact {coordinate} was not found in {} checked search roots",
                checked.len()
            ),
            Self::AmbiguousArtifact {
                coordinate,
                candidates,
            } => write!(
                formatter,
                "artifact {coordinate} has {} candidates with different fingerprints",
                candidates.len()
            ),
        }
    }
}

impl std::error::Error for DependencyLocatorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Manifest(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            Self::Summary { source, .. } => Some(source),

            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
