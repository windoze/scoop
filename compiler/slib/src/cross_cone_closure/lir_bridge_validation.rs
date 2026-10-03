//! Closure-wide MIR/LIR bridge equality and terminal-provider validation.

use std::collections::BTreeMap;

use scoop_identity::ConeIdentity;
use scoop_lir::ValidatedLirTargetSelection;

use super::{CrossConeProviderRole, MirBridgeValidatedCrossConeHirClosure};
use crate::LirBridgeValidatedCrossConeHirFrontSections;

mod errors;
mod relations;

pub use errors::*;

use relations::validate_lir_bridge_relations;
pub(crate) use relations::{AbiExpectation, validate_local_projection};

/// A semantic closure whose local strong LIR production, LIR bridge payloads,
/// MIR/LIR projections, and terminal-provider exports are all validated.
pub struct LirBridgeValidatedCrossConeHirClosure<'input> {
    pub(super) current: ConeIdentity,
    pub(super) target: ValidatedLirTargetSelection,
    pub(super) direct: Vec<ConeIdentity>,
    pub(super) dependency_first: Vec<LirBridgeValidatedCrossConeHirFrontSections<'input>>,
    pub(super) positions: BTreeMap<ConeIdentity, usize>,
    pub(super) dependency_positions: Vec<Vec<usize>>,
    pub(super) type_alias_expansions: Vec<scoop_hir::CanonicalTypeAliasExpansionsV1>,
}

impl LirBridgeValidatedCrossConeHirClosure<'_> {
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
    ) -> impl ExactSizeIterator<Item = &LirBridgeValidatedCrossConeHirFrontSections<'_>> {
        self.dependency_first.iter()
    }

    pub fn artifact(
        &self,
        identity: ConeIdentity,
    ) -> Option<&LirBridgeValidatedCrossConeHirFrontSections<'_>> {
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

    pub fn dependency_count(&self, identity: ConeIdentity) -> Option<usize> {
        self.positions
            .get(&identity)
            .map(|position| self.dependency_positions[*position].len())
    }

    pub fn type_alias_expansions(
        &self,
        identity: ConeIdentity,
    ) -> Option<&scoop_hir::CanonicalTypeAliasExpansionsV1> {
        self.positions
            .get(&identity)
            .map(|position| &self.type_alias_expansions[*position])
    }
}

impl<'input> MirBridgeValidatedCrossConeHirClosure<'input> {
    /// Validates every artifact's complete LIR production first, then proves
    /// that LIR exports and selections are exact projections of the already
    /// closed MIR bridge and resolve to the terminal provider's LIR export.
    pub fn validate_lir_bridges(
        self,
    ) -> Result<LirBridgeValidatedCrossConeHirClosure<'input>, CrossConeClosureLirBridgeError> {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
            type_alias_expansions,
        } = self;
        let artifact_count = dependency_first.len();
        let mut validated = Vec::new();
        validated.try_reserve_exact(artifact_count).map_err(|_| {
            CrossConeClosureLirBridgeError::Allocation {
                requested_slots: artifact_count,
            }
        })?;
        for front in dependency_first {
            let identity = front.identity();
            let mut artifact = front.validate_lir_bridge().map_err(|source| {
                CrossConeClosureLirBridgeError::Artifact {
                    identity,
                    source: Box::new(source),
                }
            })?;
            artifact
                .validate_shared_native_boundary(&validated, &dependency_positions)
                .map_err(|source| CrossConeClosureLirBridgeError::Artifact {
                    identity,
                    source: Box::new(crate::CrossConeLirFrontValidationError::NativeBoundary(
                        source,
                    )),
                })?;
            validated.push(artifact);
        }

        validate_lir_bridge_relations(&mut validated, &positions, &dependency_positions)?;

        Ok(LirBridgeValidatedCrossConeHirClosure {
            current,
            target,
            direct,
            dependency_first: validated,
            positions,
            dependency_positions,
            type_alias_expansions,
        })
    }
}
