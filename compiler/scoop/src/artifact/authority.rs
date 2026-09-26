use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

use scoop_identity::{ConeIdentity, SemanticIdentitySession};
use scoop_lir::CBridgeToolchainProfileV1;
use scoop_slib::{
    ArtifactSnapshot, CrossConeArtifactClosureValidationError, CrossConeArtifactSummary,
    CrossConeLayoutStrongProfile, ValidatedCompileArtifact, ValidatedCrossConeStrongLinkArtifact,
    validate_completed_cross_cone_artifact_closure,
};

use super::{ArtifactClosurePlan, ArtifactClosureValidationError};

/// Complete data for one immutable artifact, retained for subsequent consumers.
pub struct ValidatedCrossConeArtifactHandle {
    snapshot: Arc<ArtifactSnapshot>,
    compile: ValidatedCompileArtifact<CrossConeLayoutStrongProfile>,
    link: ValidatedCrossConeStrongLinkArtifact,
    publication: CrossConeArtifactSummary,
}

impl fmt::Debug for ValidatedCrossConeArtifactHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValidatedCrossConeArtifactHandle")
            .field("snapshot", &self.snapshot)
            .field("publication", &self.publication)
            .finish_non_exhaustive()
    }
}

impl ValidatedCrossConeArtifactHandle {
    pub fn snapshot(&self) -> &Arc<ArtifactSnapshot> {
        &self.snapshot
    }

    pub const fn compile(&self) -> &ValidatedCompileArtifact<CrossConeLayoutStrongProfile> {
        &self.compile
    }

    pub const fn link(&self) -> &ValidatedCrossConeStrongLinkArtifact {
        &self.link
    }

    pub const fn publication(&self) -> &CrossConeArtifactSummary {
        &self.publication
    }
}

impl ArtifactClosurePlan {
    pub(crate) fn validate_completed_artifact(
        &self,
        current: ConeIdentity,
        snapshot: Arc<ArtifactSnapshot>,
        completed: &BTreeMap<ConeIdentity, Rc<ValidatedCrossConeArtifactHandle>>,
        c_bridge_profile: &CBridgeToolchainProfileV1,
    ) -> Result<Rc<ValidatedCrossConeArtifactHandle>, CrossConeArtifactValidationError> {
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
        let mut dependency_bytes = Vec::with_capacity(order.len() - 1);
        for identity in order.iter().filter(|identity| **identity != current) {
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
            .collect();
        let mut session = SemanticIdentitySession::new();
        let closure = validate_completed_cross_cone_artifact_closure(
            current,
            self.target,
            direct,
            dependency_bytes,
            snapshot.as_bytes(),
            c_bridge_profile,
            &mut session,
        )
        .map_err(|source| CrossConeArtifactValidationError::Closure(Box::new(source)))?;
        let (compile, link, publication) = closure.into_current_parts();
        Ok(Rc::new(ValidatedCrossConeArtifactHandle {
            snapshot,
            compile,
            link,
            publication,
        }))
    }
}

#[derive(Debug)]
pub enum CrossConeArtifactValidationError {
    Plan(Box<ArtifactClosureValidationError>),
    Closure(Box<CrossConeArtifactClosureValidationError>),
}

impl fmt::Display for CrossConeArtifactValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plan(source) => source.fmt(formatter),
            Self::Closure(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeArtifactValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Plan(source) => Some(source),
            Self::Closure(source) => Some(source),
        }
    }
}
