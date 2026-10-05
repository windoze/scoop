//! Image-table references to the actual Strong or ODR registration definitions.

use super::*;
use crate::link_object::PlannedStrongObjectSymbolRoleV1;

#[allow(clippy::too_many_arguments)]
pub(super) fn verify_registration_table(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    member: &VerifiedMemberObjectRelocationIndexV1,
    atoms: &[VerifiedDefinitionAtomRangeV1],
    object: &[u8],
    atom: ObjectDefinitionAtomId,
    role: ConeImageAtomRoleV1,
    targets: impl IntoIterator<Item = (StrongDefinitionEntity, StrongDefinitionRole)>,
    all_bindings: &mut Vec<StrongRelocationBindingV1>,
) -> Result<VerifiedConeImageAtomV1, ConeImageValidationError> {
    let targets = targets.into_iter().collect::<Vec<_>>();
    let expected_bytes = vec![0; targets.len().max(1) * 8];
    let verified = require_table_bytes(member, atoms, object, atom, role, &expected_bytes, 8)?;
    require_relocation_count(member, atom, targets.len(), role)?;
    for (index, target) in targets.into_iter().enumerate() {
        let offset = u64::try_from(index).expect("table index fits u64") * 8;
        let relocation = require_relocation(member, atom, offset, role, index)?;
        let binding = patch_sites
            .builtins()
            .strong_relocations()
            .bindings()
            .iter()
            .find(|binding| {
                binding.source_member() == member.member()
                    && binding.containing_atom() == atom
                    && binding.offset_within_atom() == offset
            })
            .ok_or(ConeImageValidationError::RelocationMismatch {
                role,
                index,
                kind: ConeImageRelocationFailureV1::TargetKind,
            })?;
        let kind = if relocation.containing_atom_role() != DefinitionAtomRole::RuntimeRecord
            || binding.containing_atom_role() != DefinitionAtomRole::RuntimeRecord
        {
            Some(ConeImageRelocationFailureV1::ContainingAtomRole)
        } else if relocation.section_role() != BuiltinObjectSectionRoleV1::ReadOnlyData
            || binding.section_role() != BuiltinObjectSectionRoleV1::ReadOnlyData
        {
            Some(ConeImageRelocationFailureV1::SectionRole)
        } else if relocation.width_bytes() != 8 || binding.width_bytes() != 8 {
            Some(ConeImageRelocationFailureV1::Width)
        } else if !relocation.shape().form().is_absolute64()
            || !binding.relocation_form().is_absolute64()
        {
            Some(ConeImageRelocationFailureV1::Form)
        } else if relocation
            .shape()
            .form()
            .absolute64_addend(relocation.encoded_value())
            != Some(0)
            || binding
                .relocation_form()
                .absolute64_addend(binding.encoded_value())
                != Some(0)
        {
            Some(ConeImageRelocationFailureV1::EncodedValue)
        } else if binding.target_slot() != RelocationTargetSlotV1::Single {
            Some(ConeImageRelocationFailureV1::TargetSlot)
        } else {
            registration_target_failure(patch_sites, binding, target)
        };
        if let Some(kind) = kind {
            return relocation_error(role, index, kind);
        }
        all_bindings.push(binding.clone());
    }
    Ok(verified)
}

fn registration_target_failure(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    binding: &StrongRelocationBindingV1,
    expected: (StrongDefinitionEntity, StrongDefinitionRole),
) -> Option<ConeImageRelocationFailureV1> {
    use ConeImageRelocationFailureV1 as Failure;

    let (target_member, target_definition, target_owner) = match binding.resolution() {
        StrongRelocationResolutionV1::ObjectLocalStrong {
            target_member,
            definition,
            owner,
        }
        | StrongRelocationResolutionV1::CurrentConeUndefinedStrong {
            target_member,
            definition,
            owner,
        } => (target_member, definition, owner),
        StrongRelocationResolutionV1::ExternalCandidate { .. } => return Some(Failure::TargetKind),
    };
    let Ok(member) = verified_member(patch_sites.builtins(), target_member) else {
        return Some(Failure::TargetDefinition);
    };
    let Some(definition) = member.definitions().definition(target_definition) else {
        return Some(Failure::TargetDefinition);
    };
    let Some(primary) = member
        .definitions()
        .strong_symbol_by_table_index(definition.primary_symbol_table_index())
    else {
        return Some(Failure::TargetDefinition);
    };
    let PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
        definition,
        owner,
        definition_role,
        ..
    } = primary.role()
    else {
        return Some(Failure::TargetKind);
    };
    if (owner, definition_role) != expected {
        return Some(Failure::TargetOwner);
    }
    let expected_owner =
        LinkDefinitionOwnerV1::from_definition(primary.definition_owner(), owner, definition_role)
            .expect("a verified registration primary has a typed definition owner");
    if definition != target_definition {
        Some(Failure::TargetDefinition)
    } else if target_owner != expected_owner {
        Some(Failure::TargetOwner)
    } else if binding.symbol() != primary.macho_name() {
        Some(Failure::TargetSymbol)
    } else {
        None
    }
}
