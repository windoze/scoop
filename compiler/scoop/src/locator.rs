use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use scoop_identity::{ConeCoordinate, ConeIdentity, RequestedConeKind};
use scoop_manifest::{
    DependencyCoordinateKey, DependencyLocator, LoadedConeManifest, ManifestRootError,
    ManifestRootLocator, load_cone_manifest,
};
use scoop_slib::{
    ArtifactFingerprint, ConeKind, ConeSourceForm, PrebuiltManifestSummaryError,
    PrebuiltManifestSummaryV1, probe_prebuilt_manifest_summary,
};

use crate::ArtifactSearchRoot;

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
    summary: Box<PrebuiltManifestSummaryV1>,
}

impl PrebuiltArtifactCandidate {
    pub(crate) fn resolved_path(&self) -> &Path {
        &self.resolved_path
    }

    pub(crate) const fn summary(&self) -> &PrebuiltManifestSummaryV1 {
        &self.summary
    }

    pub(crate) fn into_parts(self) -> (PathBuf, PrebuiltManifestSummaryV1) {
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
) -> Result<LocatedDependencyClaim, DependencyLocatorError> {
    let coordinate = parent
        .parsed()
        .semantic()
        .dependency_iter()
        .find(|(candidate, _)| *candidate == key)
        .map(|(_, coordinate)| coordinate)
        .ok_or_else(|| DependencyLocatorError::UndeclaredDependency(key.clone()))?;
    let locator = parent
        .parsed()
        .locators()
        .get(key)
        .ok_or_else(|| DependencyLocatorError::MissingLocatorProjection(key.clone()))?;

    match locator {
        DependencyLocator::SourcePath(path) => locate_source_dependency(parent, coordinate, {
            resolve_from_manifest(parent, path.as_path())
        })
        .map(|source| LocatedDependencyClaim::Source(Box::new(source))),
        DependencyLocator::ArtifactPath(path) => {
            let candidate = probe_artifact_candidate(
                resolve_from_manifest(parent, path.as_path()),
                coordinate,
                target,
            )?;
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
        DependencyLocator::SearchRoots => {
            locate_from_search_roots(coordinate, search_roots, target)
                .map(|prebuilt| LocatedDependencyClaim::Prebuilt(Box::new(prebuilt)))
        }
    }
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
) -> Result<ManifestSourceProjection, DependencyLocatorError> {
    let manifest = load_dependency_manifest(expected, path)?;
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
) -> Result<LoadedConeManifest, DependencyLocatorError> {
    let locator = ManifestRootLocator::from_path(path).map_err(DependencyLocatorError::Manifest)?;
    let manifest = load_cone_manifest(&locator).map_err(DependencyLocatorError::Manifest)?;
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
        let candidate = root
            .as_path()
            .join(coordinate.group())
            .join(coordinate.name())
            .join(coordinate.version())
            .join("cone.slib");

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
    let summary = probe_prebuilt_manifest_summary(&bytes, target).map_err(|source| {
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
    summary: &PrebuiltManifestSummaryV1,
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
        source: PrebuiltManifestSummaryError,
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
mod tests {
    use super::*;
    use scoop_hir::CanonicalHirFoundation;
    use scoop_lir::{CanonicalLirFoundation, ValidatedLirTargetSelection};
    use scoop_manifest::load_cone_manifest;
    use scoop_mir::CanonicalMirFoundation;
    use scoop_slib::{
        ConeRecord, IdentityFoundationArtifact, IdentityFoundationArtifactInput, ProducerRecord,
    };

    const TARGET: ValidatedLirTargetSelection =
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;

    fn write_manifest(path: &Path, name: &str, kind: &str, dependency: &str) {
        std::fs::create_dir_all(path).unwrap();
        std::fs::write(
            path.join("Cone.toml"),
            format!(
                "schema = 1\n[cone]\ngroup = \"test\"\nname = \"{name}\"\nversion = \"1.0.0\"\nkind = \"{kind}\"\n{dependency}"
            ),
        )
        .unwrap();
    }

    fn parent_with_dependency(root: &Path, dependency: &str) -> LoadedConeManifest {
        write_manifest(root, "root", "library", dependency);
        load_cone_manifest(&ManifestRootLocator::cone_directory(root)).unwrap()
    }

    fn first_key(manifest: &LoadedConeManifest) -> DependencyCoordinateKey {
        manifest
            .parsed()
            .semantic()
            .dependency_iter()
            .next()
            .unwrap()
            .0
            .clone()
    }

    fn foundation_artifact(coordinate: ConeCoordinate, producer: &str) -> Vec<u8> {
        let hir = CanonicalHirFoundation::empty();
        let mir = CanonicalMirFoundation::empty();
        let lir = CanonicalLirFoundation::empty();
        let cone =
            ConeRecord::new(coordinate, ConeKind::Library, ConeSourceForm::Manifest).unwrap();
        IdentityFoundationArtifact::write(IdentityFoundationArtifactInput::new(
            ProducerRecord::new(producer).unwrap(),
            cone,
            TARGET,
            &hir,
            &mir,
            &lir,
        ))
        .unwrap()
        .as_bytes()
        .to_vec()
    }

    #[test]
    fn source_path_resolves_from_the_real_declaring_root() {
        let temp = tempfile::tempdir().unwrap();
        let parent_root = temp.path().join("parent");
        let dependency_root = temp.path().join("dependency");
        write_manifest(&dependency_root, "dep", "library", "");
        let parent = parent_with_dependency(
            &parent_root,
            "[dependencies]\n\"test:dep\" = { version = \"1.0.0\", path = \"../dependency\" }\n",
        );

        let claim = locate_manifest_dependency(&parent, &first_key(&parent), &[], TARGET).unwrap();
        let LocatedDependencyClaim::Source(source) = claim else {
            panic!("source locator must produce a source claim")
        };
        assert_eq!(source.manifest().coordinate().name(), "dep");
        assert_eq!(
            source.manifest().real_root(),
            std::fs::canonicalize(dependency_root).unwrap()
        );
    }

    #[test]
    fn source_coordinate_and_kind_are_checked_at_the_locator_boundary() {
        let temp = tempfile::tempdir().unwrap();
        let parent_root = temp.path().join("parent");
        let dependency_root = temp.path().join("dependency");
        write_manifest(&dependency_root, "other", "library", "");
        let parent = parent_with_dependency(
            &parent_root,
            "[dependencies]\n\"test:dep\" = { version = \"1.0.0\", path = \"../dependency\" }\n",
        );
        assert!(matches!(
            locate_manifest_dependency(&parent, &first_key(&parent), &[], TARGET,),
            Err(DependencyLocatorError::CoordinateMismatch { .. })
        ));

        write_manifest(&dependency_root, "dep", "executable", "");
        assert!(matches!(
            locate_manifest_dependency(&parent, &first_key(&parent), &[], TARGET,),
            Err(DependencyLocatorError::ExecutableDependency { .. })
        ));
    }

    #[test]
    fn explicit_artifact_is_probed_without_becoming_a_compile_or_link_proof() {
        let temp = tempfile::tempdir().unwrap();
        let parent_root = temp.path().join("parent");
        let artifact = temp.path().join("dep.slib");
        let coordinate = ConeCoordinate::new("test", "dep", "1.0.0").unwrap();
        let bytes = foundation_artifact(coordinate.clone(), "locator-test");
        std::fs::write(&artifact, &bytes).unwrap();
        let parent = parent_with_dependency(
            &parent_root,
            "[dependencies]\n\"test:dep\" = { version = \"1.0.0\", artifact = \"../dep.slib\" }\n",
        );

        let claim = locate_manifest_dependency(&parent, &first_key(&parent), &[], TARGET).unwrap();
        let LocatedDependencyClaim::Prebuilt(prebuilt) = claim else {
            panic!("artifact locator must produce a prebuilt claim")
        };
        assert_eq!(prebuilt.coordinate(), &coordinate);
        assert_eq!(prebuilt.candidate_count(), 1);
        assert_eq!(
            prebuilt.artifact_fingerprint(),
            prebuilt.first_candidate().summary().artifact_fingerprint()
        );

        std::fs::write(
            &artifact,
            foundation_artifact(
                ConeCoordinate::new("test", "other", "1.0.0").unwrap(),
                "wrong-coordinate",
            ),
        )
        .unwrap();
        assert!(matches!(
            locate_manifest_dependency(&parent, &first_key(&parent), &[], TARGET,),
            Err(DependencyLocatorError::CoordinateMismatch { .. })
        ));

        std::fs::write(&artifact, &bytes).unwrap();
    }

    #[test]
    fn search_roots_use_the_exact_unsplit_coordinate_layout() {
        let temp = tempfile::tempdir().unwrap();
        let parent_root = temp.path().join("parent");
        let search_root = temp.path().join("search");
        let coordinate = ConeCoordinate::new("test.group", "dep.name", "1.0.0").unwrap();
        let artifact = search_root
            .join("test.group")
            .join("dep.name")
            .join("1.0.0")
            .join("cone.slib");
        std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        std::fs::write(&artifact, foundation_artifact(coordinate, "locator-test")).unwrap();
        let parent = parent_with_dependency(
            &parent_root,
            "[dependencies]\n\"test.group:dep.name\" = \"1.0.0\"\n",
        );
        let roots = [ArtifactSearchRoot::new(&search_root).unwrap()];

        let claim =
            locate_manifest_dependency(&parent, &first_key(&parent), &roots, TARGET).unwrap();
        let LocatedDependencyClaim::Prebuilt(prebuilt) = claim else {
            panic!("search root must produce a prebuilt claim")
        };
        assert_eq!(
            prebuilt.first_candidate().resolved_path(),
            std::fs::canonicalize(artifact).unwrap()
        );
    }

    #[test]
    fn absent_dangling_and_wrong_type_candidates_are_distinct() {
        let temp = tempfile::tempdir().unwrap();
        let parent_root = temp.path().join("parent");
        let parent =
            parent_with_dependency(&parent_root, "[dependencies]\n\"test:dep\" = \"1.0.0\"\n");
        let search_root = temp.path().join("search");
        let roots = [ArtifactSearchRoot::new(&search_root).unwrap()];
        assert!(matches!(
            locate_manifest_dependency(&parent, &first_key(&parent), &roots, TARGET,),
            Err(DependencyLocatorError::ArtifactNotFound { .. })
        ));

        let candidate = search_root
            .join("test")
            .join("dep")
            .join("1.0.0")
            .join("cone.slib");
        std::fs::create_dir_all(candidate.parent().unwrap()).unwrap();
        std::fs::create_dir(&candidate).unwrap();
        assert!(matches!(
            locate_manifest_dependency(&parent, &first_key(&parent), &roots, TARGET,),
            Err(DependencyLocatorError::ArtifactNotRegularFile(_))
        ));

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let dangling_root = temp.path().join("dangling-search");
            let dangling = dangling_root
                .join("test")
                .join("dep")
                .join("1.0.0")
                .join("cone.slib");
            std::fs::create_dir_all(dangling.parent().unwrap()).unwrap();
            symlink(dangling_root.join("missing.slib"), &dangling).unwrap();
            let dangling_roots = [ArtifactSearchRoot::new(dangling_root).unwrap()];
            assert!(matches!(
                locate_manifest_dependency(&parent, &first_key(&parent), &dangling_roots, TARGET,),
                Err(DependencyLocatorError::Io {
                    operation: LocatorIoOperation::Canonicalize,
                    ..
                })
            ));
        }
    }

    #[test]
    fn search_root_candidates_require_one_complete_artifact_fingerprint() {
        let temp = tempfile::tempdir().unwrap();
        let parent_root = temp.path().join("parent");
        let parent =
            parent_with_dependency(&parent_root, "[dependencies]\n\"test:dep\" = \"1.0.0\"\n");
        let coordinate = ConeCoordinate::new("test", "dep", "1.0.0").unwrap();
        let roots = [temp.path().join("first"), temp.path().join("second")];
        let paths = roots.clone().map(|root| {
            root.join("test")
                .join("dep")
                .join("1.0.0")
                .join("cone.slib")
        });
        let bytes = foundation_artifact(coordinate.clone(), "same");
        for path in &paths {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, &bytes).unwrap();
        }
        let search_roots = roots.map(ArtifactSearchRoot::new).map(Result::unwrap);
        let LocatedDependencyClaim::Prebuilt(prebuilt) =
            locate_manifest_dependency(&parent, &first_key(&parent), &search_roots, TARGET)
                .unwrap()
        else {
            panic!("search root must produce a prebuilt claim")
        };
        assert_eq!(prebuilt.candidate_count(), 2);

        std::fs::write(&paths[1], foundation_artifact(coordinate, "different")).unwrap();
        assert!(matches!(
            locate_manifest_dependency(&parent, &first_key(&parent), &search_roots, TARGET,),
            Err(DependencyLocatorError::AmbiguousArtifact { .. })
        ));
    }
}
