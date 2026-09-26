//! One dependency classification pass for selected callable and descriptor uses.

use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::ConeIdentity;
use scoop_lir::{LirTargetProfile, StrongExternalLirBridgeSurfaceV1, StrongExternalLirBridgeV1};

use super::{CrossConeLinkSemanticImportSetV1, CrossConeUndefinedRequirementV1, use_key};
use crate::SlibMemberId;
use crate::link_object::{
    CanonicalDefinedLinkSymbolOwnerSetV1, CanonicalUndefinedRelocationUseV1,
    StrongDefinitionOwnerV1, StrongRelocationBindingV1, StrongRelocationResolutionV1,
    VerifiedCurrentConeStrongRelocationClosureV1,
};

mod dependencies;
mod errors;
use dependencies::{dependency_index, expected_owner, expected_symbol, resolve_owner};
pub use errors::CrossConeStrongRequirementValidationError;

/// A physical use of an explicit strong-production dependency subject.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyStrongRequirementUseV1 {
    use_site: CanonicalUndefinedRelocationUseV1,
    provider: ConeIdentity,
    member: SlibMemberId,
    owner: StrongDefinitionOwnerV1,
    bridge: StrongExternalLirBridgeV1,
}

impl DependencyStrongRequirementUseV1 {
    pub const fn use_site(&self) -> &CanonicalUndefinedRelocationUseV1 {
        &self.use_site
    }
    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }
    pub const fn member(&self) -> SlibMemberId {
        self.member
    }
    pub const fn owner(&self) -> StrongDefinitionOwnerV1 {
        self.owner
    }
    pub const fn bridge(&self) -> &StrongExternalLirBridgeV1 {
        &self.bridge
    }
}

/// Disjoint uses derived from one relocation closure and one provider index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedCrossConeStrongRequirementClosureV1 {
    target: LirTargetProfile,
    strong_closure: VerifiedCurrentConeStrongRelocationClosureV1,
    external_bridges: StrongExternalLirBridgeSurfaceV1,
    dependency_owners: Vec<CanonicalDefinedLinkSymbolOwnerSetV1>,
    external_requirements: Vec<DependencyStrongRequirementUseV1>,
    semantic_imports: CrossConeLinkSemanticImportSetV1,
    requirements: Vec<CrossConeUndefinedRequirementV1>,
    remaining_external_candidates: Vec<StrongRelocationBindingV1>,
}

impl VerifiedCrossConeStrongRequirementClosureV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.strong_closure.producer()
    }
    pub const fn target(&self) -> LirTargetProfile {
        self.target
    }
    pub const fn strong_closure(&self) -> &VerifiedCurrentConeStrongRelocationClosureV1 {
        &self.strong_closure
    }
    pub const fn external_bridges(&self) -> &StrongExternalLirBridgeSurfaceV1 {
        &self.external_bridges
    }
    pub fn dependency_owners(&self) -> &[CanonicalDefinedLinkSymbolOwnerSetV1] {
        &self.dependency_owners
    }
    pub fn external_requirements(&self) -> &[DependencyStrongRequirementUseV1] {
        &self.external_requirements
    }
    pub const fn semantic_imports(&self) -> &CrossConeLinkSemanticImportSetV1 {
        &self.semantic_imports
    }
    pub fn requirements(&self) -> &[CrossConeUndefinedRequirementV1] {
        &self.requirements
    }
    pub fn remaining_external_candidates(&self) -> &[StrongRelocationBindingV1] {
        &self.remaining_external_candidates
    }
}

/// Uses the same verifier when the profile has no ordinary callable section.
pub fn verify_dependency_strong_requirements_v1(
    target: LirTargetProfile,
    strong_closure: VerifiedCurrentConeStrongRelocationClosureV1,
    external_bridges: StrongExternalLirBridgeSurfaceV1,
    dependency_owners: &[CanonicalDefinedLinkSymbolOwnerSetV1],
) -> Result<VerifiedCrossConeStrongRequirementClosureV1, CrossConeStrongRequirementValidationError>
{
    let consumer = strong_closure.producer();
    classify(
        target,
        strong_closure,
        external_bridges,
        dependency_owners,
        CrossConeLinkSemanticImportSetV1 {
            consumer,
            imports: Vec::new(),
        },
    )
}

pub fn verify_cross_cone_strong_requirements_v1(
    target: LirTargetProfile,
    strong_closure: VerifiedCurrentConeStrongRelocationClosureV1,
    external_bridges: StrongExternalLirBridgeSurfaceV1,
    dependency_owners: &[CanonicalDefinedLinkSymbolOwnerSetV1],
    bridge: &scoop_lir::CrossConeLirBridgeSectionV1,
) -> Result<VerifiedCrossConeStrongRequirementClosureV1, CrossConeStrongRequirementValidationError>
{
    let semantic_imports = CrossConeLinkSemanticImportSetV1::from_lir_bridge(bridge)
        .map_err(CrossConeStrongRequirementValidationError::SemanticImports)?;
    classify(
        target,
        strong_closure,
        external_bridges,
        dependency_owners,
        semantic_imports,
    )
}

#[derive(Clone, Copy)]
enum SymbolUse {
    External {
        index: usize,
        member: SlibMemberId,
        owner: StrongDefinitionOwnerV1,
    },
    Callable {
        index: u32,
    },
}

fn classify(
    target: LirTargetProfile,
    strong_closure: VerifiedCurrentConeStrongRelocationClosureV1,
    external_bridges: StrongExternalLirBridgeSurfaceV1,
    dependency_owners: &[CanonicalDefinedLinkSymbolOwnerSetV1],
    semantic_imports: CrossConeLinkSemanticImportSetV1,
) -> Result<VerifiedCrossConeStrongRequirementClosureV1, CrossConeStrongRequirementValidationError>
{
    let consumer = strong_closure.producer();
    for bridge in [external_bridges.producer(), semantic_imports.consumer()] {
        if consumer != bridge {
            return Err(
                CrossConeStrongRequirementValidationError::ConsumerMismatch {
                    object: consumer,
                    bridge,
                },
            );
        }
    }
    let dependencies = dependency_index(consumer, dependency_owners)?;
    let normalization = target.contract().native_symbol_normalization();
    let mut symbols = BTreeMap::new();
    for (index, bridge) in external_bridges.bridges().iter().enumerate() {
        let owner = expected_owner(bridge)?;
        let name = normalization
            .compiler_generated_object_symbol(expected_symbol(bridge).symbol().as_str())
            .into_bytes();
        let member = resolve_owner(&dependencies, bridge.provider(), &name, owner)?;
        insert_symbol(
            &mut symbols,
            name,
            SymbolUse::External {
                index,
                member,
                owner,
            },
        )?;
    }
    for (index, import) in semantic_imports.imports().iter().enumerate() {
        let name = normalization
            .compiler_generated_object_symbol(import.expected_symbol().symbol().as_str())
            .into_bytes();
        let owner = dependencies::callable_owner(import.target())?;
        resolve_owner(&dependencies, import.provider(), &name, owner)?;
        insert_symbol(
            &mut symbols,
            name,
            SymbolUse::Callable {
                index: index as u32,
            },
        )?;
    }

    let mut used_external = BTreeSet::new();
    let mut used_imports = BTreeSet::new();
    let mut external_requirements = Vec::new();
    let mut requirements = Vec::new();
    let mut remaining_external_candidates = Vec::new();
    for binding in strong_closure.bindings() {
        if !matches!(
            binding.resolution(),
            StrongRelocationResolutionV1::ExternalCandidate { .. }
        ) {
            continue;
        }
        match symbols.get(binding.symbol()).copied() {
            Some(SymbolUse::External {
                index,
                member,
                owner,
            }) => {
                used_external.insert(index);
                let bridge = &external_bridges.bridges()[index];
                external_requirements.push(DependencyStrongRequirementUseV1 {
                    use_site: CanonicalUndefinedRelocationUseV1::from(binding),
                    provider: bridge.provider(),
                    member,
                    owner,
                    bridge: bridge.clone(),
                });
            }
            Some(SymbolUse::Callable { index }) => {
                used_imports.insert(index);
                requirements.push(CrossConeUndefinedRequirementV1 {
                    use_site: CanonicalUndefinedRelocationUseV1::from(binding),
                    import_index: index,
                });
            }
            None => remaining_external_candidates.push(binding.clone()),
        }
    }
    for (index, bridge) in external_bridges.bridges().iter().enumerate() {
        if !used_external.contains(&index) {
            return Err(
                CrossConeStrongRequirementValidationError::UnusedExternalCallableBridge {
                    name: normalization
                        .compiler_generated_object_symbol(expected_symbol(bridge).symbol().as_str())
                        .into_bytes(),
                },
            );
        }
    }
    for (index, import) in semantic_imports.imports().iter().enumerate() {
        if !used_imports.contains(&(index as u32)) {
            return Err(CrossConeStrongRequirementValidationError::UnusedImport {
                import_index: index as u32,
                provider: import.provider(),
                target: import.target(),
            });
        }
    }
    requirements.sort_unstable_by_key(|requirement| use_key(requirement.use_site()));
    external_requirements.sort_unstable_by_key(|requirement| use_key(requirement.use_site()));
    let mut used_sites = BTreeSet::new();
    for use_site in requirements
        .iter()
        .map(CrossConeUndefinedRequirementV1::use_site)
        .chain(
            external_requirements
                .iter()
                .map(DependencyStrongRequirementUseV1::use_site),
        )
    {
        if !used_sites.insert(use_key(use_site)) {
            return Err(CrossConeStrongRequirementValidationError::DuplicateUse {
                member: use_site.source_member(),
                atom: use_site.containing_atom(),
                offset: use_site.offset_within_atom(),
                target_slot: use_site.target_slot(),
            });
        }
    }
    Ok(VerifiedCrossConeStrongRequirementClosureV1 {
        target,
        strong_closure,
        external_bridges,
        dependency_owners: dependency_owners.to_vec(),
        external_requirements,
        semantic_imports,
        requirements,
        remaining_external_candidates,
    })
}

fn insert_symbol(
    symbols: &mut BTreeMap<Vec<u8>, SymbolUse>,
    name: Vec<u8>,
    usage: SymbolUse,
) -> Result<(), CrossConeStrongRequirementValidationError> {
    if symbols.insert(name.clone(), usage).is_some() {
        return Err(
            CrossConeStrongRequirementValidationError::DuplicateNormalizedSymbol { symbol: name },
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;
