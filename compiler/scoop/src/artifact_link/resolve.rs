use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_slib::{ArtifactManifestSummaryError, ArtifactSnapshot, SlibDiagnostic};

use super::{LinkRequest, LocatedLinkArtifact, failure};
use crate::{BuildResult, ImmutableInputSnapshot};

pub(super) fn artifacts(
    request: &LinkRequest,
) -> BuildResult<(LocatedLinkArtifact, Vec<LocatedLinkArtifact>)> {
    let root = read(&request.root_slib)?;
    let root_id = root.summary.cone().identity();
    let mut artifacts = BTreeMap::new();
    for path in &request.dependency_slibs {
        let artifact = read(path)?;
        let identity = artifact.summary.cone().identity();
        if identity == root_id {
            return Err(failure("root artifact cannot also be a dependency")
                .at_artifact(&artifact.path, "manifest:cone.identity"));
        }
        insert(&mut artifacts, artifact)?;
    }
    let mut pending = root
        .summary
        .direct_dependencies()
        .iter()
        .map(|edge| (root.path.clone(), edge.clone()))
        .collect::<Vec<_>>();
    let mut visited = BTreeSet::from([root_id]);
    while let Some((dependent, edge)) = pending.pop() {
        if !visited.insert(edge.identity()) {
            continue;
        }
        if !artifacts.contains_key(&edge.identity()) {
            let artifact = locate(request, edge.coordinate()).map_err(|error| {
                if !matches!(error.location, crate::BuildFailureLocation::None) {
                    error
                } else {
                    error.at_artifact(&dependent, "manifest:direct_dependencies")
                }
            })?;
            insert(&mut artifacts, artifact)?;
        }
        let artifact = &artifacts[&edge.identity()];
        pending.extend(
            artifact
                .summary
                .direct_dependencies()
                .iter()
                .map(|edge| (artifact.path.clone(), edge.clone())),
        );
    }
    Ok((root, artifacts.into_values().collect()))
}

fn read(path: &Path) -> BuildResult<LocatedLinkArtifact> {
    let input = ImmutableInputSnapshot::capture(path)
        .map_err(|error| failure(error).at_artifact(path, "container:$"))?;
    let snapshot = Arc::new(ArtifactSnapshot::from_shared(input.shared_bytes()));
    let summary = snapshot
        .manifest_summary(ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1)
        .map_err(|error| {
            let member = match &error {
                ArtifactManifestSummaryError::Envelope(error) => error.diagnostic().semantic_path(),
                ArtifactManifestSummaryError::Graph(error) => error.diagnostic().semantic_path(),
                ArtifactManifestSummaryError::LengthOverflow => "container:$".to_owned(),
            };
            failure(error).at_artifact(path, member)
        })?;
    Ok(LocatedLinkArtifact {
        path: input.source_locator().to_owned(),
        snapshot,
        summary,
    })
}

fn insert(
    artifacts: &mut BTreeMap<ConeIdentity, LocatedLinkArtifact>,
    artifact: LocatedLinkArtifact,
) -> BuildResult<()> {
    let identity = artifact.summary.cone().identity();
    if let Some(previous) = artifacts.get(&identity) {
        if previous.snapshot.as_bytes() != artifact.snapshot.as_bytes() {
            return Err(failure(format!(
                "different complete artifacts for {identity}: {} and {}",
                previous.path.display(),
                artifact.path.display()
            ))
            .at_artifact(&artifact.path, "manifest:cone.identity"));
        }
        if previous.path <= artifact.path {
            return Ok(());
        }
    }
    artifacts.insert(identity, artifact);
    Ok(())
}

fn locate(request: &LinkRequest, coordinate: &ConeCoordinate) -> BuildResult<LocatedLinkArtifact> {
    let mut paths = request
        .cone_paths
        .iter()
        .map(|root| crate::locator::artifact_candidate_path(root, coordinate))
        .collect::<Vec<_>>();
    if coordinate == &ConeCoordinate::reserved_core() {
        paths.push(
            scoop_toolchain::TrustedCoreSlotLayoutV1::new(
                &request.sysroot,
                ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            )
            .artifact()
            .to_owned(),
        );
    }
    paths.sort();
    paths.dedup();
    let mut candidates = BTreeMap::new();
    for path in paths {
        match std::fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(failure(format!("artifact {}: {error}", path.display()))
                    .at_artifact(&path, "container:$"));
            }
            Ok(_) => {}
        }
        let artifact = read(&path)?;
        if artifact.summary.cone().coordinate() != coordinate {
            return Err(failure(format!(
                "artifact {} has coordinate {}, expected {coordinate}",
                path.display(),
                artifact.summary.cone().coordinate()
            ))
            .at_artifact(&path, "manifest:cone.coordinate"));
        }
        insert(&mut candidates, artifact)?;
    }
    candidates.into_values().next().ok_or_else(|| {
        failure(format!(
            "artifact not found for {coordinate}; provide --dependency-slib or --cone-path"
        ))
    })
}
