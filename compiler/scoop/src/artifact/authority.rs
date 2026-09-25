use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use scoop_identity::{ConeIdentity, SemanticIdentitySession};
use scoop_lir::CBridgeToolchainProfileV1;
use scoop_slib::{
    ArtifactSnapshot, CrossConeArtifactClosureInput, CrossConeArtifactClosureValidationError,
    PrebuiltManifestSummaryError, PublishableCrossConeArtifact,
    validate_cross_cone_artifact_closure,
};

use super::{ArtifactClosurePlan, ArtifactClosureValidationError};

/// Owned authority for one immutable artifact that passed the complete M23-5
/// Compile, Link, dual-view, and terminal-definition closure gate.
#[derive(Debug)]
pub struct ValidatedCrossConeArtifactHandle {
    snapshot: Arc<ArtifactSnapshot>,
    publication: PublishableCrossConeArtifact,

    c_bridge_profile: CBridgeToolchainProfileV1,
}

impl ValidatedCrossConeArtifactHandle {
    pub fn snapshot(&self) -> &Arc<ArtifactSnapshot> {
        &self.snapshot
    }

    pub const fn publication(&self) -> &PublishableCrossConeArtifact {
        &self.publication
    }

    fn new(
        snapshot: Arc<ArtifactSnapshot>,
        publication: PublishableCrossConeArtifact,

        c_bridge_profile: CBridgeToolchainProfileV1,
    ) -> Self {
        Self {
            snapshot,
            publication,

            c_bridge_profile,
        }
    }

    pub(crate) const fn c_bridge_profile(&self) -> &CBridgeToolchainProfileV1 {
        &self.c_bridge_profile
    }
}

/// Purpose-preserving projection of one cross-Cone artifact authority.
///
/// The marker prevents Compile and Link consumers from exchanging handles;
/// reopening typed views requires validating the complete closure again.
#[derive(Debug)]
pub struct CrossConePurposeArtifactHandle<P> {
    artifact: Arc<ValidatedCrossConeArtifactHandle>,
    purpose: PhantomData<fn() -> P>,
}

impl<P> Clone for CrossConePurposeArtifactHandle<P> {
    fn clone(&self) -> Self {
        Self {
            artifact: Arc::clone(&self.artifact),
            purpose: PhantomData,
        }
    }
}

impl<P> CrossConePurposeArtifactHandle<P> {
    pub(crate) fn from_validated(artifact: Arc<ValidatedCrossConeArtifactHandle>) -> Self {
        Self {
            artifact,
            purpose: PhantomData,
        }
    }

    pub fn publication(&self) -> &PublishableCrossConeArtifact {
        self.artifact.publication()
    }

    pub fn snapshot(&self) -> &Arc<ArtifactSnapshot> {
        self.artifact.snapshot()
    }

    pub(crate) fn c_bridge_profile(&self) -> &CBridgeToolchainProfileV1 {
        self.artifact.c_bridge_profile()
    }
}

impl ArtifactClosurePlan {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn validate_completed_artifact(
        &self,
        current: ConeIdentity,
        snapshot: Arc<ArtifactSnapshot>,
        completed: &BTreeMap<ConeIdentity, Arc<ValidatedCrossConeArtifactHandle>>,

        c_bridge_profile: &CBridgeToolchainProfileV1,
    ) -> Result<Arc<ValidatedCrossConeArtifactHandle>, CrossConeArtifactValidationError> {
        if !self.nodes.contains_key(&current) {
            return Err(CrossConeArtifactValidationError::Plan(Box::new(
                ArtifactClosureValidationError::UnknownRoot(current),
            )));
        }
        let reachable = self.reachable_from(current);
        let order = self.closure_order(&reachable);
        if order.is_empty() {
            return Err(CrossConeArtifactValidationError::Plan(Box::new(
                ArtifactClosureValidationError::EmptyCanonicalOrder(current),
            )));
        }
        self.validate_dependency_first(&reachable, &order)
            .map_err(|source| CrossConeArtifactValidationError::Plan(Box::new(source)))?;

        let dependency_order = order
            .iter()
            .copied()
            .filter(|identity| *identity != current)
            .collect::<Vec<_>>();
        let mut dependency_bytes = Vec::with_capacity(dependency_order.len());
        for identity in &dependency_order {
            let artifact = completed.get(identity).ok_or_else(|| {
                CrossConeArtifactValidationError::Plan(Box::new(
                    ArtifactClosureValidationError::MissingArtifact(*identity),
                ))
            })?;
            dependency_bytes.push(artifact.snapshot().as_bytes());
        }
        let direct = self.direct[&current]
            .iter()
            .map(|edge| edge.dependency)
            .collect::<Vec<_>>();

        let mut summaries = Vec::with_capacity(order.len());
        for identity in &dependency_order {
            let artifact = &completed[identity];
            summaries.push((
                *identity,
                probe_summary(*identity, artifact.snapshot(), self.target)?,
            ));
        }
        summaries.push((current, probe_summary(current, &snapshot, self.target)?));

        let mut session = SemanticIdentitySession::new();
        let closure = validate_cross_cone_artifact_closure(
            CrossConeArtifactClosureInput::completed(
                current,
                self.target,
                direct,
                dependency_bytes,
                snapshot.as_bytes(),
            ),
            c_bridge_profile,
            &mut session,
        )
        .map_err(|source| CrossConeArtifactValidationError::Closure(Box::new(source)))?;

        for (identity, summary) in summaries {
            let publication = closure.publication(identity).ok_or(
                CrossConeArtifactValidationError::MissingValidatedArtifact(identity),
            )?;
            if !summary_matches_publication(&summary, publication) {
                return Err(CrossConeArtifactValidationError::SummaryViewMismatch(
                    identity,
                ));
            }
        }

        let publication = closure.into_current_publication().ok_or(
            CrossConeArtifactValidationError::MissingValidatedArtifact(current),
        )?;
        Ok(Arc::new(ValidatedCrossConeArtifactHandle::new(
            snapshot,
            publication,
            c_bridge_profile.clone(),
        )))
    }
}

fn probe_summary(
    identity: ConeIdentity,
    snapshot: &ArtifactSnapshot,
    target: scoop_lir::ValidatedLirTargetSelection,
) -> Result<scoop_slib::PrebuiltManifestSummaryV1, CrossConeArtifactValidationError> {
    let summary = snapshot.probe_prebuilt_summary(target).map_err(|source| {
        CrossConeArtifactValidationError::Summary {
            identity,
            source: Box::new(source),
        }
    })?;

    Ok(summary)
}

fn summary_matches_publication(
    summary: &scoop_slib::PrebuiltManifestSummaryV1,
    publication: &PublishableCrossConeArtifact,
) -> bool {
    summary.cone().coordinate() == publication.coordinate()
        && summary.cone().identity() == publication.identity()
        && summary.cone().kind() == publication.kind()
        && summary.cone().source_form() == publication.source_form()
        && summary.target_selection() == publication.target_selection()
        && summary.artifact_fingerprint() == publication.artifact_fingerprint()
        && summary.profile() == publication.profile()
        && summary.direct_dependencies() == publication.direct_dependencies()
        && summary.semantic_fingerprints() == publication.compile_summary().semantic_fingerprints()
}

#[derive(Debug)]
pub enum CrossConeArtifactValidationError {
    Plan(Box<ArtifactClosureValidationError>),
    Summary {
        identity: ConeIdentity,
        source: Box<PrebuiltManifestSummaryError>,
    },

    Closure(Box<CrossConeArtifactClosureValidationError>),
    MissingValidatedArtifact(ConeIdentity),
    SummaryViewMismatch(ConeIdentity),
}

impl fmt::Display for CrossConeArtifactValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plan(source) => source.fmt(formatter),
            Self::Summary { identity, source } => {
                write!(formatter, "cannot summarize artifact {identity}: {source}")
            }

            Self::Closure(source) => source.fmt(formatter),
            Self::MissingValidatedArtifact(identity) => write!(
                formatter,
                "cross-Cone closure omitted validated artifact {identity}"
            ),
            Self::SummaryViewMismatch(identity) => write!(
                formatter,
                "artifact {identity} graph summary disagrees with its validated Compile/Link views"
            ),
        }
    }
}

impl std::error::Error for CrossConeArtifactValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Plan(source) => Some(source),
            Self::Summary { source, .. } => Some(source),

            Self::Closure(source) => Some(source),
            Self::MissingValidatedArtifact(_) | Self::SummaryViewMismatch(_) => None,
        }
    }
}
