//! Dependency-graph validation for the shared Compile and Link profile.

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

mod complete;
mod read;
pub use read::read_cross_cone_layout_artifact_closure;
mod declarations;
mod foundations;
mod hir;
mod lir_callable_abis;
mod lir_callable_layouts;
mod lir_constituents;
mod lir_dependencies;
mod lir_descriptors;
mod lir_dispatch;
mod lir_layouts;
mod lir_ordinary;
mod lir_physical;
mod lir_shape_support;
mod lir_strong;
mod mir_dependencies;
mod mir_dispatch;
mod mir_source_callables;
mod mir_types;
mod type_foundations;
pub use complete::CrossConeLayoutSemanticClosureError;
pub use declarations::{
    CrossConeHirDeclarationValidationError, CrossConeLayoutHirDeclarationError,
    HirDeclarationsValidatedCrossConeLayoutClosure,
};
pub use hir::CrossConeLayoutClosureHirResolutionError;
pub use lir_callable_abis::{
    CrossConeLayoutLirCallableAbisError, LirCallableAbisValidatedCrossConeLayoutClosure,
    LirCallableAbisValidatedCrossConeLayoutSections, SharedLirCallableAbiValidationError,
    replay_shared_mir_callable_abis,
};
pub use lir_dependencies::{
    CrossConeLayoutLirDependenciesError, LirDependencyGraphReplayedCrossConeLayoutClosure,
    LirDependencyGraphReplayedCrossConeLayoutSections, SharedLirDependencyGraphError,
    replay_shared_lir_dependency_graph, replay_shared_lir_initialization_dependencies,
};
pub use lir_descriptors::{
    CrossConeLayoutLirDescriptorsError, LirDescriptorsValidatedCrossConeLayoutClosure,
    LirDescriptorsValidatedCrossConeLayoutSections, SharedLirDescriptorInputsV1,
    SharedLirDescriptorValidationError, replay_shared_mir_descriptors,
};
pub use lir_dispatch::{
    CrossConeLayoutLirDispatchError, LirDispatchValidatedCrossConeLayoutClosure,
    LirDispatchValidatedCrossConeLayoutSections, SharedLirDispatchAbiInputsV1,
    SharedLirDispatchValidationError, replay_shared_mir_dispatch,
};
pub use lir_layouts::{
    CrossConeLayoutLirLayoutsError, LirLayoutsValidatedCrossConeLayoutClosure,
    LirLayoutsValidatedCrossConeLayoutSections, SharedLirLayoutValidationError,
    replay_shared_mir_layouts,
};
pub use lir_ordinary::{
    CrossConeLayoutOrdinaryLirBridgeError, OrdinaryLirBridgeValidatedCrossConeLayoutClosure,
    OrdinaryLirBridgeValidatedCrossConeLayoutSections, SharedOrdinaryLirBridgeDependenciesV1,
    SharedOrdinaryLirBridgeValidationError, replay_shared_ordinary_lir_bridge,
};
pub use lir_physical::{
    CrossConeLayoutLirPhysicalError, LinkObjectsReplayedCrossConeLayoutClosure,
    LinkSymbolsReplayedCrossConeLayoutClosure, PhysicalImportsReplayedCrossConeLayoutClosure,
    PhysicalImportsReplayedCrossConeLayoutSections, SharedLirPhysicalError,
};
pub use lir_shape_support::{
    CrossConeLayoutLirShapeSupportError, LirExportsValidatedCrossConeLayoutClosure,
    LirExportsValidatedCrossConeLayoutSections, SharedLirShapeSupportValidationError,
    replay_shared_mir_shape_support,
};
pub use lir_strong::{
    CrossConeLayoutLirStrongProductionError, LirStrongProductionReplayedCrossConeLayoutClosure,
    LirStrongProductionReplayedCrossConeLayoutSections, SharedLirStrongProductionError,
};
pub use mir_dependencies::{
    CrossConeLayoutMirDependenciesError, MirDependencyGraphReplayedCrossConeLayoutClosure,
    MirDependencyGraphReplayedCrossConeLayoutSections, SharedMirDependencyGraphError,
    replay_shared_mir_dependency_graph,
};
pub use mir_dispatch::{
    SharedMirDispatchComponent, SharedMirDispatchValidationError, validate_shared_mir_dispatch,
};
pub use mir_source_callables::{
    CrossConeLayoutMirSourceCallablesError, MirSourceCallablesValidatedCrossConeLayoutClosure,
    MirSourceCallablesValidatedCrossConeLayoutSections, SharedMirConstructorComponent,
    SharedMirConstructorValidationError, SharedMirEqualityValidationError,
    SharedMirObjectComponent, SharedMirObjectValidationError, SharedMirSourceCallableComponent,
    SharedMirSourceCallablePartition, SharedMirSourceCallableValidationError,
    validate_shared_mir_constructors, validate_shared_mir_equality, validate_shared_mir_objects,
    validate_shared_mir_source_callables,
};
pub use mir_types::{
    SharedMirTypeComponent, SharedMirTypeValidationError, validate_shared_mir_type_exports,
};
pub use type_foundations::CrossConeLayoutTypeFoundationError;

/// Decoded artifacts visible while compiling one current Cone.
pub struct DecodedCrossConeLayoutCompileClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<DecodedCrossConeLayoutCompileSections<'input>>,
    current_artifact: Option<DecodedCrossConeLayoutCompileSections<'input>>,
}

/// Artifacts whose dependency graph, target, versions, and direct/support
/// references agree. Foundation and semantic checks follow.
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

/// Artifacts with structurally complete canonical HIR/MIR/LIR foundations
/// and imported source metadata checked against its original providers.
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
