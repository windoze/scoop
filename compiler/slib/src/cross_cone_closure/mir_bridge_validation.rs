//! Closure-wide MIR bridge eligibility and terminal-provider validation.

use std::collections::BTreeMap;

use scoop_identity::ConeIdentity;
use scoop_lir::ValidatedLirTargetSelection;

use super::{
    CrossConeProviderRole, TypeAliasExpandedCrossConeHirClosure,
    surface_validation::ConstValidatedClosureParts,
};
use crate::MirBridgeValidatedCrossConeHirFrontSections;

mod eligibility;
mod errors;
mod selected;

pub use errors::*;

use eligibility::{core_classifier, validate_export_surface};
use selected::validate_selected_closure;

/// A semantic closure whose provider MIR exports, consumer selections, HIR
/// selected-use evidence, and terminal-provider matches are all exact.
pub struct MirBridgeValidatedCrossConeHirClosure<'input> {
    pub(super) current: ConeIdentity,
    pub(super) target: ValidatedLirTargetSelection,
    pub(super) direct: Vec<ConeIdentity>,
    pub(super) dependency_first: Vec<MirBridgeValidatedCrossConeHirFrontSections<'input>>,
    pub(super) positions: BTreeMap<ConeIdentity, usize>,
    pub(super) dependency_positions: Vec<Vec<usize>>,
    pub(super) type_alias_expansions: Vec<scoop_hir::CanonicalTypeAliasExpansionsV1>,
}

impl MirBridgeValidatedCrossConeHirClosure<'_> {
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
    ) -> impl ExactSizeIterator<Item = &MirBridgeValidatedCrossConeHirFrontSections<'_>> {
        self.dependency_first.iter()
    }

    pub fn artifact(
        &self,
        identity: ConeIdentity,
    ) -> Option<&MirBridgeValidatedCrossConeHirFrontSections<'_>> {
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

impl<'input> TypeAliasExpandedCrossConeHirClosure<'input> {
    /// Validates each artifact's MIR production locally, derives the maximal
    /// core-closed export set from HIR, and closes every selected use against
    /// both its HIR route witness and terminal provider export.
    pub fn validate_mir_bridges(
        self,
    ) -> Result<MirBridgeValidatedCrossConeHirClosure<'input>, CrossConeClosureMirBridgeError> {
        let (references, type_alias_expansions) = self.into_mir_bridge_parts();
        let surfaces = references.into_routes().into_surfaces();
        let ConstValidatedClosureParts {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = surfaces.into_mir_bridge_parts();

        let artifact_count = dependency_first.len();
        let mut validated = Vec::new();
        validated.try_reserve_exact(artifact_count).map_err(|_| {
            CrossConeClosureMirBridgeError::Allocation {
                requested_slots: artifact_count,
            }
        })?;
        for front in dependency_first {
            let identity = front.identity();
            validated.push(front.validate_mir_bridge().map_err(|source| {
                CrossConeClosureMirBridgeError::Artifact {
                    identity,
                    source: Box::new(source),
                }
            })?);
        }

        if current != ConeIdentity::CORE {
            let core_position = positions
                .get(&ConeIdentity::CORE)
                .copied()
                .ok_or(CrossConeClosureMirBridgeError::MissingTrustedCore)?;
            let classifier = core_classifier(&validated[core_position])?;
            for front in &validated {
                validate_export_surface(front, &classifier).map_err(|source| {
                    CrossConeClosureMirBridgeError::Relation {
                        identity: front.identity(),
                        source: Box::new(source),
                    }
                })?;
            }
            validate_selected_closure(&validated, &positions, &dependency_positions)?;
        }

        Ok(MirBridgeValidatedCrossConeHirClosure {
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
