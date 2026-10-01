use super::*;
use crate::cross_cone_closure::graph_validation::{
    CrossConeClosureArtifact, ValidatedCrossConeClosureGraph, validate_artifact_graph,
};
use crate::{ConeKind, DecodedSlibEnvelope, DependencyRecord, ValidatedGraphArtifact};
use scoop_identity::ConeCoordinate;
use std::collections::BTreeMap;

pub(super) fn read<'a>(
    root: &'a [u8],
    dependencies: &[&'a [u8]],
    target: lir::ValidatedLirTargetSelection,
) -> Result<ValidatedCrossConeClosureGraph<ValidatedGraphArtifact<'a>>, ProgramLinkReadError> {
    let decode = |bytes| {
        DecodedSlibEnvelope::open(bytes, target)
            .map_err(error)?
            .validate_graph()
            .map_err(error)
    };
    let root = decode(root)?;
    if root.kind() != ConeKind::Executable {
        return Err(error("program root must be an executable Cone"));
    }
    let mut by_identity = BTreeMap::new();
    for bytes in dependencies {
        let artifact = decode(*bytes)?;
        if artifact.identity() == root.identity() {
            if artifact.artifact_fingerprint() != root.artifact_fingerprint() {
                return Err(error("root Cone has conflicting artifact contents"));
            }
            continue;
        }
        match by_identity.entry(artifact.identity()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(artifact);
            }
            std::collections::btree_map::Entry::Occupied(entry) => {
                if entry.get().artifact_fingerprint() != artifact.artifact_fingerprint() {
                    return Err(error(format!(
                        "Cone {} has conflicting artifact contents",
                        artifact.coordinate()
                    )));
                }
            }
        }
    }
    let mut pending = by_identity
        .into_values()
        .map(|artifact| {
            let coordinate = artifact.coordinate();
            let key = (
                coordinate.group().to_owned(),
                coordinate.name().to_owned(),
                coordinate.version().to_owned(),
            );
            (key, artifact)
        })
        .collect::<BTreeMap<_, _>>();
    let mut ready = std::collections::BTreeSet::new();
    let mut ordered = Vec::with_capacity(pending.len());
    while !pending.is_empty() {
        let key = pending
            .iter()
            .find(|(_, artifact)| {
                artifact
                    .direct_dependencies()
                    .iter()
                    .all(|dependency| ready.contains(&dependency.identity()))
            })
            .map(|(key, _)| key.clone())
            .ok_or_else(|| error("dependency graph has a cycle or a missing provider"))?;
        let artifact = pending.remove(&key).expect("selected pending artifact");
        ready.insert(artifact.identity());
        ordered.push(artifact);
    }
    validate_artifact_graph(
        root.identity(),
        target,
        root.direct_dependencies()
            .iter()
            .map(DependencyRecord::identity)
            .collect(),
        ordered,
        Some(root),
    )
    .map_err(error)
}

impl CrossConeClosureArtifact for ValidatedGraphArtifact<'_> {
    fn coordinate(&self) -> &ConeCoordinate {
        self.coordinate()
    }
    fn identity(&self) -> ConeIdentity {
        self.identity()
    }
    fn kind(&self) -> ConeKind {
        self.kind()
    }
    fn target_selection(&self) -> lir::ValidatedLirTargetSelection {
        self.target_selection()
    }
    fn direct_dependencies(&self) -> &[DependencyRecord] {
        self.direct_dependencies()
    }
    fn dependency_record(&self) -> DependencyRecord {
        let semantic = self.envelope.manifest().semantic_fingerprints();
        DependencyRecord::from_validated(
            self.coordinate().clone(),
            self.identity(),
            semantic.hir(),
            semantic.mir(),
            semantic.lir(),
        )
    }
}
