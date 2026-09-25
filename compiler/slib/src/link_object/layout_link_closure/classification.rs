use scoop_identity::ConeIdentity;
use scoop_lir::{
    CanonicalExternalShapeLinkImportsV1, LirTargetProfile, SelectedDependencyLayoutAbiSetV1,
};
use scoop_wire::{BudgetMeter, WirePath};

use super::{ExternalShapeUndefinedUseV1, LayoutLinkClosureError};
use crate::link_object::{
    CanonicalUndefinedRelocationUseV1, StrongRelocationBindingV1, StrongRelocationResolutionV1,
    VerifiedCrossConeStrongRequirementClosureV1, cross_cone_link_closure::use_key,
};

mod symbols;
use symbols::ImportSymbolIndex;

/// This proof preserves both preceding partitions and every unmatched native
/// or runtime candidate. Its imports can only come from a complete selection.
#[derive(Debug)]
pub struct VerifiedExternalShapeRequirementClosureV1<'a> {
    legacy: &'a VerifiedCrossConeStrongRequirementClosureV1,
    imports: &'a CanonicalExternalShapeLinkImportsV1<'a>,
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
    pub const fn semantic_imports(&self) -> &'a CanonicalExternalShapeLinkImportsV1<'a> {
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
    selected: &'a SelectedDependencyLayoutAbiSetV1<'a>,
    meter: &mut BudgetMeter,
) -> Result<VerifiedExternalShapeRequirementClosureV1<'a>, LayoutLinkClosureError> {
    verify_import_requirements(
        legacy,
        selected.consumer(),
        selected.physical_imports(),
        meter,
    )
}

/// Reuses the same partition rules after the owned reader has independently
/// replayed the complete physical imports. This does not grant source access.
pub fn verify_replayed_external_shape_requirements_v1<'a>(
    legacy: &'a VerifiedCrossConeStrongRequirementClosureV1,
    layout: &'a scoop_lir::PhysicalImportsReplayedLayoutAbiSectionV1<'a>,
    meter: &mut BudgetMeter,
) -> Result<VerifiedExternalShapeRequirementClosureV1<'a>, LayoutLinkClosureError> {
    verify_import_requirements(
        legacy,
        layout.exports().provider(),
        layout.physical_imports(),
        meter,
    )
}

fn verify_import_requirements<'a>(
    legacy: &'a VerifiedCrossConeStrongRequirementClosureV1,
    consumer: ConeIdentity,
    imports: &'a CanonicalExternalShapeLinkImportsV1<'a>,
    meter: &mut BudgetMeter,
) -> Result<VerifiedExternalShapeRequirementClosureV1<'a>, LayoutLinkClosureError> {
    if legacy.producer() != consumer {
        return Err(LayoutLinkClosureError::ConsumerMismatch {
            objects: legacy.producer(),
            selection: consumer,
        });
    }
    let symbols = ImportSymbolIndex::new(imports, legacy.target(), meter)?;
    symbols.reject_old_partitions(legacy, meter)?;
    let classified = classify(legacy.remaining_external_candidates(), &symbols, meter)?;
    for (index, import) in imports.records().iter().enumerate() {
        // Initialization edges retain canonical unit ids in metadata. Their
        // complete descriptor support is checked by the source/registration
        // join and need not produce an object pointer relocation.
        let metadata_support = matches!(
            import.subject(),
            scoop_lir::ExternalStrongShapeSubjectV1::InitializationDescriptor(_)
        );
        if !classified.used[index] && !metadata_support {
            return Err(LayoutLinkClosureError::UnusedImport {
                import_index: index as u32,
                provider: import.provider(),
                subject: import.subject(),
            });
        }
    }
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
    used: Vec<bool>,
}

fn classify<'a>(
    candidates: &'a [StrongRelocationBindingV1],
    symbols: &ImportSymbolIndex,
    meter: &mut BudgetMeter,
) -> Result<Classification<'a>, LayoutLinkClosureError> {
    let path = WirePath::root();
    let count = candidates.len() as u64;
    meter.check_table_entries(count, &path)?;
    meter.charge_nodes(count, &path)?;
    meter.charge_work(
        count.saturating_mul(u64::from(count.max(1).ilog2()) + 1),
        &path,
    )?;
    let mut requirements = Vec::new();
    let mut remaining = Vec::new();
    let mut used = Vec::new();
    meter.try_reserve_collection_slots(&mut requirements, candidates.len(), &path)?;
    meter.try_reserve_collection_slots(&mut remaining, candidates.len(), &path)?;
    meter.try_reserve_collection_slots(&mut used, symbols.len(), &path)?;
    used.resize(symbols.len(), false);
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
        if let Some(index) = symbols.find(binding.symbol(), meter)? {
            meter.charge_owned_bytes(binding.symbol().len() as u64, &path)?;
            meter.charge_heap(binding.symbol().len() as u64, &path)?;
            used[index as usize] = true;
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
        used,
    })
}

#[cfg(test)]
#[path = "tests/classification.rs"]
mod tests;
