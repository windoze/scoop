//! Dependency-graph type state for the M23-6 Compile profile.

use std::collections::BTreeMap;

use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_lir::ValidatedLirTargetSelection;

use crate::{
    ConeKind, CrossConeClosureGraphError, CrossConeProviderRole,
    DecodedCrossConeLayoutCompileSections, DependencyRecord,
    FoundationValidatedCrossConeLayoutCompileSections,
    HirProductionValidatedCrossConeLayoutSections, IdentityCheckedCrossConeLayoutCompileSections,
    ResolvedCrossConeLayoutHirSections,
    cross_cone_closure::graph_validation::{
        CrossConeClosureArtifact, ValidatedCrossConeClosureGraph, validate_artifact_graph,
    },
};

mod declarations;
mod foundations;
mod hir;
mod reachability;
mod type_foundations;
pub use declarations::{
    CrossConeHirDeclarationValidationError, CrossConeLayoutHirDeclarationError,
    HirDeclarationsValidatedCrossConeLayoutClosure,
};
pub use hir::CrossConeLayoutClosureHirResolutionError;
pub use type_foundations::CrossConeLayoutTypeFoundationError;

/// Untrusted M23-6 artifacts visible while compiling one current Cone.
pub struct DecodedCrossConeLayoutCompileClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<DecodedCrossConeLayoutCompileSections<'input>>,
    current_artifact: Option<DecodedCrossConeLayoutCompileSections<'input>>,
}

/// Exact-layout-profile artifacts whose dependency graph, target, versions,
/// and direct/support roles agree. No foundation or semantic authority has
/// been granted.
pub struct ProfileValidatedCrossConeLayoutCompileClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<DecodedCrossConeLayoutCompileSections<'input>>,
    positions: BTreeMap<ConeIdentity, usize>,
    dependency_positions: Vec<Vec<usize>>,
}

/// Layout-profile artifacts whose foundation identity deltas were validated
/// against exactly their recorded direct dependencies.
pub struct IdentityRegisteredCrossConeLayoutCompileClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<IdentityCheckedCrossConeLayoutCompileSections<'input>>,
    positions: BTreeMap<ConeIdentity, usize>,
    dependency_positions: Vec<Vec<usize>>,
}

/// Layout-profile artifacts with structurally complete, ODR-free HIR/MIR/LIR
/// foundations and authenticated imported source metadata.
pub struct FoundationValidatedCrossConeLayoutCompileClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<FoundationValidatedCrossConeLayoutCompileSections<'input>>,
    positions: BTreeMap<ConeIdentity, usize>,
    dependency_positions: Vec<Vec<usize>>,
}

/// Both old and new HIR transports are resolved through each artifact's one
/// validated identity graph. Semantic tables remain untrusted.
pub struct ResolvedCrossConeLayoutHirClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<ResolvedCrossConeLayoutHirSections<'input>>,
    positions: BTreeMap<ConeIdentity, usize>,
    dependency_positions: Vec<Vec<usize>>,
}

/// Resolved layout-profile HIR transports whose unchanged core bootstrap
/// production surface has also been replayed.
pub struct HirProductionValidatedCrossConeLayoutClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<HirProductionValidatedCrossConeLayoutSections<'input>>,
    positions: BTreeMap<ConeIdentity, usize>,
    dependency_positions: Vec<Vec<usize>>,
}

impl<'input> DecodedCrossConeLayoutCompileClosure<'input> {
    pub const fn new(
        current: ConeIdentity,
        target: ValidatedLirTargetSelection,
        direct: Vec<ConeIdentity>,
        dependency_first: Vec<DecodedCrossConeLayoutCompileSections<'input>>,
    ) -> Self {
        Self {
            current,
            target,
            direct,
            dependency_first,
            current_artifact: None,
        }
    }

    /// Keeps the current artifact separate until the validator proves its
    /// identity and exact direct-dependency set.
    pub const fn with_current_artifact(
        current: ConeIdentity,
        target: ValidatedLirTargetSelection,
        direct: Vec<ConeIdentity>,
        dependency_first: Vec<DecodedCrossConeLayoutCompileSections<'input>>,
        current_artifact: DecodedCrossConeLayoutCompileSections<'input>,
    ) -> Self {
        Self {
            current,
            target,
            direct,
            dependency_first,
            current_artifact: Some(current_artifact),
        }
    }

    pub fn validate_profile_graph(
        self,
    ) -> Result<ProfileValidatedCrossConeLayoutCompileClosure<'input>, CrossConeClosureGraphError>
    {
        let validated = validate_artifact_graph(
            self.current,
            self.target,
            self.direct,
            self.dependency_first,
            self.current_artifact,
        )?;
        Ok(ProfileValidatedCrossConeLayoutCompileClosure::from_graph(
            validated,
        ))
    }
}

impl<'input> ProfileValidatedCrossConeLayoutCompileClosure<'input> {
    fn from_graph(
        graph: ValidatedCrossConeClosureGraph<DecodedCrossConeLayoutCompileSections<'input>>,
    ) -> Self {
        Self {
            current: graph.current,
            target: graph.target,
            direct: graph.direct,
            dependency_first: graph.dependency_first,
            positions: graph.positions,
            dependency_positions: graph.dependency_positions,
        }
    }

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
    ) -> impl ExactSizeIterator<Item = &DecodedCrossConeLayoutCompileSections<'_>> {
        self.dependency_first.iter()
    }

    pub fn artifact(
        &self,
        identity: ConeIdentity,
    ) -> Option<&DecodedCrossConeLayoutCompileSections<'_>> {
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
}

macro_rules! impl_layout_closure_accessors {
    ($state:ident, $artifact:ident) => {
        impl $state<'_> {
            pub const fn current(&self) -> ConeIdentity {
                self.current
            }

            pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
                self.target
            }

            pub fn direct_providers(&self) -> &[ConeIdentity] {
                &self.direct
            }

            pub fn dependency_first(&self) -> impl ExactSizeIterator<Item = &$artifact<'_>> {
                self.dependency_first.iter()
            }

            pub fn artifact(&self, identity: ConeIdentity) -> Option<&$artifact<'_>> {
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
        }
    };
}

impl_layout_closure_accessors!(
    IdentityRegisteredCrossConeLayoutCompileClosure,
    IdentityCheckedCrossConeLayoutCompileSections
);
impl_layout_closure_accessors!(
    FoundationValidatedCrossConeLayoutCompileClosure,
    FoundationValidatedCrossConeLayoutCompileSections
);
impl_layout_closure_accessors!(
    ResolvedCrossConeLayoutHirClosure,
    ResolvedCrossConeLayoutHirSections
);
impl_layout_closure_accessors!(
    HirProductionValidatedCrossConeLayoutClosure,
    HirProductionValidatedCrossConeLayoutSections
);

impl<'input> HirProductionValidatedCrossConeLayoutClosure<'input> {
    pub(crate) fn hir_semantic_validation_parts(
        &mut self,
    ) -> (
        &mut [HirProductionValidatedCrossConeLayoutSections<'input>],
        &[Vec<usize>],
    ) {
        (&mut self.dependency_first, &self.dependency_positions)
    }

    pub(crate) fn into_mir_semantic_validation_parts(
        self,
    ) -> (
        ConeIdentity,
        ValidatedLirTargetSelection,
        Vec<ConeIdentity>,
        Vec<HirProductionValidatedCrossConeLayoutSections<'input>>,
        Vec<Vec<usize>>,
    ) {
        (
            self.current,
            self.target,
            self.direct,
            self.dependency_first,
            self.dependency_positions,
        )
    }
}

#[cfg(test)]
pub(crate) fn layout_hir_semantic_closure_for_test<'input>(
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<HirProductionValidatedCrossConeLayoutSections<'input>>,
    dependency_positions: Vec<Vec<usize>>,
) -> HirProductionValidatedCrossConeLayoutClosure<'input> {
    debug_assert_eq!(dependency_first.len(), dependency_positions.len());
    debug_assert!(
        dependency_positions
            .iter()
            .enumerate()
            .all(|(position, dependencies)| dependencies
                .iter()
                .all(|dependency| *dependency < position))
    );
    let positions = dependency_first
        .iter()
        .enumerate()
        .map(|(position, artifact)| (artifact.identity(), position))
        .collect();
    HirProductionValidatedCrossConeLayoutClosure {
        current,
        target,
        direct,
        dependency_first,
        positions,
        dependency_positions,
    }
}

impl CrossConeClosureArtifact for DecodedCrossConeLayoutCompileSections<'_> {
    fn coordinate(&self) -> &ConeCoordinate {
        self.coordinate()
    }

    fn identity(&self) -> ConeIdentity {
        self.identity()
    }

    fn kind(&self) -> ConeKind {
        self.kind()
    }

    fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target_selection()
    }

    fn direct_dependencies(&self) -> &[DependencyRecord] {
        self.direct_dependencies()
    }

    fn dependency_record(&self) -> DependencyRecord {
        self.dependency_record()
    }
}

#[cfg(test)]
mod tests;
