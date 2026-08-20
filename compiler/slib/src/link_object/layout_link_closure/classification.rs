use scoop_identity::ConeIdentity;
use scoop_lir::{CanonicalExternalShapeLinkImportsV1, LirTargetProfile};
use scoop_wire::WirePath;

use super::{ExternalShapeUndefinedUseV1, LayoutLinkClosureError};
use crate::link_object::{
    CanonicalUndefinedRelocationUseV1, StrongRelocationBindingV1, StrongRelocationResolutionV1,
    VerifiedCrossConeStrongRequirementClosureV1, cross_cone_link_closure::use_key,
};

mod symbols;
use symbols::ImportSymbolIndex;

/// Keeps the resolved callable relocations and classifies the remaining shape
/// uses while preserving every unmatched native or runtime candidate.
#[derive(Debug)]
pub struct VerifiedExternalShapeRequirementClosureV1<'a> {
    legacy: &'a VerifiedCrossConeStrongRequirementClosureV1,
    imports: &'a CanonicalExternalShapeLinkImportsV1,
    requirements: Vec<ExternalShapeUndefinedUseV1>,
    remaining: Vec<&'a StrongRelocationBindingV1>,
}

impl<'a> VerifiedExternalShapeRequirementClosureV1<'a> {
    pub const fn producer(&self) -> ConeIdentity {
        self.legacy.producer()
    }
    pub const fn target(&self) -> LirTargetProfile {
        self.legacy.target()
    }
    pub const fn legacy_closure(&self) -> &'a VerifiedCrossConeStrongRequirementClosureV1 {
        self.legacy
    }
    pub const fn semantic_imports(&self) -> &'a CanonicalExternalShapeLinkImportsV1 {
        self.imports
    }
    pub fn requirements(&self) -> &[ExternalShapeUndefinedUseV1] {
        &self.requirements
    }
    pub fn remaining_external_candidates(&self) -> &[&'a StrongRelocationBindingV1] {
        &self.remaining
    }
}

pub fn verify_external_shape_requirements_v1<'a>(
    legacy: &'a VerifiedCrossConeStrongRequirementClosureV1,
    consumer: ConeIdentity,
    imports: &'a CanonicalExternalShapeLinkImportsV1,
) -> Result<VerifiedExternalShapeRequirementClosureV1<'a>, LayoutLinkClosureError> {
    if legacy.producer() != consumer {
        return Err(LayoutLinkClosureError::ConsumerMismatch {
            objects: legacy.producer(),
            selection: consumer,
        });
    }
    let symbols = ImportSymbolIndex::new(imports, legacy.target())?;
    let classified = classify(legacy.remaining_external_candidates(), &symbols)?;
    Ok(VerifiedExternalShapeRequirementClosureV1 {
        legacy,
        imports,
        requirements: classified.requirements,
        remaining: classified.remaining,
    })
}

struct Classification<'a> {
    requirements: Vec<ExternalShapeUndefinedUseV1>,
    remaining: Vec<&'a StrongRelocationBindingV1>,
}

fn classify<'a>(
    candidates: &'a [StrongRelocationBindingV1],
    symbols: &ImportSymbolIndex,
) -> Result<Classification<'a>, LayoutLinkClosureError> {
    let path = WirePath::root();

    let mut requirements = Vec::new();
    let mut remaining = Vec::new();
    scoop_wire::allocation::try_reserve(&mut requirements, candidates.len(), &path)?;
    scoop_wire::allocation::try_reserve(&mut remaining, candidates.len(), &path)?;
    for binding in candidates {
        if !matches!(
            binding.resolution(),
            StrongRelocationResolutionV1::ExternalCandidate { .. }
        ) {
            return Err(LayoutLinkClosureError::NonExternalRemainder {
                member: binding.source_member(),
                atom: binding.containing_atom(),
            });
        }
        if let Some(index) = symbols.find(binding.symbol())? {
            requirements.push(ExternalShapeUndefinedUseV1 {
                use_site: CanonicalUndefinedRelocationUseV1::from(binding),
                import_index: index,
            });
        } else {
            remaining.push(binding);
        }
    }
    requirements.sort_unstable_by_key(|requirement| use_key(requirement.use_site()));
    for pair in requirements.windows(2) {
        if use_key(pair[0].use_site()) == use_key(pair[1].use_site()) {
            let use_site = pair[1].use_site();
            return Err(LayoutLinkClosureError::DuplicateUse {
                member: use_site.source_member(),
                atom: use_site.containing_atom(),
                offset: use_site.offset_within_atom(),
                target_slot: use_site.target_slot(),
            });
        }
    }
    Ok(Classification {
        requirements,
        remaining,
    })
}

#[cfg(test)]
#[path = "tests/classification.rs"]
mod tests;
