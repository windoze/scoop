//! Closure-wide non-generic type-alias authorization and expansion.

use std::fmt;

use scoop_hir::{
    CanonicalTypeAliasExpansionsV1, CrossConeHirInterfaceSectionV1,
    ExternalHirReferenceSemanticAuthority, ExternalHirTargetV1, TypeAliasClosureAuthority,
    TypeAliasExpansionError, TypeAliasInterfaceRecordV1, TypeAliasTargetV1,
};
use scoop_identity::{ConeIdentity, PersistentTypeAliasId};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_wire::WirePath;

use super::{
    CrossConeProviderRole, ExternalReferenceValidatedCrossConeHirClosure,
    route_validation::{
        CanonicalCrossConeRouteAuthority, CrossConeHirReferenceAuthorityError, RouteAuthorityInputs,
    },
    surface_validation::transitive_dependency_positions,
};
use crate::ConstValidatedCrossConeHirFrontSections;

type AuthorizedAliasEdge = (PersistentTypeAliasId, PersistentTypeAliasId);

/// An externally closed HIR graph whose public non-generic aliases have been
/// expanded to final signature types without exposing transitive providers as
/// ordinary lookup candidates.
pub struct TypeAliasExpandedCrossConeHirClosure<'input> {
    references: ExternalReferenceValidatedCrossConeHirClosure<'input>,
    expansions: Vec<CanonicalTypeAliasExpansionsV1>,
}

impl<'input> TypeAliasExpandedCrossConeHirClosure<'input> {
    pub const fn current(&self) -> ConeIdentity {
        self.references.current()
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.references.target_selection()
    }

    pub fn direct_providers(&self) -> &[ConeIdentity] {
        self.references.direct_providers()
    }

    pub fn dependency_first(
        &self,
    ) -> impl ExactSizeIterator<Item = &ConstValidatedCrossConeHirFrontSections<'_>> {
        self.references.dependency_first()
    }

    pub fn artifact(
        &self,
        identity: ConeIdentity,
    ) -> Option<&ConstValidatedCrossConeHirFrontSections<'_>> {
        self.references.artifact(identity)
    }

    pub fn role(&self, identity: ConeIdentity) -> Option<CrossConeProviderRole> {
        self.references.role(identity)
    }

    pub fn dependency_count(&self, identity: ConeIdentity) -> Option<usize> {
        self.references.dependency_count(identity)
    }

    pub fn type_alias_expansions(
        &self,
        identity: ConeIdentity,
    ) -> Option<&CanonicalTypeAliasExpansionsV1> {
        self.references
            .dependency_first()
            .position(|artifact| artifact.identity() == identity)
            .map(|position| &self.expansions[position])
    }

    pub(super) fn into_mir_bridge_parts(
        self,
    ) -> (
        ExternalReferenceValidatedCrossConeHirClosure<'input>,
        Vec<CanonicalTypeAliasExpansionsV1>,
    ) {
        (self.references, self.expansions)
    }
}

impl<'input> ExternalReferenceValidatedCrossConeHirClosure<'input> {
    /// Derives authorized alias edges from the already validated external
    /// reference closure, then expands alias chains dependency-first.
    pub fn validate_and_expand_type_aliases(
        mut self,
    ) -> Result<TypeAliasExpandedCrossConeHirClosure<'input>, CrossConeClosureTypeAliasExpansionError>
    {
        let artifact_count = self.dependency_first().len();
        let mut expansions = Vec::new();
        expansions.try_reserve_exact(artifact_count).map_err(|_| {
            CrossConeClosureTypeAliasExpansionError::Allocation {
                requested_slots: artifact_count,
            }
        })?;
        let mut authorized_by_artifact = Vec::<Vec<AuthorizedAliasEdge>>::new();
        authorized_by_artifact
            .try_reserve_exact(artifact_count)
            .map_err(|_| CrossConeClosureTypeAliasExpansionError::Allocation {
                requested_slots: artifact_count,
            })?;

        {
            let (artifacts, dependency_positions) =
                self.surfaces_mut().hir_semantic_validation_parts();
            for position in 0..artifacts.len() {
                let reachable = transitive_dependency_positions(position, dependency_positions);
                let (previous, current_and_later) = artifacts.split_at_mut(position);
                let current = &mut current_and_later[0];
                let identity = current.identity();

                let route_inputs = RouteAuthorityInputs::try_new(
                    previous,
                    &dependency_positions[position],
                    &reachable,
                )
                .map_err(|requested_slots| {
                    CrossConeClosureTypeAliasExpansionError::AuthorityAllocation {
                        identity,
                        requested_slots,
                    }
                })?;

                let (identities, interface, meter) = current.hir_semantic_parts();
                let mut route_authority = CanonicalCrossConeRouteAuthority::try_new(
                    identity,
                    identities,
                    interface,
                    route_inputs.direct(),
                    route_inputs.providers(),
                    route_inputs.closure_node_count(),
                )
                .map_err(|requested_slots| {
                    CrossConeClosureTypeAliasExpansionError::AuthorityAllocation {
                        identity,
                        requested_slots,
                    }
                })?;
                let path = WirePath::root();
                let authorized = validate_alias_authority(interface, &mut route_authority)
                    .map_err(|source| {
                        CrossConeClosureTypeAliasExpansionError::ArtifactAuthority {
                            identity,
                            source: Box::new(source),
                        }
                    })?;
                drop(route_authority);
                drop(route_inputs);

                let mut alias_providers = Vec::new();
                alias_providers
                    .try_reserve_exact(reachable.len())
                    .map_err(
                        |_| CrossConeClosureTypeAliasExpansionError::AuthorityAllocation {
                            identity,
                            requested_slots: reachable.len(),
                        },
                    )?;
                alias_providers.extend(reachable.iter().map(|dependency| AliasProviderView {
                    interface: previous[*dependency].hir_interface(),
                    authorized: &authorized_by_artifact[*dependency],
                }));
                let alias_authority =
                    CanonicalTypeAliasClosureAuthority::try_new(&authorized, &alias_providers)
                        .map_err(|requested_slots| {
                            CrossConeClosureTypeAliasExpansionError::AuthorityAllocation {
                                identity,
                                requested_slots,
                            }
                        })?;
                let expanded = interface
                    .type_aliases()
                    .expand_alias_closure(&alias_authority, meter, &path.field(5))
                    .map_err(|source| {
                        CrossConeClosureTypeAliasExpansionError::ArtifactExpansion {
                            identity,
                            source,
                        }
                    })?;
                authorized_by_artifact.push(authorized);
                expansions.push(expanded);
            }
        }

        Ok(TypeAliasExpandedCrossConeHirClosure {
            references: self,
            expansions,
        })
    }
}

fn validate_alias_authority(
    interface: &CrossConeHirInterfaceSectionV1,
    authority: &mut CanonicalCrossConeRouteAuthority<'_>,
) -> Result<Vec<AuthorizedAliasEdge>, CrossConeHirAliasAuthorityValidationError> {
    let aliases = interface.type_aliases();
    let mut authorized = Vec::new();
    authorized
        .try_reserve_exact(aliases.records().len())
        .map_err(|_| CrossConeHirAliasAuthorityValidationError::Allocation {
            requested_slots: aliases.records().len(),
        })?;
    let current = authority.current_cone();
    for record in aliases.records() {
        let TypeAliasTargetV1::Alias(target) = record.target() else {
            continue;
        };
        let source = record.alias();
        let target = *target;
        let origin = authority
            .external_hir_target_origin(ExternalHirTargetV1::TypeAlias(target))
            .map_err(
                |error| CrossConeHirAliasAuthorityValidationError::TargetOrigin {
                    source,
                    target,
                    error,
                },
            )?;
        if origin == current {
            if aliases.get(target).is_none() {
                return Err(
                    CrossConeHirAliasAuthorityValidationError::MissingCurrentPublicTarget {
                        source,
                        target,
                    },
                );
            }
        } else if interface
            .external_references()
            .get(ExternalHirTargetV1::TypeAlias(target))
            .is_none()
        {
            return Err(
                CrossConeHirAliasAuthorityValidationError::MissingForeignReference {
                    source,
                    target,
                },
            );
        }
        authorized.push((source, target));
    }
    Ok(authorized)
}

#[derive(Clone, Copy)]
struct AliasProviderView<'a> {
    interface: &'a CrossConeHirInterfaceSectionV1,
    authorized: &'a [AuthorizedAliasEdge],
}

struct CanonicalTypeAliasClosureAuthority<'a> {
    records: Vec<(PersistentTypeAliasId, &'a TypeAliasInterfaceRecordV1)>,
    authorized: Vec<AuthorizedAliasEdge>,
}

impl<'a> CanonicalTypeAliasClosureAuthority<'a> {
    fn try_new(
        current_authorized: &[AuthorizedAliasEdge],
        providers: &[AliasProviderView<'a>],
    ) -> Result<Self, usize> {
        let record_count = providers.iter().try_fold(0_usize, |count, provider| {
            count.checked_add(provider.interface.type_aliases().records().len())
        });
        let Some(record_count) = record_count else {
            return Err(usize::MAX);
        };
        let mut records = Vec::new();
        records
            .try_reserve_exact(record_count)
            .map_err(|_| record_count)?;
        for provider in providers {
            records.extend(
                provider
                    .interface
                    .type_aliases()
                    .records()
                    .iter()
                    .map(|record| (record.alias(), record)),
            );
        }
        records.sort_unstable_by_key(|(alias, _)| *alias);
        records.dedup_by_key(|(alias, _)| *alias);

        let authorized_count = providers
            .iter()
            .try_fold(current_authorized.len(), |count, provider| {
                count.checked_add(provider.authorized.len())
            });
        let Some(authorized_count) = authorized_count else {
            return Err(usize::MAX);
        };
        let mut authorized = Vec::new();
        authorized
            .try_reserve_exact(authorized_count)
            .map_err(|_| authorized_count)?;
        authorized.extend_from_slice(current_authorized);
        for provider in providers {
            authorized.extend_from_slice(provider.authorized);
        }
        authorized.sort_unstable();
        authorized.dedup();

        Ok(Self {
            records,
            authorized,
        })
    }
}

impl TypeAliasClosureAuthority for CanonicalTypeAliasClosureAuthority<'_> {
    fn external_type_alias(
        &self,
        alias: PersistentTypeAliasId,
    ) -> Option<&TypeAliasInterfaceRecordV1> {
        self.records
            .binary_search_by_key(&alias, |(candidate, _)| *candidate)
            .ok()
            .map(|position| self.records[position].1)
    }

    fn is_type_alias_edge_authorized(
        &self,
        source: PersistentTypeAliasId,
        target: PersistentTypeAliasId,
    ) -> bool {
        self.authorized.binary_search(&(source, target)).is_ok()
    }
}

#[derive(Debug)]
pub enum CrossConeHirAliasAuthorityValidationError {
    TargetOrigin {
        source: PersistentTypeAliasId,
        target: PersistentTypeAliasId,
        error: CrossConeHirReferenceAuthorityError,
    },
    MissingCurrentPublicTarget {
        source: PersistentTypeAliasId,
        target: PersistentTypeAliasId,
    },
    MissingForeignReference {
        source: PersistentTypeAliasId,
        target: PersistentTypeAliasId,
    },
    Allocation {
        requested_slots: usize,
    },
}

impl fmt::Display for CrossConeHirAliasAuthorityValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetOrigin {
                source,
                target,
                error,
            } => write!(
                formatter,
                "type alias {source} target {target} has no canonical origin: {error}"
            ),
            Self::MissingCurrentPublicTarget { source, target } => write!(
                formatter,
                "type alias {source} targets current non-public alias {target}"
            ),
            Self::MissingForeignReference { source, target } => write!(
                formatter,
                "type alias {source} targets foreign alias {target} without an external reference"
            ),
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} authorized type-alias edges"
            ),
        }
    }
}

impl std::error::Error for CrossConeHirAliasAuthorityValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TargetOrigin { error, .. } => Some(error),
            Self::MissingCurrentPublicTarget { .. }
            | Self::MissingForeignReference { .. }
            | Self::Allocation { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum CrossConeClosureTypeAliasExpansionError {
    Allocation {
        requested_slots: usize,
    },
    AuthorityAllocation {
        identity: ConeIdentity,
        requested_slots: usize,
    },
    ArtifactAuthority {
        identity: ConeIdentity,
        source: Box<CrossConeHirAliasAuthorityValidationError>,
    },
    ArtifactExpansion {
        identity: ConeIdentity,
        source: TypeAliasExpansionError,
    },
}

impl fmt::Display for CrossConeClosureTypeAliasExpansionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} cross-Cone type-alias expansion slots"
            ),
            Self::AuthorityAllocation {
                identity,
                requested_slots,
            } => write!(
                formatter,
                "cannot allocate {requested_slots} type-alias authority slots for {identity}"
            ),
            Self::ArtifactAuthority { identity, source } => {
                write!(
                    formatter,
                    "invalid type-alias authority for {identity}: {source}"
                )
            }
            Self::ArtifactExpansion { identity, source } => {
                write!(
                    formatter,
                    "cannot expand type aliases for {identity}: {source}"
                )
            }
        }
    }
}

impl std::error::Error for CrossConeClosureTypeAliasExpansionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ArtifactAuthority { source, .. } => Some(source.as_ref()),
            Self::ArtifactExpansion { source, .. } => Some(source),
            Self::Allocation { .. } | Self::AuthorityAllocation { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests;
