//! Classification of ordinary dependency relocations before native/runtime handling.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{ConeIdentity, StrongCallableDefinitionOwner};

use super::{
    CrossConeLinkSemanticImportBuildError, CrossConeLinkSemanticImportSetV1,
    CrossConeUndefinedRequirementV1, use_key,
};
use crate::SlibMemberId;
use crate::link_object::{
    CanonicalUndefinedRelocationUseV1, StrongRelocationBindingV1, StrongRelocationResolutionV1,
    VerifiedCoreStrongRequirementClosureV1,
};

/// Core classification plus the separately typed ordinary dependency slice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedCrossConeStrongRequirementClosureV1 {
    core_closure: VerifiedCoreStrongRequirementClosureV1,
    semantic_imports: CrossConeLinkSemanticImportSetV1,
    requirements: Vec<CrossConeUndefinedRequirementV1>,
    remaining_external_candidates: Vec<StrongRelocationBindingV1>,
}

impl VerifiedCrossConeStrongRequirementClosureV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.core_closure.producer()
    }

    pub const fn core_closure(&self) -> &VerifiedCoreStrongRequirementClosureV1 {
        &self.core_closure
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

/// Partitions ordinary dependency uses out of the core-unmatched relocation set.
pub fn verify_cross_cone_strong_requirements_v1(
    core_closure: VerifiedCoreStrongRequirementClosureV1,
    bridge: &scoop_lir::CrossConeLirBridgeSectionV1,
) -> Result<VerifiedCrossConeStrongRequirementClosureV1, CrossConeStrongRequirementValidationError>
{
    if core_closure.producer() != bridge.artifact() {
        return Err(
            CrossConeStrongRequirementValidationError::ConsumerMismatch {
                object: core_closure.producer(),
                bridge: bridge.artifact(),
            },
        );
    }
    let semantic_imports = CrossConeLinkSemanticImportSetV1::from_lir_bridge(bridge)
        .map_err(CrossConeStrongRequirementValidationError::SemanticImports)?;
    let normalization = core_closure
        .target()
        .contract()
        .native_symbol_normalization();
    let mut imports_by_symbol = BTreeMap::new();
    for (index, import) in semantic_imports.imports().iter().enumerate() {
        let symbol = normalization
            .compiler_generated_object_symbol(import.expected_symbol().symbol().as_str())
            .into_bytes();
        if let Some(previous) = imports_by_symbol.insert(symbol.clone(), index as u32) {
            return Err(
                CrossConeStrongRequirementValidationError::DuplicateNormalizedSymbol {
                    symbol,
                    first_import: previous,
                    second_import: index as u32,
                },
            );
        }
    }

    let mut used_imports = BTreeSet::new();
    let mut requirements = Vec::new();
    let mut remaining_external_candidates = Vec::new();
    for binding in core_closure.remaining_external_candidates() {
        let StrongRelocationResolutionV1::ExternalCandidate { .. } = binding.resolution() else {
            return Err(
                CrossConeStrongRequirementValidationError::NonExternalCoreRemainder {
                    member: binding.source_member(),
                    atom: binding.containing_atom(),
                },
            );
        };
        if let Some(import_index) = imports_by_symbol.get(binding.symbol()).copied() {
            used_imports.insert(import_index);
            requirements.push(CrossConeUndefinedRequirementV1 {
                use_site: CanonicalUndefinedRelocationUseV1::from(binding),
                import_index,
            });
        } else {
            remaining_external_candidates.push(binding.clone());
        }
    }
    requirements.sort_unstable_by_key(|requirement| use_key(requirement.use_site()));
    if let Some(pair) = requirements
        .windows(2)
        .find(|pair| use_key(pair[0].use_site()) == use_key(pair[1].use_site()))
    {
        let use_site = pair[0].use_site();
        return Err(CrossConeStrongRequirementValidationError::DuplicateUse {
            member: use_site.source_member(),
            atom: use_site.containing_atom(),
            offset: use_site.offset_within_atom(),
            target_slot: use_site.target_slot(),
        });
    }
    if let Some((index, import)) = semantic_imports
        .imports()
        .iter()
        .enumerate()
        .find(|(index, _)| !used_imports.contains(&(*index as u32)))
    {
        return Err(CrossConeStrongRequirementValidationError::UnusedImport {
            import_index: index as u32,
            provider: import.provider(),
            target: import.target(),
        });
    }

    Ok(VerifiedCrossConeStrongRequirementClosureV1 {
        core_closure,
        semantic_imports,
        requirements,
        remaining_external_candidates,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CrossConeStrongRequirementValidationError {
    ConsumerMismatch {
        object: ConeIdentity,
        bridge: ConeIdentity,
    },
    SemanticImports(CrossConeLinkSemanticImportBuildError),
    DuplicateNormalizedSymbol {
        symbol: Vec<u8>,
        first_import: u32,
        second_import: u32,
    },
    NonExternalCoreRemainder {
        member: SlibMemberId,
        atom: scoop_identity::ObjectDefinitionAtomId,
    },
    DuplicateUse {
        member: SlibMemberId,
        atom: scoop_identity::ObjectDefinitionAtomId,
        offset: u64,
        target_slot: crate::link_object::RelocationTargetSlotV1,
    },
    UnusedImport {
        import_index: u32,
        provider: ConeIdentity,
        target: StrongCallableDefinitionOwner,
    },
}

impl fmt::Display for CrossConeStrongRequirementValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid cross-Cone strong requirement closure: {self:?}"
        )
    }
}

impl std::error::Error for CrossConeStrongRequirementValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SemanticImports(source) => Some(source),
            Self::ConsumerMismatch { .. }
            | Self::DuplicateNormalizedSymbol { .. }
            | Self::NonExternalCoreRemainder { .. }
            | Self::DuplicateUse { .. }
            | Self::UnusedImport { .. } => None,
        }
    }
}
