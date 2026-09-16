//! Purpose-preserving artifact closure authority for an exact resolved graph.

mod completion;
mod error;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use scoop_identity::{ArtifactCapabilityProfileId, ConeCoordinate, ConeIdentity};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_slib::{
    ArtifactFingerprint, CompileArtifactPurpose, ConeKind, ConeSourceForm, DependencyRecord,
    DualValidatedArtifactHandle, DualValidatedArtifactReopenError, LinkArtifactPurpose,
    PurposeArtifactHandle, SingleConeStrongProfile, ValidatedCompileArtifact,
    ValidatedSingleConeStrongLinkArtifact,
};

pub use completion::{
    CompiledCompletionError, CompletedNode, CompletedNodeOrigin, PrebuiltCompletionError,
    PrivateArtifactPath, TrustedCoreCompletionError, TrustedCoreReceiptBindingField,
};
pub(crate) use completion::{
    complete_compiled_candidate, complete_prebuilt_candidates, complete_trusted_core_candidate,
};
pub use error::{ArtifactClosureValidationError, ArtifactPlanField};

#[derive(Clone, Debug)]
pub(crate) struct PlannedArtifactNode {
    coordinate: ConeCoordinate,
    kind: ConeKind,
    source_form: ConeSourceForm,
    expected_artifact: Option<ArtifactFingerprint>,
}

impl PlannedArtifactNode {
    pub(crate) const fn new(
        coordinate: ConeCoordinate,
        kind: ConeKind,
        source_form: ConeSourceForm,
        expected_artifact: Option<ArtifactFingerprint>,
    ) -> Self {
        Self {
            coordinate,
            kind,
            source_form,
            expected_artifact,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PlannedArtifactEdge {
    dependent: ConeIdentity,
    dependency: ConeIdentity,
    expected_semantic: Option<DependencyRecord>,
}

impl PlannedArtifactEdge {
    pub(crate) const fn new(
        dependent: ConeIdentity,
        dependency: ConeIdentity,
        expected_semantic: Option<DependencyRecord>,
    ) -> Self {
        Self {
            dependent,
            dependency,
            expected_semantic,
        }
    }
}

/// Immutable projection of the exact resolved graph used to validate artifact
/// closures. It contains no artifact bytes and grants no Compile or Link
/// authority by itself.
#[derive(Clone, Debug)]
pub struct ArtifactClosurePlan {
    root: ConeIdentity,
    target: ValidatedLirTargetSelection,
    dependency_first: Vec<ConeIdentity>,
    nodes: BTreeMap<ConeIdentity, PlannedArtifactNode>,
    direct: BTreeMap<ConeIdentity, Vec<PlannedArtifactEdge>>,
}

impl ArtifactClosurePlan {
    pub(crate) fn new(
        root: ConeIdentity,
        target: ValidatedLirTargetSelection,
        dependency_first: Vec<ConeIdentity>,
        nodes: BTreeMap<ConeIdentity, PlannedArtifactNode>,
        edges: impl IntoIterator<Item = PlannedArtifactEdge>,
    ) -> Self {
        let mut direct: BTreeMap<_, Vec<_>> = nodes
            .keys()
            .copied()
            .map(|identity| (identity, Vec::new()))
            .collect();
        for edge in edges {
            direct.entry(edge.dependent).or_default().push(edge);
        }
        for edges in direct.values_mut() {
            edges.sort_by_key(|edge| edge.dependency);
        }
        Self {
            root,
            target,
            dependency_first,
            nodes,
            direct,
        }
    }

    pub const fn graph_root(&self) -> ConeIdentity {
        self.root
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target
    }

    /// Validates one completed root and its transitive dependency artifacts,
    /// then projects two independently typed closures from the dual handles.
    pub fn validate(
        &self,
        root: ConeIdentity,
        artifacts: &BTreeMap<ConeIdentity, Arc<DualValidatedArtifactHandle>>,
    ) -> Result<ValidatedDualArtifactClosure, ArtifactClosureValidationError> {
        if !self.nodes.contains_key(&root) {
            return Err(ArtifactClosureValidationError::UnknownRoot(root));
        }
        let reachable = self.reachable_from(root);
        for identity in &reachable {
            let artifact = artifacts
                .get(identity)
                .ok_or(ArtifactClosureValidationError::MissingArtifact(*identity))?;
            self.validate_node(*identity, artifact, artifacts)?;
        }
        self.validate_versions(&reachable, artifacts)?;
        let order = self.closure_order(&reachable);
        if order.is_empty() {
            return Err(ArtifactClosureValidationError::EmptyCanonicalOrder(root));
        }
        self.validate_dependency_first(&reachable, &order)?;

        let mut compile = BTreeMap::new();
        let mut link = BTreeMap::new();
        for identity in &order {
            let artifact = Arc::clone(&artifacts[identity]);
            compile.insert(
                *identity,
                PurposeArtifactHandle::<CompileArtifactPurpose>::from_dual(Arc::clone(&artifact)),
            );
            link.insert(
                *identity,
                PurposeArtifactHandle::<LinkArtifactPurpose>::from_dual(artifact),
            );
        }
        let edges = self.validated_edges(&reachable);
        Ok(ValidatedDualArtifactClosure {
            compile: ValidatedArtifactClosure {
                root,
                order: order.clone(),
                artifacts: compile,
                edges: edges.clone(),
            },
            link: ValidatedArtifactClosure {
                root,
                order,
                artifacts: link,
                edges,
            },
        })
    }

    fn reachable_from(&self, root: ConeIdentity) -> BTreeSet<ConeIdentity> {
        let mut reachable = BTreeSet::new();
        let mut pending = vec![root];
        while let Some(identity) = pending.pop() {
            if !reachable.insert(identity) {
                continue;
            }
            if let Some(edges) = self.direct.get(&identity) {
                pending.extend(edges.iter().rev().map(|edge| edge.dependency));
            }
        }
        reachable
    }

    fn closure_order(&self, reachable: &BTreeSet<ConeIdentity>) -> Vec<ConeIdentity> {
        self.dependency_first
            .iter()
            .filter(|identity| reachable.contains(identity))
            .copied()
            .collect()
    }

    fn validate_node(
        &self,
        identity: ConeIdentity,
        artifact: &DualValidatedArtifactHandle,
        artifacts: &BTreeMap<ConeIdentity, Arc<DualValidatedArtifactHandle>>,
    ) -> Result<(), ArtifactClosureValidationError> {
        self.validate_artifact_shape(identity, artifact)?;
        self.validate_dependencies(
            identity,
            artifact.publication().direct_dependencies(),
            artifacts,
        )
    }

    pub(crate) fn validate_artifact_shape(
        &self,
        identity: ConeIdentity,
        artifact: &DualValidatedArtifactHandle,
    ) -> Result<(), ArtifactClosureValidationError> {
        let plan = &self.nodes[&identity];
        let actual = artifact.publication();
        if actual.identity() != identity {
            return Err(ArtifactClosureValidationError::IdentityMismatch {
                planned: identity,
                actual: actual.identity(),
            });
        }
        if actual.coordinate() != &plan.coordinate {
            return Err(ArtifactClosureValidationError::CoordinateMismatch {
                identity,
                expected: Box::new(plan.coordinate.clone()),
                actual: Box::new(actual.coordinate().clone()),
            });
        }
        if actual.kind() != plan.kind {
            return Err(ArtifactClosureValidationError::KindMismatch {
                identity,
                expected: plan.kind,
                actual: actual.kind(),
            });
        }
        if actual.source_form() != plan.source_form {
            return Err(ArtifactClosureValidationError::SourceFormMismatch {
                identity,
                expected: plan.source_form,
                actual: actual.source_form(),
            });
        }
        if actual.target_selection() != self.target {
            return Err(ArtifactClosureValidationError::TargetMismatch {
                identity,
                expected: self.target,
                actual: actual.target_selection(),
            });
        }
        let expected_profile = ArtifactCapabilityProfileId::single_cone_strong();
        if actual.profile() != &expected_profile {
            return Err(ArtifactClosureValidationError::ProfileMismatch {
                identity,
                expected: Box::new(expected_profile),
                actual: Box::new(actual.profile().clone()),
            });
        }
        if let Some(expected) = plan.expected_artifact
            && actual.artifact_fingerprint() != expected
        {
            return Err(
                ArtifactClosureValidationError::ArtifactFingerprintMismatch {
                    identity,
                    expected,
                    actual: actual.artifact_fingerprint(),
                },
            );
        }
        Ok(())
    }

    fn validate_dependencies(
        &self,
        dependent: ConeIdentity,
        records: &[DependencyRecord],
        artifacts: &BTreeMap<ConeIdentity, Arc<DualValidatedArtifactHandle>>,
    ) -> Result<(), ArtifactClosureValidationError> {
        let expected = &self.direct[&dependent];
        let expected_by_identity: BTreeMap<_, _> = expected
            .iter()
            .map(|edge| (edge.dependency, edge))
            .collect();
        let mut actual_by_identity = BTreeMap::new();
        for record in records {
            if actual_by_identity
                .insert(record.identity(), record)
                .is_some()
            {
                return Err(ArtifactClosureValidationError::DuplicateDependencyRecord {
                    dependent,
                    dependency: record.identity(),
                });
            }
            if !expected_by_identity.contains_key(&record.identity()) {
                return Err(
                    ArtifactClosureValidationError::UnexpectedArtifactDependency {
                        dependent,
                        dependency: record.identity(),
                    },
                );
            }
        }
        for edge in expected {
            let record = actual_by_identity.get(&edge.dependency).copied().ok_or(
                ArtifactClosureValidationError::MissingDependencyRecord {
                    dependent,
                    dependency: edge.dependency,
                },
            )?;
            let dependency_plan = &self.nodes[&edge.dependency];
            if record.coordinate() != &dependency_plan.coordinate {
                return Err(
                    ArtifactClosureValidationError::DependencyCoordinateMismatch {
                        dependent,
                        dependency: edge.dependency,
                        expected: Box::new(dependency_plan.coordinate.clone()),
                        actual: Box::new(record.coordinate().clone()),
                    },
                );
            }
            if let Some(expected_semantic) = &edge.expected_semantic
                && record != expected_semantic
            {
                return Err(ArtifactClosureValidationError::PlannedDependencyChanged {
                    dependent,
                    dependency: edge.dependency,
                    expected: Box::new(expected_semantic.clone()),
                    actual: Box::new(record.clone()),
                });
            }
            let completed = artifacts.get(&edge.dependency).ok_or(
                ArtifactClosureValidationError::MissingArtifact(edge.dependency),
            )?;
            let completed_record = completed.publication().dependency_record();
            if record != &completed_record {
                return Err(ArtifactClosureValidationError::StaleDependency {
                    dependent,
                    dependency: edge.dependency,
                    recorded: Box::new(record.clone()),
                    completed: Box::new(completed_record),
                });
            }
        }
        Ok(())
    }

    fn validate_versions(
        &self,
        reachable: &BTreeSet<ConeIdentity>,
        artifacts: &BTreeMap<ConeIdentity, Arc<DualValidatedArtifactHandle>>,
    ) -> Result<(), ArtifactClosureValidationError> {
        let mut versions = BTreeMap::<(String, String), String>::new();
        for identity in reachable {
            let coordinate = artifacts[identity].publication().coordinate();
            let key = (coordinate.group().to_owned(), coordinate.name().to_owned());
            if let Some(first) = versions.get(&key)
                && first != coordinate.version()
            {
                return Err(ArtifactClosureValidationError::MultipleVersions {
                    group: key.0,
                    name: key.1,
                    first: first.clone(),
                    second: coordinate.version().to_owned(),
                });
            }
            versions.insert(key, coordinate.version().to_owned());
        }
        Ok(())
    }

    fn validate_dependency_first(
        &self,
        reachable: &BTreeSet<ConeIdentity>,
        order: &[ConeIdentity],
    ) -> Result<(), ArtifactClosureValidationError> {
        let positions: BTreeMap<_, _> = order
            .iter()
            .enumerate()
            .map(|(position, identity)| (*identity, position))
            .collect();
        for dependent in reachable {
            for edge in &self.direct[dependent] {
                if positions[&edge.dependency] >= positions[dependent] {
                    return Err(ArtifactClosureValidationError::InvalidCanonicalOrder {
                        dependent: *dependent,
                        dependency: edge.dependency,
                    });
                }
            }
        }
        Ok(())
    }

    fn validated_edges(
        &self,
        reachable: &BTreeSet<ConeIdentity>,
    ) -> BTreeSet<ValidatedDependencyEdge> {
        reachable
            .iter()
            .flat_map(|dependent| {
                self.direct[dependent]
                    .iter()
                    .map(|edge| ValidatedDependencyEdge {
                        dependent: *dependent,
                        dependency: edge.dependency,
                    })
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ValidatedDependencyEdge {
    dependent: ConeIdentity,
    dependency: ConeIdentity,
}

impl ValidatedDependencyEdge {
    pub const fn dependent(self) -> ConeIdentity {
        self.dependent
    }

    pub const fn dependency(self) -> ConeIdentity {
        self.dependency
    }
}

/// A non-empty, dependency-first closure whose handles retain their purpose in
/// the Rust type. It has no public constructor.
#[derive(Clone, Debug)]
pub struct ValidatedArtifactClosure<P> {
    root: ConeIdentity,
    order: Vec<ConeIdentity>,
    artifacts: BTreeMap<ConeIdentity, PurposeArtifactHandle<P>>,
    edges: BTreeSet<ValidatedDependencyEdge>,
}

impl<P> ValidatedArtifactClosure<P> {
    pub const fn root(&self) -> ConeIdentity {
        self.root
    }

    pub fn dependency_first(&self) -> &[ConeIdentity] {
        &self.order
    }

    pub fn artifact(&self, identity: ConeIdentity) -> Option<&PurposeArtifactHandle<P>> {
        self.artifacts.get(&identity)
    }

    pub fn artifacts(
        &self,
    ) -> impl ExactSizeIterator<Item = (ConeIdentity, &PurposeArtifactHandle<P>)> {
        self.order
            .iter()
            .map(|identity| (*identity, &self.artifacts[identity]))
    }

    pub fn edges(&self) -> impl ExactSizeIterator<Item = ValidatedDependencyEdge> + '_ {
        self.edges.iter().copied()
    }
}

impl ValidatedArtifactClosure<CompileArtifactPurpose> {
    pub fn with_view<R>(
        &self,
        identity: ConeIdentity,
        use_view: impl for<'view> FnOnce(&ValidatedCompileArtifact<'view, SingleConeStrongProfile>) -> R,
    ) -> Result<Option<R>, DualValidatedArtifactReopenError> {
        self.artifact(identity)
            .map(|artifact| artifact.with_view(use_view))
            .transpose()
    }
}

impl ValidatedArtifactClosure<LinkArtifactPurpose> {
    pub fn with_view<R>(
        &self,
        identity: ConeIdentity,
        use_view: impl for<'view> FnOnce(&ValidatedSingleConeStrongLinkArtifact<'view>) -> R,
    ) -> Result<Option<R>, DualValidatedArtifactReopenError> {
        self.artifact(identity)
            .map(|artifact| artifact.with_view(use_view))
            .transpose()
    }
}

#[derive(Clone, Debug)]
pub struct ValidatedDualArtifactClosure {
    compile: ValidatedArtifactClosure<CompileArtifactPurpose>,
    link: ValidatedArtifactClosure<LinkArtifactPurpose>,
}

impl ValidatedDualArtifactClosure {
    pub const fn compile(&self) -> &ValidatedArtifactClosure<CompileArtifactPurpose> {
        &self.compile
    }

    pub const fn link(&self) -> &ValidatedArtifactClosure<LinkArtifactPurpose> {
        &self.link
    }

    pub fn into_parts(
        self,
    ) -> (
        ValidatedArtifactClosure<CompileArtifactPurpose>,
        ValidatedArtifactClosure<LinkArtifactPurpose>,
    ) {
        (self.compile, self.link)
    }
}

#[cfg(test)]
mod tests;
