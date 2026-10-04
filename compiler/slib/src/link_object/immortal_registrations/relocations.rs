use scoop_identity::{
    DefinitionAtomRole, DefinitionAtomSubkey, ObjectDefinitionAtomId, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PersistentImmortalObjectId,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    ImmortalObjectTypeRegistrationRefV1, LirTargetProfile, StrongImmortalObjectRegistrationPlanV1,
};

use super::physical::verified_member;
use super::verification::required_scoop_member;
use super::{
    ImmortalObjectRegistrationRelocationFailureV1, StrongImmortalObjectRegistrationValidationError,
};
use crate::link_object::{
    BuiltinObjectSectionRoleV1, LinkDefinitionOwnerV1, RelocationTargetSlotV1,
    StrongRelocationBindingV1, StrongRelocationResolutionV1, VerifiedMemberObjectRelocationIndexV1,
    VerifiedObjectRelocationFormV1, VerifiedScoopLirDigestPatchSiteSetV1,
};

const OBJECT_POINTER_OFFSET: u64 = 152;
const TYPE_REGISTRATION_POINTER_OFFSET: u64 = 176;

pub(super) fn registration_relocations<'a>(
    patch_sites: &'a VerifiedScoopLirDigestPatchSiteSetV1,
    registration_member: &VerifiedMemberObjectRelocationIndexV1,
    plan: StrongImmortalObjectRegistrationPlanV1,
) -> Result<[&'a StrongRelocationBindingV1; 2], StrongImmortalObjectRegistrationValidationError> {
    use ImmortalObjectRegistrationRelocationFailureV1 as Failure;

    let physical_count = registration_member
        .relocations()
        .iter()
        .filter(|relocation| relocation.containing_atom() == plan.registration_primary_atom())
        .count();
    let bindings = patch_sites
        .builtins()
        .strong_relocations()
        .bindings()
        .iter()
        .filter(|binding| {
            binding.source_member() == registration_member.member()
                && binding.containing_atom() == plan.registration_primary_atom()
        })
        .collect::<Vec<_>>();
    if physical_count != 2 || bindings.len() != 2 {
        return object_relocation_error(plan.object(), Failure::Count);
    }
    let object = bindings
        .iter()
        .find(|binding| binding.offset_within_atom() == OBJECT_POINTER_OFFSET)
        .copied()
        .ok_or_else(|| object_relocation_error_value(plan.object(), Failure::MissingOffset))?;
    let type_registration = bindings
        .iter()
        .find(|binding| binding.offset_within_atom() == TYPE_REGISTRATION_POINTER_OFFSET)
        .copied()
        .ok_or_else(|| type_relocation_error_value(plan.object(), Failure::MissingOffset))?;
    Ok([object, type_registration])
}

pub(super) fn verify_object_relocation(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongImmortalObjectRegistrationPlanV1,
    binding: &StrongRelocationBindingV1,
) -> Result<StrongRelocationBindingV1, StrongImmortalObjectRegistrationValidationError> {
    use ImmortalObjectRegistrationRelocationFailureV1 as Failure;

    validate_relocation_shape(binding, plan, OBJECT_POINTER_OFFSET)
        .map_err(|kind| object_relocation_error_value(plan.object(), kind))?;
    let target_member =
        required_scoop_member(patch_sites.builtins(), plan, plan.object_definition_plan())?;
    let target = verified_member(patch_sites.builtins(), target_member)?;
    let definition = target
        .definitions()
        .definition(plan.object_definition_plan())
        .ok_or(
            StrongImmortalObjectRegistrationValidationError::MissingVerifiedDefinition {
                object: plan.object(),
                definition: plan.object_definition_plan(),
            },
        )?;
    if definition.primary_atom() != plan.object_primary_atom() {
        return Err(
            StrongImmortalObjectRegistrationValidationError::ImmortalObjectPrimaryAtomMismatch {
                object: plan.object(),
                expected: plan.object_primary_atom(),
                actual: definition.primary_atom(),
            },
        );
    }
    let symbol = target
        .definitions()
        .strong_symbol_by_table_index(definition.primary_symbol_table_index())
        .ok_or(
            StrongImmortalObjectRegistrationValidationError::MissingImmortalObjectPrimarySymbol(
                plan.object(),
            ),
        )?;
    let expected_owner = LinkDefinitionOwnerV1::from_definition(
        symbol.definition_owner(),
        StrongDefinitionEntity::immortal_object(plan.object()),
        StrongDefinitionRole::ImmortalObject,
    )
    .expect("the verified immortal object has a complete definition owner");
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
            return object_relocation_error(plan.object(), Failure::TargetDefinition);
        }
    };
    let kind = if actual_definition != plan.object_definition_plan() {
        Some(Failure::TargetDefinition)
    } else if actual_member != target_member {
        Some(Failure::TargetMember)
    } else if actual_owner != expected_owner {
        Some(Failure::TargetOwner)
    } else if binding.symbol() != symbol.macho_name() {
        Some(Failure::TargetSymbol)
    } else {
        None
    };
    if let Some(kind) = kind {
        return object_relocation_error(plan.object(), kind);
    }
    Ok(binding.clone())
}

pub(super) fn verify_type_registration_relocation(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    producer: scoop_identity::ConeIdentity,
    plan: StrongImmortalObjectRegistrationPlanV1,
    binding: &StrongRelocationBindingV1,
) -> Result<StrongRelocationBindingV1, StrongImmortalObjectRegistrationValidationError> {
    use ImmortalObjectRegistrationRelocationFailureV1 as Failure;

    validate_relocation_shape(binding, plan, TYPE_REGISTRATION_POINTER_OFFSET)
        .map_err(|kind| type_relocation_error_value(plan.object(), kind))?;
    match plan.semantic().type_registration_ref() {
        ImmortalObjectTypeRegistrationRefV1::Local(exact_type) => {
            let definition_key = ObjectDefinitionPlanKey::strong(
                producer,
                StrongDefinitionEntity::exact_type(exact_type),
                StrongDefinitionRole::TypeRegistration,
            )
            .expect("local type registration is a valid strong definition");
            let definition = ObjectDefinitionPlanId::from_key(&definition_key)
                .expect("local type registration definition is hashable");
            let expected_primary = ObjectDefinitionAtomId::from_key(&ObjectDefinitionAtomKey::new(
                definition,
                DefinitionAtomRole::Primary,
                DefinitionAtomSubkey::Singleton,
            ))
            .expect("local type registration primary atom is hashable");
            let target_member = required_scoop_member(patch_sites.builtins(), plan, definition)?;
            let target = verified_member(patch_sites.builtins(), target_member)?;
            let target_definition = target.definitions().definition(definition).ok_or(
                StrongImmortalObjectRegistrationValidationError::MissingVerifiedDefinition {
                    object: plan.object(),
                    definition,
                },
            )?;
            if target_definition.primary_atom() != expected_primary {
                return Err(
                    StrongImmortalObjectRegistrationValidationError::
                        TypeRegistrationPrimaryAtomMismatch {
                            object: plan.object(),
                            expected: expected_primary,
                            actual: target_definition.primary_atom(),
                        },
                );
            }
            let symbol = target
                .definitions()
                .strong_symbol_by_table_index(target_definition.primary_symbol_table_index())
                .ok_or(
                    StrongImmortalObjectRegistrationValidationError::
                        MissingTypeRegistrationPrimarySymbol(plan.object()),
                )?;
            let expected_owner = LinkDefinitionOwnerV1::from_strong_primary(
                StrongDefinitionEntity::exact_type(exact_type),
                StrongDefinitionRole::TypeRegistration,
            )
            .expect("type registration is a valid strong definition owner");
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
                    return type_relocation_error(plan.object(), Failure::TargetDefinition);
                }
            };
            let kind = if actual_definition != definition {
                Some(Failure::TargetDefinition)
            } else if actual_member != target_member {
                Some(Failure::TargetMember)
            } else if actual_owner != expected_owner {
                Some(Failure::TargetOwner)
            } else if binding.symbol() != symbol.macho_name() {
                Some(Failure::TargetSymbol)
            } else {
                None
            };
            if let Some(kind) = kind {
                return type_relocation_error(plan.object(), kind);
            }
        }
        ImmortalObjectTypeRegistrationRefV1::DependencyExternal { .. } => {
            if !matches!(
                binding.resolution(),
                StrongRelocationResolutionV1::ExternalCandidate { .. }
            ) {
                return type_relocation_error(plan.object(), Failure::TargetDefinition);
            }
            let normalization = LirTargetProfile::DARWIN_AARCH64
                .contract()
                .native_symbol_normalization();
            let expected = normalization.compiler_generated_object_symbol(
                plan.type_registration_symbol().symbol().as_str(),
            );
            if binding.symbol() != expected.as_bytes() {
                return type_relocation_error(plan.object(), Failure::TargetSymbol);
            }
        }
    }
    Ok(binding.clone())
}

fn validate_relocation_shape(
    binding: &StrongRelocationBindingV1,
    plan: StrongImmortalObjectRegistrationPlanV1,
    offset: u64,
) -> Result<(), ImmortalObjectRegistrationRelocationFailureV1> {
    use ImmortalObjectRegistrationRelocationFailureV1 as Failure;

    if binding.containing_atom() != plan.registration_primary_atom() {
        Err(Failure::ContainingAtom)
    } else if binding.containing_atom_role() != DefinitionAtomRole::Primary {
        Err(Failure::ContainingAtomRole)
    } else if binding.section_role() != BuiltinObjectSectionRoleV1::ReadOnlyData {
        Err(Failure::SectionRole)
    } else if binding.offset_within_atom() != offset {
        Err(Failure::MissingOffset)
    } else if binding.width_bytes() != 8 {
        Err(Failure::Width)
    } else if binding.relocation_form() != VerifiedObjectRelocationFormV1::Unsigned64 {
        Err(Failure::Form)
    } else if binding.encoded_value() != 0 {
        Err(Failure::EncodedValue)
    } else if binding.target_slot() != RelocationTargetSlotV1::Single {
        Err(Failure::TargetSlot)
    } else {
        Ok(())
    }
}

fn object_relocation_error<T>(
    object: PersistentImmortalObjectId,
    kind: ImmortalObjectRegistrationRelocationFailureV1,
) -> Result<T, StrongImmortalObjectRegistrationValidationError> {
    Err(object_relocation_error_value(object, kind))
}

fn object_relocation_error_value(
    object: PersistentImmortalObjectId,
    kind: ImmortalObjectRegistrationRelocationFailureV1,
) -> StrongImmortalObjectRegistrationValidationError {
    StrongImmortalObjectRegistrationValidationError::ObjectRelocationMismatch { object, kind }
}

fn type_relocation_error<T>(
    object: PersistentImmortalObjectId,
    kind: ImmortalObjectRegistrationRelocationFailureV1,
) -> Result<T, StrongImmortalObjectRegistrationValidationError> {
    Err(type_relocation_error_value(object, kind))
}

fn type_relocation_error_value(
    object: PersistentImmortalObjectId,
    kind: ImmortalObjectRegistrationRelocationFailureV1,
) -> StrongImmortalObjectRegistrationValidationError {
    StrongImmortalObjectRegistrationValidationError::TypeRegistrationRelocationMismatch {
        object,
        kind,
    }
}
