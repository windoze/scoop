//! Foundation identity and structure transactions for a complete layout graph.

use super::*;
use crate::{
    CrossConeClosureFoundationError, CrossConeClosureIdentityError,
    cross_cone_closure::source_provenance::validate_imported_source_metadata,
};

impl<'input> ProfileValidatedCrossConeLayoutCompileClosure<'input> {
    /// Rehashes every identity delta against only the artifact's recorded
    /// dependency authorities. The process-wide semantic session is untouched.
    pub fn validate_identities(
        self,
    ) -> Result<
        IdentityRegisteredCrossConeLayoutCompileClosure<'input>,
        CrossConeClosureIdentityError,
    > {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let artifact_count = dependency_first.len();
        let mut validated = Vec::<IdentityCheckedCrossConeLayoutCompileSections<'input>>::new();
        validated.try_reserve_exact(artifact_count).map_err(|_| {
            CrossConeClosureIdentityError::Allocation {
                requested_slots: artifact_count,
            }
        })?;

        for (position, front) in dependency_first.into_iter().enumerate() {
            let identity = front.identity();
            let dependencies = &dependency_positions[position];
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
                    .map(|dependency| validated[*dependency].identity_graph()),
            );
            let artifact = front
                .validate_foundation_identities(authorities)
                .map_err(|source| CrossConeClosureIdentityError::Artifact { identity, source })?;
            validated.push(artifact);
        }

        Ok(IdentityRegisteredCrossConeLayoutCompileClosure {
            current,
            target,
            direct,
            dependency_first: validated,
            positions,
            dependency_positions,
        })
    }
}

impl<'input> IdentityRegisteredCrossConeLayoutCompileClosure<'input> {
    /// Validates every ODR-free foundation and authenticates source metadata
    /// copied from an earlier terminal provider in the exact closure.
    pub fn validate_foundation_structure(
        self,
    ) -> Result<
        FoundationValidatedCrossConeLayoutCompileClosure<'input>,
        CrossConeClosureFoundationError,
    > {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let artifact_count = dependency_first.len();
        let mut validated = Vec::<FoundationValidatedCrossConeLayoutCompileSections<'input>>::new();
        validated.try_reserve_exact(artifact_count).map_err(|_| {
            CrossConeClosureFoundationError::Allocation {
                requested_slots: artifact_count,
            }
        })?;

        for (position, front) in dependency_first.into_iter().enumerate() {
            let identity = front.identity();
            let artifact = front.validate_foundation_structure().map_err(|source| {
                CrossConeClosureFoundationError::Artifact {
                    identity,
                    source: Box::new(source),
                }
            })?;
            validate_imported_source_metadata(identity, artifact.hir_foundation(), |provider| {
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
            validated.push(artifact);
        }

        Ok(FoundationValidatedCrossConeLayoutCompileClosure {
            current,
            target,
            direct,
            dependency_first: validated,
            positions,
            dependency_positions,
        })
    }
}
