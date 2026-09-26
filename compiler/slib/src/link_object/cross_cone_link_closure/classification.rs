//! One dependency classification pass for selected callable and descriptor uses.

use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::ConeIdentity;
use scoop_lir::LirTargetProfile;

use super::{CrossConeLinkSemanticImportSetV1, CrossConeUndefinedRequirementV1, use_key};
use crate::link_object::{
    CanonicalDefinedLinkSymbolOwnerSetV1, CanonicalUndefinedRelocationUseV1,
    StrongRelocationBindingV1, StrongRelocationResolutionV1,
    VerifiedCurrentConeStrongRelocationClosureV1,
};

mod dependencies;
mod errors;
use dependencies::{dependency_index, resolve_owner};
pub use errors::CrossConeStrongRequirementValidationError;

/// Disjoint uses derived from one relocation closure and one provider index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedCrossConeStrongRequirementClosureV1 {
    target: LirTargetProfile,
    strong_closure: VerifiedCurrentConeStrongRelocationClosureV1,
    dependency_owners: Vec<CanonicalDefinedLinkSymbolOwnerSetV1>,
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
    pub fn dependency_owners(&self) -> &[CanonicalDefinedLinkSymbolOwnerSetV1] {
        &self.dependency_owners
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
    dependency_owners: &[CanonicalDefinedLinkSymbolOwnerSetV1],
) -> Result<VerifiedCrossConeStrongRequirementClosureV1, CrossConeStrongRequirementValidationError>
{
    let consumer = strong_closure.producer();
    classify(
        target,
        strong_closure,
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
    dependency_owners: &[CanonicalDefinedLinkSymbolOwnerSetV1],
    bridge: &scoop_lir::CrossConeLirBridgeSectionV1,
) -> Result<VerifiedCrossConeStrongRequirementClosureV1, CrossConeStrongRequirementValidationError>
{
    let semantic_imports = CrossConeLinkSemanticImportSetV1::from_lir_bridge(bridge)
        .map_err(CrossConeStrongRequirementValidationError::SemanticImports)?;
    classify(target, strong_closure, dependency_owners, semantic_imports)
}

#[derive(Clone, Copy)]
enum SymbolUse {
    Callable { index: u32 },
}

fn classify(
    target: LirTargetProfile,
    strong_closure: VerifiedCurrentConeStrongRelocationClosureV1,
    dependency_owners: &[CanonicalDefinedLinkSymbolOwnerSetV1],
    semantic_imports: CrossConeLinkSemanticImportSetV1,
) -> Result<VerifiedCrossConeStrongRequirementClosureV1, CrossConeStrongRequirementValidationError>
{
    let consumer = strong_closure.producer();
    for bridge in [semantic_imports.consumer()] {
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
            Some(SymbolUse::Callable { index }) => {
                requirements.push(CrossConeUndefinedRequirementV1 {
                    use_site: CanonicalUndefinedRelocationUseV1::from(binding),
                    import_index: index,
                });
            }
            None => remaining_external_candidates.push(binding.clone()),
        }
    }
    requirements.sort_unstable_by_key(|requirement| use_key(requirement.use_site()));
    let mut used_sites = BTreeSet::new();
    for use_site in requirements
        .iter()
        .map(CrossConeUndefinedRequirementV1::use_site)
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
        dependency_owners: dependency_owners.to_vec(),
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
