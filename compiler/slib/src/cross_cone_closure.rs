//! Graph and provider-role validation for the M23-5 semantic closure.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_hir::{CoreBootstrapInterfaceValidationError, CrossConeHirInterfaceResolutionError};
use scoop_identity::{
    ConeCoordinate, ConeIdentity, IdentityReferenceError, IdentityValidationError,
    ValidatedIdentityGraph,
};
use scoop_lir::ValidatedLirTargetSelection;

use crate::{
    CallableValidatedCrossConeHirFrontSections, ConeKind, CrossConeHirCallableSurfaceError,
    CrossConeHirNominalSurfaceError, CrossConeHirPropertySurfaceError,
    DecodedCrossConeHirFrontSections, DependencyRecord,
    FoundationValidatedCrossConeHirFrontSections, HirProductionValidatedCrossConeHirFrontSections,
    NominalValidatedCrossConeHirFrontSections, PropertyValidatedCrossConeHirFrontSections,
    ResolvedCrossConeHirFrontSections, StrongProfileFoundationError,
};

/// Untrusted assembly input for the dependency artifacts visible while
/// compiling one current Cone.
pub struct DecodedCrossConeClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<DecodedCrossConeHirFrontSections<'input>>,
}

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

/// Directness is a closed role assigned only after the whole dependency
/// graph has been checked. A support provider cannot be promoted by a bool.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CrossConeProviderRole {
    Direct,
    Support,
}

/// Cross-Cone HIR fronts whose exact profile and complete dependency graph
/// agree. Identity, surface, route, and bridge obligations remain unproven.
pub struct ProfileValidatedCrossConeHirClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<DecodedCrossConeHirFrontSections<'input>>,
    positions: BTreeMap<ConeIdentity, usize>,
    dependency_positions: Vec<Vec<usize>>,
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

/// Cross-Cone HIR fronts whose foundation identities were rehashed and
/// resolved against their exact dependency authority. No graph has been
/// committed to the caller's semantic session yet.
pub struct IdentityRegisteredCrossConeHirClosure<'input> {
    profile: ProfileValidatedCrossConeHirClosure<'input>,
    identity_graphs: Vec<ValidatedIdentityGraph>,
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
        let mut validated = Vec::new();
        validated.try_reserve_exact(artifact_count).map_err(|_| {
            CrossConeClosureFoundationError::Allocation {
                requested_slots: artifact_count,
            }
        })?;
        for (front, identities) in dependency_first.into_iter().zip(identity_graphs) {
            let identity = front.identity();
            validated.push(
                front
                    .validate_foundation_structure(identities)
                    .map_err(|source| CrossConeClosureFoundationError::Artifact {
                        identity,
                        source: Box::new(source),
                    })?,
            );
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

/// Cross-Cone HIR fronts with fully checked ODR-free foundations. General HIR
/// surfaces and legacy production payloads remain unresolved and unvalidated.
pub struct FoundationValidatedCrossConeHirClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<FoundationValidatedCrossConeHirFrontSections<'input>>,
    positions: BTreeMap<ConeIdentity, usize>,
    dependency_positions: Vec<Vec<usize>>,
}

impl<'input> FoundationValidatedCrossConeHirClosure<'input> {
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
    ) -> impl ExactSizeIterator<Item = &FoundationValidatedCrossConeHirFrontSections<'_>> {
        self.dependency_first.iter()
    }

    pub fn artifact(
        &self,
        identity: ConeIdentity,
    ) -> Option<&FoundationValidatedCrossConeHirFrontSections<'_>> {
        self.positions
            .get(&identity)
            .map(|position| &self.dependency_first[*position])
    }

    pub fn role(&self, identity: ConeIdentity) -> Option<CrossConeProviderRole> {
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

    /// Resolves every general HIR section against the same identity graph
    /// that proved its foundation. This does not yet grant public-surface or
    /// route authority.
    pub fn resolve_hir_interfaces(
        self,
    ) -> Result<ResolvedCrossConeHirClosure<'input>, CrossConeClosureHirResolutionError> {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let artifact_count = dependency_first.len();
        let mut resolved = Vec::new();
        resolved.try_reserve_exact(artifact_count).map_err(|_| {
            CrossConeClosureHirResolutionError::Allocation {
                requested_slots: artifact_count,
            }
        })?;
        for front in dependency_first {
            let identity = front.identity();
            resolved.push(front.resolve_hir_interface().map_err(|source| {
                CrossConeClosureHirResolutionError::Artifact {
                    identity,
                    source: Box::new(source),
                }
            })?);
        }

        Ok(ResolvedCrossConeHirClosure {
            current,
            target,
            direct,
            dependency_first: resolved,
            positions,
            dependency_positions,
        })
    }
}

/// Cross-Cone providers whose general HIR wire references are typed and
/// resolved. Semantic ownership, public-surface, and route closure are still
/// pending, so this state cannot be imported into a world or session.
pub struct ResolvedCrossConeHirClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<ResolvedCrossConeHirFrontSections<'input>>,
    positions: BTreeMap<ConeIdentity, usize>,
    dependency_positions: Vec<Vec<usize>>,
}

impl ResolvedCrossConeHirClosure<'_> {
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
    ) -> impl ExactSizeIterator<Item = &ResolvedCrossConeHirFrontSections<'_>> {
        self.dependency_first.iter()
    }

    pub fn artifact(
        &self,
        identity: ConeIdentity,
    ) -> Option<&ResolvedCrossConeHirFrontSections<'_>> {
        self.positions
            .get(&identity)
            .map(|position| &self.dependency_first[*position])
    }

    pub fn role(&self, identity: ConeIdentity) -> Option<CrossConeProviderRole> {
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

impl<'input> ResolvedCrossConeHirClosure<'input> {
    /// Validates every provider's unchanged M23-3 HIR production section
    /// before its direct-public surface is used as M23-5 semantic authority.
    pub fn validate_hir_productions(
        self,
    ) -> Result<HirProductionValidatedCrossConeHirClosure<'input>, CrossConeClosureHirProductionError>
    {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let artifact_count = dependency_first.len();
        let mut validated = Vec::new();
        validated.try_reserve_exact(artifact_count).map_err(|_| {
            CrossConeClosureHirProductionError::Allocation {
                requested_slots: artifact_count,
            }
        })?;
        for front in dependency_first {
            let identity = front.identity();
            validated.push(front.validate_hir_production().map_err(|source| {
                CrossConeClosureHirProductionError::Artifact { identity, source }
            })?);
        }

        Ok(HirProductionValidatedCrossConeHirClosure {
            current,
            target,
            direct,
            dependency_first: validated,
            positions,
            dependency_positions,
        })
    }
}

/// Cross-Cone providers whose foundations, general HIR references, and legacy
/// direct-public surfaces are validated. General surface semantics and routes
/// remain pending, so this state still cannot be imported.
pub struct HirProductionValidatedCrossConeHirClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<HirProductionValidatedCrossConeHirFrontSections<'input>>,
    positions: BTreeMap<ConeIdentity, usize>,
    dependency_positions: Vec<Vec<usize>>,
}

impl HirProductionValidatedCrossConeHirClosure<'_> {
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
    ) -> impl ExactSizeIterator<Item = &HirProductionValidatedCrossConeHirFrontSections<'_>> {
        self.dependency_first.iter()
    }

    pub fn artifact(
        &self,
        identity: ConeIdentity,
    ) -> Option<&HirProductionValidatedCrossConeHirFrontSections<'_>> {
        self.positions
            .get(&identity)
            .map(|position| &self.dependency_first[*position])
    }

    pub fn role(&self, identity: ConeIdentity) -> Option<CrossConeProviderRole> {
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

impl<'input> HirProductionValidatedCrossConeHirClosure<'input> {
    /// Validates public nominal declarations dependency-first. Each provider
    /// receives semantic surfaces only from its own transitive closure; an
    /// already validated sibling can never satisfy a nominal lookup.
    pub fn validate_nominal_surfaces(
        self,
    ) -> Result<NominalValidatedCrossConeHirClosure<'input>, CrossConeClosureNominalSurfaceError>
    {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let artifact_count = dependency_first.len();
        let mut validated = Vec::<NominalValidatedCrossConeHirFrontSections<'input>>::new();
        validated.try_reserve_exact(artifact_count).map_err(|_| {
            CrossConeClosureNominalSurfaceError::Allocation {
                requested_slots: artifact_count,
            }
        })?;

        for (position, front) in dependency_first.into_iter().enumerate() {
            let identity = front.identity();
            let reachable = transitive_dependency_positions(position, &dependency_positions);
            let mut dependencies = Vec::new();
            dependencies
                .try_reserve_exact(reachable.len())
                .map_err(
                    |_| CrossConeClosureNominalSurfaceError::AuthorityAllocation {
                        identity,
                        requested_slots: reachable.len(),
                    },
                )?;
            dependencies.extend(
                reachable
                    .into_iter()
                    .map(|dependency| validated[dependency].nominal_provider_view()),
            );
            let front = front
                .validate_nominal_surface(dependencies)
                .map_err(|source| CrossConeClosureNominalSurfaceError::Artifact {
                    identity,
                    source: Box::new(source),
                })?;
            validated.push(front);
        }

        Ok(NominalValidatedCrossConeHirClosure {
            current,
            target,
            direct,
            dependency_first: validated,
            positions,
            dependency_positions,
        })
    }
}

/// Cross-Cone providers whose nominal interfaces are canonical and owned by
/// the declaring provider. Callable, property, alias, route, and bridge
/// semantics remain pending, so this state cannot be imported.
pub struct NominalValidatedCrossConeHirClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<NominalValidatedCrossConeHirFrontSections<'input>>,
    positions: BTreeMap<ConeIdentity, usize>,
    dependency_positions: Vec<Vec<usize>>,
}

impl NominalValidatedCrossConeHirClosure<'_> {
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
    ) -> impl ExactSizeIterator<Item = &NominalValidatedCrossConeHirFrontSections<'_>> {
        self.dependency_first.iter()
    }

    pub fn artifact(
        &self,
        identity: ConeIdentity,
    ) -> Option<&NominalValidatedCrossConeHirFrontSections<'_>> {
        self.positions
            .get(&identity)
            .map(|position| &self.dependency_first[*position])
    }

    pub fn role(&self, identity: ConeIdentity) -> Option<CrossConeProviderRole> {
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

impl<'input> NominalValidatedCrossConeHirClosure<'input> {
    /// Validates public property declaration shapes dependency-first while
    /// exposing only each provider's transitive nominal authority.
    pub fn validate_property_surfaces(
        self,
    ) -> Result<PropertyValidatedCrossConeHirClosure<'input>, CrossConeClosurePropertySurfaceError>
    {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let artifact_count = dependency_first.len();
        let mut validated = Vec::<PropertyValidatedCrossConeHirFrontSections<'input>>::new();
        validated.try_reserve_exact(artifact_count).map_err(|_| {
            CrossConeClosurePropertySurfaceError::Allocation {
                requested_slots: artifact_count,
            }
        })?;

        for (position, front) in dependency_first.into_iter().enumerate() {
            let identity = front.identity();
            let reachable = transitive_dependency_positions(position, &dependency_positions);
            let mut dependencies = Vec::new();
            dependencies
                .try_reserve_exact(reachable.len())
                .map_err(
                    |_| CrossConeClosurePropertySurfaceError::AuthorityAllocation {
                        identity,
                        requested_slots: reachable.len(),
                    },
                )?;
            dependencies.extend(
                reachable
                    .into_iter()
                    .map(|dependency| validated[dependency].nominal_provider_view()),
            );
            let front = front
                .validate_property_surface(dependencies)
                .map_err(|source| CrossConeClosurePropertySurfaceError::Artifact {
                    identity,
                    source: Box::new(source),
                })?;
            validated.push(front);
        }

        Ok(PropertyValidatedCrossConeHirClosure {
            current,
            target,
            direct,
            dependency_first: validated,
            positions,
            dependency_positions,
        })
    }
}

/// Cross-Cone providers whose nominal and property interfaces are canonical.
/// Callable, alias, source/default/const, route, and bridge obligations remain.
pub struct PropertyValidatedCrossConeHirClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<PropertyValidatedCrossConeHirFrontSections<'input>>,
    positions: BTreeMap<ConeIdentity, usize>,
    dependency_positions: Vec<Vec<usize>>,
}

impl PropertyValidatedCrossConeHirClosure<'_> {
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
    ) -> impl ExactSizeIterator<Item = &PropertyValidatedCrossConeHirFrontSections<'_>> {
        self.dependency_first.iter()
    }

    pub fn artifact(
        &self,
        identity: ConeIdentity,
    ) -> Option<&PropertyValidatedCrossConeHirFrontSections<'_>> {
        self.positions
            .get(&identity)
            .map(|position| &self.dependency_first[*position])
    }

    pub fn role(&self, identity: ConeIdentity) -> Option<CrossConeProviderRole> {
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

impl<'input> PropertyValidatedCrossConeHirClosure<'input> {
    /// Validates public callable declaration shapes dependency-first while
    /// exposing only property-validated providers from each transitive Cone.
    pub fn validate_callable_surfaces(
        self,
    ) -> Result<CallableValidatedCrossConeHirClosure<'input>, CrossConeClosureCallableSurfaceError>
    {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let artifact_count = dependency_first.len();
        let mut validated = Vec::<CallableValidatedCrossConeHirFrontSections<'input>>::new();
        validated.try_reserve_exact(artifact_count).map_err(|_| {
            CrossConeClosureCallableSurfaceError::Allocation {
                requested_slots: artifact_count,
            }
        })?;

        for (position, front) in dependency_first.into_iter().enumerate() {
            let identity = front.identity();
            let reachable = transitive_dependency_positions(position, &dependency_positions);
            let mut dependencies = Vec::new();
            dependencies
                .try_reserve_exact(reachable.len())
                .map_err(
                    |_| CrossConeClosureCallableSurfaceError::AuthorityAllocation {
                        identity,
                        requested_slots: reachable.len(),
                    },
                )?;
            dependencies.extend(
                reachable
                    .into_iter()
                    .map(|dependency| validated[dependency].nominal_provider_view()),
            );
            let front = front
                .validate_callable_surface(dependencies)
                .map_err(|source| CrossConeClosureCallableSurfaceError::Artifact {
                    identity,
                    source: Box::new(source),
                })?;
            validated.push(front);
        }

        Ok(CallableValidatedCrossConeHirClosure {
            current,
            target,
            direct,
            dependency_first: validated,
            positions,
            dependency_positions,
        })
    }
}

/// Cross-Cone providers whose nominal, property, and callable interfaces are
/// canonical. The remaining surface, route, and bridge obligations prevent
/// import.
pub struct CallableValidatedCrossConeHirClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<CallableValidatedCrossConeHirFrontSections<'input>>,
    positions: BTreeMap<ConeIdentity, usize>,
    dependency_positions: Vec<Vec<usize>>,
}

impl CallableValidatedCrossConeHirClosure<'_> {
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
    ) -> impl ExactSizeIterator<Item = &CallableValidatedCrossConeHirFrontSections<'_>> {
        self.dependency_first.iter()
    }

    pub fn artifact(
        &self,
        identity: ConeIdentity,
    ) -> Option<&CallableValidatedCrossConeHirFrontSections<'_>> {
        self.positions
            .get(&identity)
            .map(|position| &self.dependency_first[*position])
    }

    pub fn role(&self, identity: ConeIdentity) -> Option<CrossConeProviderRole> {
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

fn transitive_dependency_positions(
    position: usize,
    dependency_positions: &[Vec<usize>],
) -> Vec<usize> {
    let mut reachable = vec![false; position];
    let mut pending = dependency_positions[position].clone();
    while let Some(dependency) = pending.pop() {
        if reachable[dependency] {
            continue;
        }
        reachable[dependency] = true;
        pending.extend(dependency_positions[dependency].iter().copied());
    }
    reachable
        .into_iter()
        .enumerate()
        .filter_map(|(dependency, reachable)| reachable.then_some(dependency))
        .collect()
}

fn validate_profile_graph(
    closure: DecodedCrossConeClosure<'_>,
) -> Result<ProfileValidatedCrossConeHirClosure<'_>, CrossConeClosureGraphError> {
    let DecodedCrossConeClosure {
        current,
        target,
        direct,
        dependency_first,
    } = closure;

    validate_direct_order(&direct)?;
    if current == ConeIdentity::CORE {
        if !direct.is_empty() || !dependency_first.is_empty() {
            return Err(CrossConeClosureGraphError::CoreHasDependencyProviders);
        }
    } else if direct.binary_search(&ConeIdentity::CORE).is_err() {
        return Err(CrossConeClosureGraphError::MissingTrustedCore);
    }

    let mut positions = BTreeMap::new();
    let mut versions = BTreeMap::<(String, String), ConeCoordinate>::new();
    for (position, artifact) in dependency_first.iter().enumerate() {
        let identity = artifact.identity();
        if identity == current {
            return Err(CrossConeClosureGraphError::CurrentArtifactPresent { current });
        }
        if positions.insert(identity, position).is_some() {
            return Err(CrossConeClosureGraphError::DuplicateArtifact { identity });
        }
        if artifact.kind() != ConeKind::Library {
            return Err(CrossConeClosureGraphError::InvalidProviderKind {
                identity,
                actual: artifact.kind(),
            });
        }
        if artifact.target_selection() != target {
            return Err(CrossConeClosureGraphError::TargetMismatch {
                identity,
                expected: target,
                actual: artifact.target_selection(),
            });
        }
        validate_unique_version(&mut versions, artifact.coordinate(), identity)?;
    }

    for identity in &direct {
        if !positions.contains_key(identity) {
            return Err(CrossConeClosureGraphError::MissingDirectArtifact {
                identity: *identity,
            });
        }
    }

    let mut dependency_positions = Vec::with_capacity(dependency_first.len());
    for (position, artifact) in dependency_first.iter().enumerate() {
        let dependent = artifact.identity();
        let mut direct_positions = Vec::with_capacity(artifact.direct_dependencies().len());
        for recorded in artifact.direct_dependencies() {
            let dependency = recorded.identity();
            let Some(dependency_position) = positions.get(&dependency).copied() else {
                return Err(CrossConeClosureGraphError::MissingDependencyArtifact {
                    dependent,
                    dependency,
                });
            };
            if dependency_position >= position {
                return Err(CrossConeClosureGraphError::InvalidDependencyFirstOrder {
                    dependent,
                    dependency,
                });
            }
            let actual = dependency_first[dependency_position].dependency_record();
            if recorded != &actual {
                return Err(CrossConeClosureGraphError::StaleDependency {
                    dependent,
                    dependency,
                    recorded: Box::new(recorded.clone()),
                    actual: Box::new(actual),
                });
            }
            direct_positions.push(dependency_position);
        }
        dependency_positions.push(direct_positions);
    }

    let mut reachable = BTreeSet::new();
    let mut pending = direct.clone();
    while let Some(identity) = pending.pop() {
        if !reachable.insert(identity) {
            continue;
        }
        let artifact = &dependency_first[positions[&identity]];
        pending.extend(
            artifact
                .direct_dependencies()
                .iter()
                .rev()
                .map(DependencyRecord::identity),
        );
    }
    if let Some(artifact) = dependency_first
        .iter()
        .find(|artifact| !reachable.contains(&artifact.identity()))
    {
        return Err(CrossConeClosureGraphError::UnreachableSupport {
            identity: artifact.identity(),
        });
    }

    Ok(ProfileValidatedCrossConeHirClosure {
        current,
        target,
        direct,
        dependency_first,
        positions,
        dependency_positions,
    })
}

fn validate_direct_order(direct: &[ConeIdentity]) -> Result<(), CrossConeClosureGraphError> {
    for (index, pair) in direct.windows(2).enumerate() {
        if pair[0] >= pair[1] {
            return Err(CrossConeClosureGraphError::NonCanonicalDirectProviders {
                index: index + 1,
                previous: pair[0],
                actual: pair[1],
            });
        }
    }
    Ok(())
}

fn validate_unique_version(
    versions: &mut BTreeMap<(String, String), ConeCoordinate>,
    coordinate: &ConeCoordinate,
    _identity: ConeIdentity,
) -> Result<(), CrossConeClosureGraphError> {
    let key = (coordinate.group().to_owned(), coordinate.name().to_owned());
    if let Some(first) = versions.get(&key)
        && first.version() != coordinate.version()
    {
        return Err(CrossConeClosureGraphError::MultipleVersions {
            first: Box::new(first.clone()),
            second: Box::new(coordinate.clone()),
        });
    }
    versions.insert(key, coordinate.clone());
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CrossConeClosureGraphError {
    CoreHasDependencyProviders,
    MissingTrustedCore,
    NonCanonicalDirectProviders {
        index: usize,
        previous: ConeIdentity,
        actual: ConeIdentity,
    },
    CurrentArtifactPresent {
        current: ConeIdentity,
    },
    DuplicateArtifact {
        identity: ConeIdentity,
    },
    InvalidProviderKind {
        identity: ConeIdentity,
        actual: ConeKind,
    },
    TargetMismatch {
        identity: ConeIdentity,
        expected: ValidatedLirTargetSelection,
        actual: ValidatedLirTargetSelection,
    },
    MultipleVersions {
        first: Box<ConeCoordinate>,
        second: Box<ConeCoordinate>,
    },
    MissingDirectArtifact {
        identity: ConeIdentity,
    },
    MissingDependencyArtifact {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
    },
    InvalidDependencyFirstOrder {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
    },
    StaleDependency {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
        recorded: Box<DependencyRecord>,
        actual: Box<DependencyRecord>,
    },
    UnreachableSupport {
        identity: ConeIdentity,
    },
}

impl fmt::Display for CrossConeClosureGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid cross-Cone semantic closure graph: {self:?}"
        )
    }
}

impl std::error::Error for CrossConeClosureGraphError {}

#[derive(Debug)]
pub enum CrossConeClosureIdentityError {
    Allocation {
        requested_slots: usize,
    },
    AuthorityAllocation {
        identity: ConeIdentity,
        requested_slots: usize,
    },
    Artifact {
        identity: ConeIdentity,
        source: IdentityValidationError,
    },
}

impl fmt::Display for CrossConeClosureIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} cross-Cone identity graph slots"
            ),
            Self::AuthorityAllocation {
                identity,
                requested_slots,
            } => write!(
                formatter,
                "cannot allocate {requested_slots} dependency authority slots for {identity}"
            ),
            Self::Artifact { identity, source } => {
                write!(
                    formatter,
                    "invalid identity foundation for {identity}: {source}"
                )
            }
        }
    }
}

impl std::error::Error for CrossConeClosureIdentityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact { source, .. } => Some(source),
            Self::Allocation { .. } | Self::AuthorityAllocation { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum CrossConeClosureFoundationError {
    Allocation {
        requested_slots: usize,
    },
    Artifact {
        identity: ConeIdentity,
        source: Box<StrongProfileFoundationError>,
    },
}

impl fmt::Display for CrossConeClosureFoundationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} validated cross-Cone foundation slots"
            ),
            Self::Artifact { identity, source } => {
                write!(
                    formatter,
                    "invalid cross-Cone foundations for {identity}: {source}"
                )
            }
        }
    }
}

impl std::error::Error for CrossConeClosureFoundationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact { source, .. } => Some(source),
            Self::Allocation { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum CrossConeClosureHirResolutionError {
    Allocation {
        requested_slots: usize,
    },
    Artifact {
        identity: ConeIdentity,
        source: Box<CrossConeHirInterfaceResolutionError<IdentityReferenceError>>,
    },
}

impl fmt::Display for CrossConeClosureHirResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} resolved cross-Cone HIR slots"
            ),
            Self::Artifact { identity, source } => {
                write!(
                    formatter,
                    "cannot resolve cross-Cone HIR interface for {identity}: {source}"
                )
            }
        }
    }
}

impl std::error::Error for CrossConeClosureHirResolutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact { source, .. } => Some(source.as_ref()),
            Self::Allocation { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum CrossConeClosureHirProductionError {
    Allocation {
        requested_slots: usize,
    },
    Artifact {
        identity: ConeIdentity,
        source: CoreBootstrapInterfaceValidationError,
    },
}

impl fmt::Display for CrossConeClosureHirProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} validated cross-Cone HIR production slots"
            ),
            Self::Artifact { identity, source } => write!(
                formatter,
                "invalid legacy HIR production surface for {identity}: {source}"
            ),
        }
    }
}

impl std::error::Error for CrossConeClosureHirProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact { source, .. } => Some(source),
            Self::Allocation { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum CrossConeClosureNominalSurfaceError {
    Allocation {
        requested_slots: usize,
    },
    AuthorityAllocation {
        identity: ConeIdentity,
        requested_slots: usize,
    },
    Artifact {
        identity: ConeIdentity,
        source: Box<CrossConeHirNominalSurfaceError>,
    },
}

#[derive(Debug)]
pub enum CrossConeClosureCallableSurfaceError {
    Allocation {
        requested_slots: usize,
    },
    AuthorityAllocation {
        identity: ConeIdentity,
        requested_slots: usize,
    },
    Artifact {
        identity: ConeIdentity,
        source: Box<CrossConeHirCallableSurfaceError>,
    },
}

#[derive(Debug)]
pub enum CrossConeClosurePropertySurfaceError {
    Allocation {
        requested_slots: usize,
    },
    AuthorityAllocation {
        identity: ConeIdentity,
        requested_slots: usize,
    },
    Artifact {
        identity: ConeIdentity,
        source: Box<CrossConeHirPropertySurfaceError>,
    },
}

impl fmt::Display for CrossConeClosurePropertySurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} property-validated cross-Cone HIR slots"
            ),
            Self::AuthorityAllocation {
                identity,
                requested_slots,
            } => write!(
                formatter,
                "cannot allocate {requested_slots} property dependency authority slots for {identity}"
            ),
            Self::Artifact { identity, source } => {
                write!(
                    formatter,
                    "invalid property HIR surface for {identity}: {source}"
                )
            }
        }
    }
}

impl std::error::Error for CrossConeClosurePropertySurfaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact { source, .. } => Some(source),
            Self::Allocation { .. } | Self::AuthorityAllocation { .. } => None,
        }
    }
}

impl fmt::Display for CrossConeClosureCallableSurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} callable-validated cross-Cone HIR slots"
            ),
            Self::AuthorityAllocation {
                identity,
                requested_slots,
            } => write!(
                formatter,
                "cannot allocate {requested_slots} callable dependency authority slots for {identity}"
            ),
            Self::Artifact { identity, source } => {
                write!(
                    formatter,
                    "invalid callable HIR surface for {identity}: {source}"
                )
            }
        }
    }
}

impl std::error::Error for CrossConeClosureCallableSurfaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact { source, .. } => Some(source),
            Self::Allocation { .. } | Self::AuthorityAllocation { .. } => None,
        }
    }
}

impl fmt::Display for CrossConeClosureNominalSurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} nominal-validated cross-Cone HIR slots"
            ),
            Self::AuthorityAllocation {
                identity,
                requested_slots,
            } => write!(
                formatter,
                "cannot allocate {requested_slots} nominal dependency authority slots for {identity}"
            ),
            Self::Artifact { identity, source } => {
                write!(
                    formatter,
                    "invalid nominal HIR surface for {identity}: {source}"
                )
            }
        }
    }
}

impl std::error::Error for CrossConeClosureNominalSurfaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact { source, .. } => Some(source),
            Self::Allocation { .. } | Self::AuthorityAllocation { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests;
