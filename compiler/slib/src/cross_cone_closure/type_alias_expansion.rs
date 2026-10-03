//! Dependency-first type-alias expansion from checked declaration references.

use super::{CrossConeProviderRole, ExternalReferenceValidatedCrossConeHirClosure};
use crate::ConstValidatedCrossConeHirFrontSections;
use scoop_hir::{
    CanonicalTypeAliasExpansionsV1, CrossConeHirInterfaceSectionV1, ExternalHirTargetV1,
    TypeAliasExpansionError, TypeAliasTargetV1,
};
use scoop_identity::{
    ConeIdentity, PersistentTypeAliasId, SourceDeclarationKey, ValidatedIdentityGraph,
};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_wire::{WireError, WirePath};
use std::fmt;

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
    pub fn validate_and_expand_type_aliases(
        mut self,
    ) -> Result<TypeAliasExpandedCrossConeHirClosure<'input>, CrossConeClosureTypeAliasExpansionError>
    {
        let mut expansions = Vec::new();
        let (artifacts, dependency_positions) = self.surfaces_mut().hir_semantic_validation_parts();
        for (position, artifact) in artifacts.iter_mut().enumerate() {
            let identity = artifact.identity();
            let (identities, interface) = artifact.hir_semantic_parts();
            validate_alias_targets(identity, interface, identities).map_err(|source| {
                CrossConeClosureTypeAliasExpansionError::ArtifactReferences { identity, source }
            })?;
            let reachable = crate::dependency_reachability::transitive_positions(
                position,
                dependency_positions,
            )
            .map_err(|source| CrossConeClosureTypeAliasExpansionError::Resource {
                identity,
                source,
            })?;
            let dependencies = reachable
                .iter()
                .map(|position| &expansions[*position])
                .collect::<Vec<_>>();
            let expanded = interface
                .type_aliases()
                .expand_alias_closure(&dependencies, &WirePath::root().field(5))
                .map_err(
                    |source| CrossConeClosureTypeAliasExpansionError::ArtifactExpansion {
                        identity,
                        source,
                    },
                )?;
            scoop_wire::allocation::try_reserve(&mut expansions, 1, &WirePath::root()).map_err(
                |source| CrossConeClosureTypeAliasExpansionError::Resource { identity, source },
            )?;
            expansions.push(expanded);
        }
        Ok(TypeAliasExpandedCrossConeHirClosure {
            references: self,
            expansions,
        })
    }
}

pub(crate) fn validate_alias_targets(
    current: ConeIdentity,
    interface: &CrossConeHirInterfaceSectionV1,
    identities: &ValidatedIdentityGraph,
) -> Result<(), CrossConeHirAliasReferenceError> {
    use CrossConeHirAliasReferenceError as Error;
    for record in interface.type_aliases().records() {
        let TypeAliasTargetV1::Alias(target) = record.target() else {
            continue;
        };
        let source = record.alias();
        let target = *target;
        let declaration = identities
            .canonical_key::<_, SourceDeclarationKey>(target)
            .map_err(|_| Error::MissingDeclaration { source, target })?;
        if declaration.origin() == current {
            if interface.type_aliases().get(target).is_none() {
                return Err(Error::MissingCurrentPublicTarget { source, target });
            }
        } else if interface
            .external_references()
            .get(ExternalHirTargetV1::TypeAlias(target))
            .is_none()
        {
            return Err(Error::MissingForeignReference { source, target });
        }
    }
    Ok(())
}

#[derive(Debug)]
pub enum CrossConeHirAliasReferenceError {
    MissingDeclaration {
        source: PersistentTypeAliasId,
        target: PersistentTypeAliasId,
    },
    MissingCurrentPublicTarget {
        source: PersistentTypeAliasId,
        target: PersistentTypeAliasId,
    },
    MissingForeignReference {
        source: PersistentTypeAliasId,
        target: PersistentTypeAliasId,
    },
}
impl fmt::Display for CrossConeHirAliasReferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDeclaration { source, target } => write!(
                f,
                "type alias {source} has no declaration for target {target}"
            ),
            Self::MissingCurrentPublicTarget { source, target } => write!(
                f,
                "type alias {source} targets current non-public alias {target}"
            ),
            Self::MissingForeignReference { source, target } => write!(
                f,
                "type alias {source} targets foreign alias {target} without an external reference"
            ),
        }
    }
}
impl std::error::Error for CrossConeHirAliasReferenceError {}

#[derive(Debug)]
pub enum CrossConeClosureTypeAliasExpansionError {
    Resource {
        identity: ConeIdentity,
        source: WireError,
    },
    ArtifactReferences {
        identity: ConeIdentity,
        source: CrossConeHirAliasReferenceError,
    },
    ArtifactExpansion {
        identity: ConeIdentity,
        source: TypeAliasExpansionError,
    },
}
impl fmt::Display for CrossConeClosureTypeAliasExpansionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource { identity, source } => {
                write!(f, "cannot read type aliases for {identity}: {source}")
            }
            Self::ArtifactReferences { identity, source } => {
                write!(f, "invalid type-alias reference for {identity}: {source}")
            }
            Self::ArtifactExpansion { identity, source } => {
                write!(f, "cannot expand type aliases for {identity}: {source}")
            }
        }
    }
}
impl std::error::Error for CrossConeClosureTypeAliasExpansionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource { source, .. } => Some(source),
            Self::ArtifactReferences { source, .. } => Some(source),
            Self::ArtifactExpansion { source, .. } => Some(source),
        }
    }
}
#[cfg(test)]
mod tests;
