//! Closure-wide HIR declaration-surface validation.

use std::collections::BTreeMap;

use scoop_identity::ConeIdentity;
use scoop_lir::ValidatedLirTargetSelection;

use super::{CrossConeProviderRole, HirProductionValidatedCrossConeHirClosure};
use crate::{
    CallableValidatedCrossConeHirFrontSections, ConstValidatedCrossConeHirFrontSections,
    DefinitionSourceValidatedCrossConeHirFrontSections, InternallyClosedCrossConeHirFrontSections,
    NominalValidatedCrossConeHirFrontSections, PropertyValidatedCrossConeHirFrontSections,
    SourceInterfaceValidatedCrossConeHirFrontSections, TypeAliasValidatedCrossConeHirFrontSections,
};

mod const_value;
mod definition_source;
mod dependency_views;
mod errors;
mod source_interface;

pub use const_value::*;
pub use definition_source::*;
pub use errors::*;
pub use source_interface::*;

#[cfg(test)]
pub(super) use dependency_views::transitive_positions_for_test;
pub(super) use dependency_views::{nominal_dependencies, transitive_dependency_positions};

/// Shared graph carrier behind each consuming surface-validation state.
struct ValidatedSurfaceClosure<T> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<T>,
    positions: BTreeMap<ConeIdentity, usize>,
    dependency_positions: Vec<Vec<usize>>,
}

/// Providers whose internal general-HIR relations are exact.
pub struct InternallyClosedCrossConeHirClosure<'input>(
    ValidatedSurfaceClosure<InternallyClosedCrossConeHirFrontSections<'input>>,
);

/// Providers whose nominal interfaces have canonical typed ownership.
pub struct NominalValidatedCrossConeHirClosure<'input>(
    ValidatedSurfaceClosure<NominalValidatedCrossConeHirFrontSections<'input>>,
);

/// Providers whose nominal and property interfaces are canonical.
pub struct PropertyValidatedCrossConeHirClosure<'input>(
    ValidatedSurfaceClosure<PropertyValidatedCrossConeHirFrontSections<'input>>,
);

/// Providers whose nominal, property, and callable interfaces are canonical.
pub struct CallableValidatedCrossConeHirClosure<'input>(
    ValidatedSurfaceClosure<CallableValidatedCrossConeHirFrontSections<'input>>,
);

/// Providers whose declaration interfaces, including non-generic aliases,
/// are canonical. Later route and bridge obligations still prevent import.
pub struct TypeAliasValidatedCrossConeHirClosure<'input>(
    ValidatedSurfaceClosure<TypeAliasValidatedCrossConeHirFrontSections<'input>>,
);

macro_rules! impl_surface_closure_accessors {
    ($state:ident, $front:ident) => {
        impl $state<'_> {
            pub const fn current(&self) -> ConeIdentity {
                self.0.current
            }

            pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
                self.0.target
            }

            pub fn direct_providers(&self) -> &[ConeIdentity] {
                &self.0.direct
            }

            pub fn dependency_first(&self) -> impl ExactSizeIterator<Item = &$front<'_>> {
                self.0.dependency_first.iter()
            }

            pub fn artifact(&self, identity: ConeIdentity) -> Option<&$front<'_>> {
                self.0
                    .positions
                    .get(&identity)
                    .map(|position| &self.0.dependency_first[*position])
            }

            pub fn role(&self, identity: ConeIdentity) -> Option<CrossConeProviderRole> {
                self.0.positions.get(&identity).map(|_| {
                    if self.0.direct.binary_search(&identity).is_ok() {
                        CrossConeProviderRole::Direct
                    } else {
                        CrossConeProviderRole::Support
                    }
                })
            }

            pub fn dependency_count(&self, identity: ConeIdentity) -> Option<usize> {
                self.0
                    .positions
                    .get(&identity)
                    .map(|position| self.0.dependency_positions[*position].len())
            }
        }
    };
}

impl_surface_closure_accessors!(
    InternallyClosedCrossConeHirClosure,
    InternallyClosedCrossConeHirFrontSections
);
impl_surface_closure_accessors!(
    DefinitionSourceValidatedCrossConeHirClosure,
    DefinitionSourceValidatedCrossConeHirFrontSections
);
impl_surface_closure_accessors!(
    NominalValidatedCrossConeHirClosure,
    NominalValidatedCrossConeHirFrontSections
);
impl_surface_closure_accessors!(
    PropertyValidatedCrossConeHirClosure,
    PropertyValidatedCrossConeHirFrontSections
);
impl_surface_closure_accessors!(
    CallableValidatedCrossConeHirClosure,
    CallableValidatedCrossConeHirFrontSections
);
impl_surface_closure_accessors!(
    TypeAliasValidatedCrossConeHirClosure,
    TypeAliasValidatedCrossConeHirFrontSections
);
impl_surface_closure_accessors!(
    SourceInterfaceValidatedCrossConeHirClosure,
    SourceInterfaceValidatedCrossConeHirFrontSections
);
impl_surface_closure_accessors!(
    ConstValidatedCrossConeHirClosure,
    ConstValidatedCrossConeHirFrontSections
);

impl<'input> ConstValidatedCrossConeHirClosure<'input> {
    pub(super) fn route_validation_parts(
        &self,
    ) -> (
        &[ConstValidatedCrossConeHirFrontSections<'input>],
        &[Vec<usize>],
    ) {
        (&self.0.dependency_first, &self.0.dependency_positions)
    }

    pub(super) fn hir_semantic_validation_parts(
        &mut self,
    ) -> (
        &mut [ConstValidatedCrossConeHirFrontSections<'input>],
        &[Vec<usize>],
    ) {
        (&mut self.0.dependency_first, &self.0.dependency_positions)
    }
}

impl<'input> HirProductionValidatedCrossConeHirClosure<'input> {
    /// Validates every provider's exact section-internal HIR relations before
    /// any table is exposed as cross-provider semantic authority.
    pub fn validate_internal_hir_closures(
        self,
    ) -> Result<InternallyClosedCrossConeHirClosure<'input>, CrossConeClosureInternalHirError> {
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
            CrossConeClosureInternalHirError::Allocation {
                requested_slots: artifact_count,
            }
        })?;
        for front in dependency_first {
            let identity = front.identity();
            validated.push(front.validate_internal_hir_closures().map_err(|source| {
                CrossConeClosureInternalHirError::Artifact {
                    identity,
                    source: Box::new(source),
                }
            })?);
        }
        Ok(InternallyClosedCrossConeHirClosure(
            ValidatedSurfaceClosure {
                current,
                target,
                direct,
                dependency_first: validated,
                positions,
                dependency_positions,
            },
        ))
    }
}

impl<'input> DefinitionSourceValidatedCrossConeHirClosure<'input> {
    /// Validates public nominal declarations dependency-first and exposes only
    /// each provider's transitive dependency closure.
    pub fn validate_nominal_surfaces(
        self,
    ) -> Result<NominalValidatedCrossConeHirClosure<'input>, CrossConeClosureNominalSurfaceError>
    {
        let ValidatedSurfaceClosure {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self.0;
        let artifact_count = dependency_first.len();
        let mut validated = Vec::new();
        validated.try_reserve_exact(artifact_count).map_err(|_| {
            CrossConeClosureNominalSurfaceError::Allocation {
                requested_slots: artifact_count,
            }
        })?;
        for (position, front) in dependency_first.into_iter().enumerate() {
            let identity = front.identity();
            let dependencies = nominal_dependencies(position, &dependency_positions, &validated)
                .map_err(|requested_slots| {
                    CrossConeClosureNominalSurfaceError::AuthorityAllocation {
                        identity,
                        requested_slots,
                    }
                })?;
            validated.push(
                front
                    .validate_nominal_surface(dependencies)
                    .map_err(|source| CrossConeClosureNominalSurfaceError::Artifact {
                        identity,
                        source: Box::new(source),
                    })?,
            );
        }
        Ok(NominalValidatedCrossConeHirClosure(
            ValidatedSurfaceClosure {
                current,
                target,
                direct,
                dependency_first: validated,
                positions,
                dependency_positions,
            },
        ))
    }
}

impl<'input> NominalValidatedCrossConeHirClosure<'input> {
    pub fn validate_property_surfaces(
        self,
    ) -> Result<PropertyValidatedCrossConeHirClosure<'input>, CrossConeClosurePropertySurfaceError>
    {
        let ValidatedSurfaceClosure {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self.0;
        let artifact_count = dependency_first.len();
        let mut validated = Vec::new();
        validated.try_reserve_exact(artifact_count).map_err(|_| {
            CrossConeClosurePropertySurfaceError::Allocation {
                requested_slots: artifact_count,
            }
        })?;
        for (position, front) in dependency_first.into_iter().enumerate() {
            let identity = front.identity();
            let dependencies = nominal_dependencies(position, &dependency_positions, &validated)
                .map_err(|requested_slots| {
                    CrossConeClosurePropertySurfaceError::AuthorityAllocation {
                        identity,
                        requested_slots,
                    }
                })?;
            validated.push(
                front
                    .validate_property_surface(dependencies)
                    .map_err(|source| CrossConeClosurePropertySurfaceError::Artifact {
                        identity,
                        source: Box::new(source),
                    })?,
            );
        }
        Ok(PropertyValidatedCrossConeHirClosure(
            ValidatedSurfaceClosure {
                current,
                target,
                direct,
                dependency_first: validated,
                positions,
                dependency_positions,
            },
        ))
    }
}

impl<'input> PropertyValidatedCrossConeHirClosure<'input> {
    pub fn validate_callable_surfaces(
        self,
    ) -> Result<CallableValidatedCrossConeHirClosure<'input>, CrossConeClosureCallableSurfaceError>
    {
        let ValidatedSurfaceClosure {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self.0;
        let artifact_count = dependency_first.len();
        let mut validated = Vec::new();
        validated.try_reserve_exact(artifact_count).map_err(|_| {
            CrossConeClosureCallableSurfaceError::Allocation {
                requested_slots: artifact_count,
            }
        })?;
        for (position, front) in dependency_first.into_iter().enumerate() {
            let identity = front.identity();
            let dependencies = nominal_dependencies(position, &dependency_positions, &validated)
                .map_err(|requested_slots| {
                    CrossConeClosureCallableSurfaceError::AuthorityAllocation {
                        identity,
                        requested_slots,
                    }
                })?;
            validated.push(
                front
                    .validate_callable_surface(dependencies)
                    .map_err(|source| CrossConeClosureCallableSurfaceError::Artifact {
                        identity,
                        source: Box::new(source),
                    })?,
            );
        }
        Ok(CallableValidatedCrossConeHirClosure(
            ValidatedSurfaceClosure {
                current,
                target,
                direct,
                dependency_first: validated,
                positions,
                dependency_positions,
            },
        ))
    }
}

impl<'input> CallableValidatedCrossConeHirClosure<'input> {
    pub fn validate_type_alias_surfaces(
        self,
    ) -> Result<TypeAliasValidatedCrossConeHirClosure<'input>, CrossConeClosureTypeAliasSurfaceError>
    {
        let ValidatedSurfaceClosure {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self.0;
        let artifact_count = dependency_first.len();
        let mut validated = Vec::new();
        validated.try_reserve_exact(artifact_count).map_err(|_| {
            CrossConeClosureTypeAliasSurfaceError::Allocation {
                requested_slots: artifact_count,
            }
        })?;
        for (position, front) in dependency_first.into_iter().enumerate() {
            let identity = front.identity();
            let dependencies = nominal_dependencies(position, &dependency_positions, &validated)
                .map_err(|requested_slots| {
                    CrossConeClosureTypeAliasSurfaceError::AuthorityAllocation {
                        identity,
                        requested_slots,
                    }
                })?;
            validated.push(
                front
                    .validate_type_alias_surface(dependencies)
                    .map_err(|source| CrossConeClosureTypeAliasSurfaceError::Artifact {
                        identity,
                        source: Box::new(source),
                    })?,
            );
        }
        Ok(TypeAliasValidatedCrossConeHirClosure(
            ValidatedSurfaceClosure {
                current,
                target,
                direct,
                dependency_first: validated,
                positions,
                dependency_positions,
            },
        ))
    }
}
