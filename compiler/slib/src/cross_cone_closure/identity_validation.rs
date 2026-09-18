//! Dependency-profile and foundation-identity type-state transitions.

use scoop_identity::{ConeIdentity, ValidatedIdentityGraph};
use scoop_lir::ValidatedLirTargetSelection;

use crate::{DecodedCrossConeHirFrontSections, FoundationValidatedCrossConeHirFrontSections};

use super::{
    CrossConeClosureFoundationError, CrossConeClosureGraphError, CrossConeClosureIdentityError,
    CrossConeProviderRole, DecodedCrossConeClosure, FoundationValidatedCrossConeHirClosure,
    IdentityRegisteredCrossConeHirClosure, ProfileValidatedCrossConeHirClosure,
    graph_validation::validate_profile_graph, source_provenance::validate_imported_source_metadata,
};

impl<'input> DecodedCrossConeClosure<'input> {
    pub const fn new(
        current: ConeIdentity,
        target: ValidatedLirTargetSelection,
        direct: Vec<ConeIdentity>,
        dependency_first: Vec<DecodedCrossConeHirFrontSections<'input>>,
    ) -> Self {
        Self {
            current,
            target,
            direct,
            dependency_first,
            current_artifact: None,
        }
    }

    /// Constructs the closure used to validate one completed artifact.
    ///
    /// Dependency artifacts remain dependency-first; the current artifact is
    /// retained separately so callers cannot accidentally grant it provider
    /// visibility or place it before one of its own dependencies.
    pub const fn with_current_artifact(
        current: ConeIdentity,
        target: ValidatedLirTargetSelection,
        direct: Vec<ConeIdentity>,
        dependency_first: Vec<DecodedCrossConeHirFrontSections<'input>>,
        current_artifact: DecodedCrossConeHirFrontSections<'input>,
    ) -> Self {
        Self {
            current,
            target,
            direct,
            dependency_first,
            current_artifact: Some(current_artifact),
        }
    }

    /// Replays the graph facts needed by semantic import without trusting
    /// artifact argument order or role labels supplied by the caller.
    pub fn validate_profile_graph(
        self,
    ) -> Result<ProfileValidatedCrossConeHirClosure<'input>, CrossConeClosureGraphError> {
        validate_profile_graph(self)
    }
}

impl<'input> ProfileValidatedCrossConeHirClosure<'input> {
    pub const fn current(&self) -> ConeIdentity {
        self.current
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target
    }

    pub fn direct_providers(&self) -> &[ConeIdentity] {
        &self.direct
    }

    pub fn dependency_first(
        &self,
    ) -> impl ExactSizeIterator<Item = &DecodedCrossConeHirFrontSections<'_>> {
        self.dependency_first.iter()
    }

    pub fn artifact(
        &self,
        identity: ConeIdentity,
    ) -> Option<&DecodedCrossConeHirFrontSections<'_>> {
        self.positions
            .get(&identity)
            .map(|position| &self.dependency_first[*position])
    }

    pub fn role(&self, identity: ConeIdentity) -> Option<CrossConeProviderRole> {
        if identity == self.current {
            return None;
        }
        self.positions.get(&identity).map(|_| {
            if self.direct.binary_search(&identity).is_ok() {
                CrossConeProviderRole::Direct
            } else {
                CrossConeProviderRole::Support
            }
        })
    }

    /// Validates every provider foundation in dependency-first order without
    /// mutating the process-wide semantic session. Each provider receives
    /// canonical authority only from its own direct dependencies; authority
    /// carried by those graphs closes the transitive chain and folds diamonds.
    pub fn validate_identities(
        mut self,
    ) -> Result<IdentityRegisteredCrossConeHirClosure<'input>, CrossConeClosureIdentityError> {
        let artifact_count = self.dependency_first.len();
        let mut identity_graphs = Vec::new();
        identity_graphs
            .try_reserve_exact(artifact_count)
            .map_err(|_| CrossConeClosureIdentityError::Allocation {
                requested_slots: artifact_count,
            })?;

        for (position, front) in self.dependency_first.iter_mut().enumerate() {
            let identity = front.identity();
            let dependencies = &self.dependency_positions[position];
            let mut authorities = Vec::new();
            authorities
                .try_reserve_exact(dependencies.len())
                .map_err(|_| CrossConeClosureIdentityError::AuthorityAllocation {
                    identity,
                    requested_slots: dependencies.len(),
                })?;
            authorities.extend(
                dependencies
                    .iter()
                    .map(|dependency| &identity_graphs[*dependency]),
            );
            let graph = front
                .validate_foundation_identities(authorities)
                .map_err(|source| CrossConeClosureIdentityError::Artifact { identity, source })?;
            identity_graphs.push(graph);
        }

        Ok(IdentityRegisteredCrossConeHirClosure {
            profile: self,
            identity_graphs,
        })
    }
}

impl<'input> IdentityRegisteredCrossConeHirClosure<'input> {
    pub const fn current(&self) -> ConeIdentity {
        self.profile.current()
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.profile.target_selection()
    }

    pub fn direct_providers(&self) -> &[ConeIdentity] {
        self.profile.direct_providers()
    }

    pub fn dependency_first(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (
            &DecodedCrossConeHirFrontSections<'_>,
            &ValidatedIdentityGraph,
        ),
    > {
        self.profile
            .dependency_first
            .iter()
            .zip(self.identity_graphs.iter())
    }

    pub fn artifact(
        &self,
        identity: ConeIdentity,
    ) -> Option<(
        &DecodedCrossConeHirFrontSections<'_>,
        &ValidatedIdentityGraph,
    )> {
        self.profile.positions.get(&identity).map(|position| {
            (
                &self.profile.dependency_first[*position],
                &self.identity_graphs[*position],
            )
        })
    }

    pub fn role(&self, identity: ConeIdentity) -> Option<CrossConeProviderRole> {
        self.profile.role(identity)
    }

    /// Validates every provider's HIR/MIR/LIR foundation structure and the
    /// M23-5 `RejectAll` ODR policy before any production or public surface is
    /// allowed to consume those foundations.
    pub fn validate_foundation_structure(
        self,
    ) -> Result<FoundationValidatedCrossConeHirClosure<'input>, CrossConeClosureFoundationError>
    {
        let Self {
            profile,
            identity_graphs,
        } = self;
        let ProfileValidatedCrossConeHirClosure {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = profile;
        let artifact_count = dependency_first.len();
        let mut validated = Vec::<FoundationValidatedCrossConeHirFrontSections<'input>>::new();
        validated.try_reserve_exact(artifact_count).map_err(|_| {
            CrossConeClosureFoundationError::Allocation {
                requested_slots: artifact_count,
            }
        })?;
        for (position, (front, identities)) in dependency_first
            .into_iter()
            .zip(identity_graphs)
            .enumerate()
        {
            let identity = front.identity();
            let front = front
                .validate_foundation_structure(identities)
                .map_err(|source| CrossConeClosureFoundationError::Artifact {
                    identity,
                    source: Box::new(source),
                })?;
            validate_imported_source_metadata(identity, front.hir_foundation(), |provider| {
                positions
                    .get(&provider)
                    .copied()
                    .filter(|provider_position| *provider_position < position)
                    .map(|provider_position| validated[provider_position].hir_foundation())
            })
            .map_err(|source| CrossConeClosureFoundationError::SourceProvenance {
                identity,
                source,
            })?;
            validated.push(front);
        }

        Ok(FoundationValidatedCrossConeHirClosure {
            current,
            target,
            direct,
            dependency_first: validated,
            positions,
            dependency_positions,
        })
    }
}
