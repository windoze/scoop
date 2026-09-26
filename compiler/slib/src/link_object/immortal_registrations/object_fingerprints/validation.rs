use std::collections::BTreeMap;

use scoop_identity::{
    DefinitionAtomRole, LinkageClass, ObjectDefinitionPlanId, ObjectDefinitionPlanKey,
    PersistentSymbolKey, PersistentSymbolRequest, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    ImmortalObjectTypeRegistrationRefV1, LirTargetProfile, StrongImmortalObjectRegistrationPlanV1,
};

use super::{ImmortalObjectByteFailureV1, StrongImmortalObjectDefinitionFingerprintError};
use crate::SlibMemberId;
use crate::link_object::immortal_registrations::physical::{atom_file_range, verified_member};
use crate::link_object::{
    BuiltinObjectSectionRoleV1, CanonicalObjectDefinitionRequirementV1,
    CanonicalUndefinedRelocationUseV1, FinalUndefinedSymbolRequirementV1,
    ImmortalObjectRegistrationRelocationFailureV1, LinkDefinitionOwnerV1, RelocationTargetSlotV1,
    StrongDefinitionOwnerV1, StrongRelocationBindingV1, StrongRelocationResolutionV1,
    VerifiedBuiltinObjectStrongRelocationSetV1, VerifiedDarwinArm64RelocationFormV1,
    VerifiedMemberObjectRelocationIndexV1, VerifiedObjectDefinitionRequirementSetV1,
};

pub(super) struct ValidatedImmortalObject<'a> {
    pub(super) member: &'a VerifiedMemberObjectRelocationIndexV1,
    pub(super) bytes: &'a [u8],
}

pub(super) fn validate_immortal_object<'a>(
    builtins: &'a VerifiedBuiltinObjectStrongRelocationSetV1,
    requirements: &VerifiedObjectDefinitionRequirementSetV1,
    objects: &BTreeMap<SlibMemberId, &'a [u8]>,
    plan: StrongImmortalObjectRegistrationPlanV1,
) -> Result<ValidatedImmortalObject<'a>, StrongImmortalObjectDefinitionFingerprintError> {
    let object = plan.object();
    let member = builtins
        .member_plan()
        .member_for_definition(plan.object_definition_plan())
        .ok_or(
            StrongImmortalObjectDefinitionFingerprintError::MissingDefinitionAssignment { object },
        )?;
    let verified_member = verified_member(builtins, member)
        .map_err(StrongImmortalObjectDefinitionFingerprintError::ObjectValidation)?;
    let definition = verified_member
        .definitions()
        .definition(plan.object_definition_plan())
        .ok_or(
            StrongImmortalObjectDefinitionFingerprintError::MissingObjectDefinition { object },
        )?;
    if definition.primary_atom() != plan.object_primary_atom() {
        return Err(StrongImmortalObjectDefinitionFingerprintError::PrimaryAtomMismatch { object });
    }
    let atom = definition
        .atoms()
        .iter()
        .find(|atom| {
            atom.atom() == plan.object_primary_atom()
                && atom.atom_role() == DefinitionAtomRole::Primary
        })
        .copied()
        .ok_or(StrongImmortalObjectDefinitionFingerprintError::MissingPrimaryAtom { object })?;
    let (section_role, file_start, file_end) =
        atom_file_range(verified_member, atom).map_err(|_| {
            StrongImmortalObjectDefinitionFingerprintError::InvalidPrimaryAtomRange { object }
        })?;
    if section_role != BuiltinObjectSectionRoleV1::ReadOnlyData {
        return Err(
            StrongImmortalObjectDefinitionFingerprintError::PrimaryAtomSectionMismatch {
                object,
                actual: section_role,
            },
        );
    }
    let actual_size = file_end.checked_sub(file_start).ok_or(
        StrongImmortalObjectDefinitionFingerprintError::InvalidPrimaryAtomRange { object },
    )?;
    if actual_size != plan.object_size() {
        return Err(
            StrongImmortalObjectDefinitionFingerprintError::ObjectSizeMismatch {
                object,
                expected: plan.object_size(),
                actual: actual_size,
            },
        );
    }
    let full_object = objects
        .get(&member)
        .copied()
        .ok_or(StrongImmortalObjectDefinitionFingerprintError::MissingObject(member))?;
    let start = usize::try_from(file_start)
        .map_err(|_| StrongImmortalObjectDefinitionFingerprintError::ObjectRange { object })?;
    let end = usize::try_from(file_end)
        .map_err(|_| StrongImmortalObjectDefinitionFingerprintError::ObjectRange { object })?;
    let bytes = full_object
        .get(start..end)
        .ok_or(StrongImmortalObjectDefinitionFingerprintError::ObjectRange { object })?;
    validate_string_bytes(bytes, object)?;
    validate_descriptor_relocation(builtins, requirements, verified_member, plan)?;
    Ok(ValidatedImmortalObject {
        member: verified_member,
        bytes,
    })
}

fn validate_string_bytes(
    bytes: &[u8],
    object: scoop_identity::PersistentImmortalObjectId,
) -> Result<(), StrongImmortalObjectDefinitionFingerprintError> {
    use ImmortalObjectByteFailureV1 as Failure;

    if bytes.len() < 24 {
        return object_bytes_error(object, Failure::HeaderTooShort);
    }
    if let Some((offset, actual)) = bytes[..16]
        .iter()
        .copied()
        .enumerate()
        .find(|(_, byte)| *byte != 0)
    {
        return object_bytes_error(
            object,
            Failure::ByteMismatch {
                offset: u64::try_from(offset).expect("String header offset fits u64"),
                expected: 0,
                actual,
            },
        );
    }
    let length = u64::from_le_bytes(bytes[16..24].try_into().expect("eight-byte String length"));
    let payload_end = 24_u64
        .checked_add(length)
        .ok_or_else(|| object_bytes_error_value(object, Failure::LengthOverflow))?;
    let expected_extent = payload_end
        .checked_add(7)
        .map(|value| value & !7)
        .ok_or_else(|| object_bytes_error_value(object, Failure::LengthOverflow))?;
    if usize::try_from(expected_extent).ok() != Some(bytes.len()) {
        return object_bytes_error(object, Failure::ExtentMismatch);
    }
    let payload_end = usize::try_from(payload_end)
        .map_err(|_| object_bytes_error_value(object, Failure::LengthOverflow))?;
    if std::str::from_utf8(&bytes[24..payload_end]).is_err() {
        return object_bytes_error(object, Failure::InvalidUtf8);
    }
    if let Some((offset, actual)) = bytes[payload_end..]
        .iter()
        .copied()
        .enumerate()
        .find(|(_, byte)| *byte != 0)
    {
        return object_bytes_error(
            object,
            Failure::ByteMismatch {
                offset: u64::try_from(payload_end + offset)
                    .expect("String padding offset fits u64"),
                expected: 0,
                actual,
            },
        );
    }
    Ok(())
}

fn validate_descriptor_relocation(
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    requirements: &VerifiedObjectDefinitionRequirementSetV1,
    member: &VerifiedMemberObjectRelocationIndexV1,
    plan: StrongImmortalObjectRegistrationPlanV1,
) -> Result<(), StrongImmortalObjectDefinitionFingerprintError> {
    use ImmortalObjectRegistrationRelocationFailureV1 as Failure;

    let physical_count = member
        .relocations()
        .iter()
        .filter(|relocation| relocation.containing_atom() == plan.object_primary_atom())
        .count();
    let bindings = builtins
        .strong_relocations()
        .bindings()
        .iter()
        .filter(|binding| {
            binding.source_member() == member.member()
                && binding.containing_atom() == plan.object_primary_atom()
        })
        .collect::<Vec<_>>();
    if physical_count != 1 || bindings.len() != 1 {
        return relocation_error(plan, Failure::Count);
    }
    let binding = bindings[0];
    if binding.containing_atom_role() != DefinitionAtomRole::Primary {
        return relocation_error(plan, Failure::ContainingAtomRole);
    }
    if binding.section_role() != BuiltinObjectSectionRoleV1::ReadOnlyData {
        return relocation_error(plan, Failure::SectionRole);
    }
    if binding.offset_within_atom() != 0 {
        return relocation_error(plan, Failure::MissingOffset);
    }
    if binding.width_bytes() != 8 {
        return relocation_error(plan, Failure::Width);
    }
    if binding.relocation_form() != VerifiedDarwinArm64RelocationFormV1::Unsigned64 {
        return relocation_error(plan, Failure::Form);
    }
    if binding.encoded_value() != 0 {
        return relocation_error(plan, Failure::EncodedValue);
    }
    if binding.target_slot() != RelocationTargetSlotV1::Single {
        return relocation_error(plan, Failure::TargetSlot);
    }

    let owner = StrongDefinitionOwnerV1::new(
        StrongDefinitionEntity::exact_type(plan.type_registration()),
        StrongDefinitionRole::TypeDescriptor,
    )
    .expect("type descriptors are valid strong definition owners");
    match plan.semantic().type_registration_ref() {
        ImmortalObjectTypeRegistrationRefV1::Local(_) => {
            validate_local_descriptor_target(builtins, requirements, plan, binding, owner)
        }
        ImmortalObjectTypeRegistrationRefV1::DependencyExternal { provider, .. } => {
            validate_external_descriptor_target(requirements, plan, binding, provider)
        }
    }
}

fn validate_local_descriptor_target(
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    requirements: &VerifiedObjectDefinitionRequirementSetV1,
    plan: StrongImmortalObjectRegistrationPlanV1,
    binding: &StrongRelocationBindingV1,
    expected_owner: StrongDefinitionOwnerV1,
) -> Result<(), StrongImmortalObjectDefinitionFingerprintError> {
    use ImmortalObjectRegistrationRelocationFailureV1 as Failure;

    let key = ObjectDefinitionPlanKey::strong(
        builtins.producer(),
        StrongDefinitionEntity::exact_type(plan.type_registration()),
        StrongDefinitionRole::TypeDescriptor,
    )
    .map_err(
        |source| StrongImmortalObjectDefinitionFingerprintError::DefinitionIdentity {
            object: plan.object(),
            source,
        },
    )?;
    let definition = ObjectDefinitionPlanId::from_key(&key).map_err(|source| {
        StrongImmortalObjectDefinitionFingerprintError::IdentityHash {
            object: plan.object(),
            source,
        }
    })?;
    let target_member = builtins
        .member_plan()
        .member_for_definition(definition)
        .ok_or(
            StrongImmortalObjectDefinitionFingerprintError::MissingDefinitionAssignment {
                object: plan.object(),
            },
        )?;
    let target = verified_member(builtins, target_member)
        .map_err(StrongImmortalObjectDefinitionFingerprintError::ObjectValidation)?;
    let target_definition = target.definitions().definition(definition).ok_or(
        StrongImmortalObjectDefinitionFingerprintError::MissingObjectDefinition {
            object: plan.object(),
        },
    )?;
    let symbol = target
        .definitions()
        .strong_symbol_by_table_index(target_definition.primary_symbol_table_index())
        .ok_or_else(|| relocation_error_value(plan, Failure::TargetSymbol))?;
    let (actual_member, actual_definition, actual_owner) = match binding.resolution() {
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
        StrongRelocationResolutionV1::ExternalCandidate { .. } => {
            return relocation_error(plan, Failure::TargetDefinition);
        }
    };
    if actual_definition != definition {
        return relocation_error(plan, Failure::TargetDefinition);
    }
    if actual_member != target_member {
        return relocation_error(plan, Failure::TargetMember);
    }
    if actual_owner != LinkDefinitionOwnerV1::StrongDefinition(expected_owner) {
        return relocation_error(plan, Failure::TargetOwner);
    }
    if binding.symbol() != symbol.macho_name() {
        return relocation_error(plan, Failure::TargetSymbol);
    }
    match binding.resolution() {
        StrongRelocationResolutionV1::ObjectLocalStrong { .. } => Ok(()),
        StrongRelocationResolutionV1::CurrentConeUndefinedStrong { .. }
            if requirement_for(binding, requirements)
                == Some(CanonicalObjectDefinitionRequirementV1::Legacy(
                    FinalUndefinedSymbolRequirementV1::IntraConeStrong {
                        owner: expected_owner,
                    },
                )) =>
        {
            Ok(())
        }
        _ => relocation_error(plan, Failure::TargetOwner),
    }
}

fn validate_external_descriptor_target(
    requirements: &VerifiedObjectDefinitionRequirementSetV1,
    plan: StrongImmortalObjectRegistrationPlanV1,
    binding: &StrongRelocationBindingV1,
    provider: scoop_identity::ConeIdentity,
) -> Result<(), StrongImmortalObjectDefinitionFingerprintError> {
    use ImmortalObjectRegistrationRelocationFailureV1 as Failure;

    if !matches!(
        binding.resolution(),
        StrongRelocationResolutionV1::ExternalCandidate { .. }
    ) {
        return relocation_error(plan, Failure::TargetDefinition);
    }
    let request = PersistentSymbolRequest::new(
        PersistentSymbolKey::TypeDescriptor(plan.type_registration()),
        LinkageClass::ConeStrong,
    )
    .map_err(
        |source| StrongImmortalObjectDefinitionFingerprintError::Symbol {
            object: plan.object(),
            source,
        },
    )?;
    let expected_symbol = LirTargetProfile::DARWIN_AARCH64
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(request.symbol().as_str());
    if binding.symbol() != expected_symbol.as_bytes() {
        return relocation_error(plan, Failure::TargetSymbol);
    }
    if requirement_for(binding, requirements)
        != Some(
            CanonicalObjectDefinitionRequirementV1::DependencyShapeStrong {
                provider,
                subject: scoop_lir::ExternalStrongShapeSubjectV1::TypeDescriptor(
                    plan.type_registration(),
                ),
            },
        )
    {
        return relocation_error(plan, Failure::TargetOwner);
    }
    Ok(())
}

fn requirement_for(
    binding: &StrongRelocationBindingV1,
    requirements: &VerifiedObjectDefinitionRequirementSetV1,
) -> Option<CanonicalObjectDefinitionRequirementV1> {
    let use_site = CanonicalUndefinedRelocationUseV1::from(binding);
    requirements.requirement_for(&use_site)
}

fn object_bytes_error<T>(
    object: scoop_identity::PersistentImmortalObjectId,
    kind: ImmortalObjectByteFailureV1,
) -> Result<T, StrongImmortalObjectDefinitionFingerprintError> {
    Err(object_bytes_error_value(object, kind))
}

fn object_bytes_error_value(
    object: scoop_identity::PersistentImmortalObjectId,
    kind: ImmortalObjectByteFailureV1,
) -> StrongImmortalObjectDefinitionFingerprintError {
    StrongImmortalObjectDefinitionFingerprintError::ObjectBytes { object, kind }
}

fn relocation_error<T>(
    plan: StrongImmortalObjectRegistrationPlanV1,
    kind: ImmortalObjectRegistrationRelocationFailureV1,
) -> Result<T, StrongImmortalObjectDefinitionFingerprintError> {
    Err(relocation_error_value(plan, kind))
}

fn relocation_error_value(
    plan: StrongImmortalObjectRegistrationPlanV1,
    kind: ImmortalObjectRegistrationRelocationFailureV1,
) -> StrongImmortalObjectDefinitionFingerprintError {
    StrongImmortalObjectDefinitionFingerprintError::DescriptorRelocation {
        object: plan.object(),
        kind,
    }
}
