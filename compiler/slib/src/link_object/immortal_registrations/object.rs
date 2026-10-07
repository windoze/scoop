//! Validate the immortal String payload at the object-file boundary.

use std::collections::BTreeMap;

use scoop_identity::{LinkageClass, PersistentSymbolKey, PersistentSymbolRequest};
use scoop_lir::{ImmortalObjectTypeRegistrationRefV1, StrongImmortalObjectRegistrationPlanV1};

use super::physical::{atom_file_range, verified_member};
use super::verification::required_scoop_member;
use super::{
    ImmortalObjectBodyFailureV1 as Failure,
    StrongImmortalObjectRegistrationValidationError as Error,
};
use crate::SlibMemberId;
use crate::link_object::{
    BuiltinObjectSectionRoleV1, RelocationTargetSlotV1, StrongRelocationResolutionV1,
    VerifiedBuiltinObjectStrongRelocationSetV1,
};

pub(super) fn validate(
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    objects: &BTreeMap<SlibMemberId, &[u8]>,
    plan: StrongImmortalObjectRegistrationPlanV1,
) -> Result<(), Error> {
    let invalid = |kind| Error::InvalidObjectBody {
        object: plan.object(),
        kind,
    };
    let member = required_scoop_member(builtins, plan, plan.object_definition_plan())?;
    let verified = verified_member(builtins, member)?;
    let definition = verified
        .definitions()
        .definition(plan.object_definition_plan())
        .ok_or_else(|| invalid(Failure::Extent))?;
    let atom = definition
        .atoms()
        .iter()
        .find(|atom| atom.atom() == plan.object_primary_atom())
        .copied()
        .ok_or_else(|| invalid(Failure::Extent))?;
    let (role, start, end) =
        atom_file_range(verified, atom).map_err(|_| invalid(Failure::Extent))?;
    if role != BuiltinObjectSectionRoleV1::ReadOnlyData
        || end - start != plan.object_size()
        || atom.start() % plan.required_alignment() != 0
    {
        return Err(invalid(Failure::Extent));
    }
    let bytes = objects[&member]
        .get(start as usize..end as usize)
        .ok_or_else(|| invalid(Failure::Extent))?;
    validate_string(bytes).map_err(invalid)?;
    let relocations = verified
        .relocations()
        .iter()
        .filter(|relocation| relocation.containing_atom() == plan.object_primary_atom());
    let mut bindings = builtins
        .strong_relocations()
        .bindings()
        .iter()
        .filter(|binding| {
            binding.source_member() == member
                && binding.containing_atom() == plan.object_primary_atom()
        });
    let Some(binding) = bindings.next() else {
        return Err(invalid(Failure::DescriptorRelocation));
    };
    if relocations.count() != 1
        || bindings.next().is_some()
        || binding.offset_within_atom() != 0
        || binding.width_bytes() != 8
        || !binding.relocation_form().is_absolute64()
        || binding
            .relocation_form()
            .absolute64_addend(binding.encoded_value())
            != Some(0)
        || binding.target_slot() != RelocationTargetSlotV1::Single
    {
        return Err(invalid(Failure::DescriptorRelocation));
    }
    let symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::TypeDescriptor(plan.type_registration()),
        LinkageClass::ConeStrong,
    )
    .expect("an exact type has a descriptor symbol");
    let expected = builtins
        .member_plan()
        .target()
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(symbol.symbol().as_str());
    let local = matches!(
        binding.resolution(),
        StrongRelocationResolutionV1::ObjectLocalStrong { .. }
            | StrongRelocationResolutionV1::CurrentConeUndefinedStrong { .. }
    );
    if binding.symbol() != expected.as_bytes()
        || local
            != matches!(
                plan.semantic().type_registration_ref(),
                ImmortalObjectTypeRegistrationRefV1::Local(_)
            )
    {
        return Err(invalid(Failure::DescriptorRelocation));
    }
    Ok(())
}

fn validate_string(bytes: &[u8]) -> Result<(), Failure> {
    if bytes.len() < 24 || bytes[..16].iter().any(|byte| *byte != 0) {
        return Err(Failure::Header);
    }
    let length = u64::from_le_bytes(bytes[16..24].try_into().expect("eight-byte String length"));
    let end = 24u64.checked_add(length).ok_or(Failure::Length)?;
    let extent = end.checked_add(7).ok_or(Failure::Length)? & !7;
    if extent != bytes.len() as u64 {
        return Err(Failure::Length);
    }
    let end = usize::try_from(end).map_err(|_| Failure::Length)?;
    std::str::from_utf8(&bytes[24..end]).map_err(|_| Failure::Utf8)?;
    if bytes[end..].iter().any(|byte| *byte != 0) {
        return Err(Failure::Padding);
    }
    Ok(())
}
