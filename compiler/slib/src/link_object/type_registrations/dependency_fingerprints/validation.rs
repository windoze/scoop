use scoop_identity::{
    DefinitionAtomRole, ObjectDefinitionPlanId, PersistentExactTypeId, PersistentSymbolKey,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{RefScan, StrongTypeRegistrationPlan};

use super::{StrongTypeDependencyFingerprintError, TypeDependencyArtifactV1};
use crate::link_object::type_registrations::versioned::{
    DescriptorReferenceKind, LinkDescriptorReference,
};
use crate::link_object::{
    BuiltinObjectSectionRoleV1, VerifiedBuiltinObjectStrongRelocationSetV1,
    VerifiedRelocationTargetV1, VerifiedStrongTypeRegistrationV1,
};

pub(super) const TYPE_DESCRIPTOR_SIZE: usize = 152;

pub(super) fn exact_bytes(
    object: &[u8],
    checked_offset: u64,
    size: usize,
    exact_type: PersistentExactTypeId,
    artifact: TypeDependencyArtifactV1,
) -> Result<&[u8], StrongTypeDependencyFingerprintError> {
    let start = usize::try_from(checked_offset).map_err(|_| {
        StrongTypeDependencyFingerprintError::Range {
            exact_type,
            artifact,
        }
    })?;
    let end = start
        .checked_add(size)
        .ok_or(StrongTypeDependencyFingerprintError::Range {
            exact_type,
            artifact,
        })?;
    object
        .get(start..end)
        .ok_or(StrongTypeDependencyFingerprintError::Range {
            exact_type,
            artifact,
        })
}

pub(super) fn validate_descriptor<D, C>(
    actual: &[u8],
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    verified: &VerifiedStrongTypeRegistrationV1,
    plan: &StrongTypeRegistrationPlan<D, C>,
    definitions: &std::collections::BTreeMap<
        ObjectDefinitionPlanId,
        (StrongDefinitionEntity, StrongDefinitionRole),
    >,
) -> Result<(), StrongTypeDependencyFingerprintError>
where
    D: LinkDescriptorReference,
{
    validate_descriptor_bytes(actual, plan)?;
    validate_descriptor_relocations(builtins, verified, plan, definitions)
}

fn validate_descriptor_bytes<D: Copy, C>(
    actual: &[u8],
    plan: &StrongTypeRegistrationPlan<D, C>,
) -> Result<(), StrongTypeDependencyFingerprintError> {
    let expected = expected_descriptor_bytes(plan);
    if let Some(offset) = actual
        .iter()
        .zip(&expected)
        .position(|(actual, expected)| *actual != *expected)
    {
        return Err(
            StrongTypeDependencyFingerprintError::DescriptorByteMismatch {
                exact_type: plan.exact_type(),
                offset_within_descriptor: offset as u64,
                expected: expected[offset],
                actual: actual[offset],
            },
        );
    }
    Ok(())
}

fn expected_descriptor_bytes<D: Copy, C>(plan: &StrongTypeRegistrationPlan<D, C>) -> Vec<u8> {
    let semantic = plan.semantic();
    let shape = semantic.instance_shape();
    let function = semantic.relations();
    let mut bytes = vec![0; TYPE_DESCRIPTOR_SIZE + function.related_types().len() * 8];
    write_u64(&mut bytes, 0, plan.runtime_type().get());
    write_u32(&mut bytes, 8, shape.instance_kind().tag());
    write_u32(&mut bytes, 12, shape.inline_storage_kind().tag());
    write_u64(&mut bytes, 16, shape.minimum_size());
    write_u64(&mut bytes, 24, shape.instance_alignment());
    write_u64(&mut bytes, 32, shape.inline_offset());
    write_u64(&mut bytes, 40, shape.inline_size());
    write_u64(&mut bytes, 48, shape.inline_stride());
    write_u64(&mut bytes, 56, shape.inline_alignment());
    write_u64(
        &mut bytes,
        104,
        u64::try_from(semantic.itables().len()).expect("itable count fits u64"),
    );
    write_u64(
        &mut bytes,
        120,
        u64::try_from(semantic.diagnostic_name().len()).expect("diagnostic length fits u64"),
    );
    write_u32(&mut bytes, 128, function.runtime_kind());
    write_u32(
        &mut bytes,
        132,
        u32::try_from(function.related_types().len())
            .expect("function arity fits its metadata count"),
    );
    bytes
}

fn validate_descriptor_relocations<D, C>(
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    verified: &VerifiedStrongTypeRegistrationV1,
    plan: &StrongTypeRegistrationPlan<D, C>,
    definitions: &std::collections::BTreeMap<
        ObjectDefinitionPlanId,
        (StrongDefinitionEntity, StrongDefinitionRole),
    >,
) -> Result<(), StrongTypeDependencyFingerprintError>
where
    D: LinkDescriptorReference,
{
    let descriptor = verified.descriptor();
    let member = builtins
        .strong_relocations()
        .members()
        .iter()
        .find(|member| member.member() == descriptor.member())
        .ok_or(StrongTypeDependencyFingerprintError::MissingObject(
            descriptor.member(),
        ))?;
    let mut actual = member
        .relocations()
        .iter()
        .filter(|relocation| relocation.containing_atom() == plan.descriptor_primary_atom())
        .collect::<Vec<_>>();
    actual.sort_unstable_by_key(|relocation| relocation.offset_within_atom());
    let expected = expected_relocation_offsets(plan);
    if actual
        .iter()
        .map(|relocation| relocation.offset_within_atom())
        .ne(expected.iter().copied())
    {
        return Err(
            StrongTypeDependencyFingerprintError::DescriptorRelocationSetMismatch {
                exact_type: plan.exact_type(),
                expected,
                actual: actual
                    .iter()
                    .map(|relocation| relocation.offset_within_atom())
                    .collect(),
            },
        );
    }
    for relocation in actual {
        let offset = relocation.offset_within_atom();
        if relocation.containing_atom_role() != DefinitionAtomRole::Primary
            || relocation.section_role() != BuiltinObjectSectionRoleV1::ReadOnlyData
            || relocation.width_bytes() != 8
        {
            return descriptor_relocation_error(plan.exact_type(), offset);
        }
        // These local pointers were resolved against their exact atom above.
        if offset == 112 && relocation == descriptor.diagnostic_relocation()
            || offset == 96
                && descriptor.itable_directory().descriptor_relocation() == Some(relocation)
        {
            continue;
        }
        if relocation
            .shape()
            .form()
            .absolute64_addend(relocation.encoded_value())
            != Some(0)
        {
            return descriptor_relocation_error(plan.exact_type(), offset);
        }
        let Some(target) = relocation.shape().absolute64_target() else {
            return descriptor_relocation_error(plan.exact_type(), offset);
        };
        let resolved =
            super::super::targets::local_definition(builtins, member.member(), relocation)
                .map(|definition| VerifiedRelocationTargetV1::StrongDefinition { definition });
        let target = resolved.as_ref().unwrap_or(target);
        let target_matches = match offset {
            64 => matches!(
                (plan.inline_scan().definition_plan(), target),
                (
                    Some(expected),
                    VerifiedRelocationTargetV1::StrongDefinition { definition }
                ) if expected == *definition
            ),
            72 => definition_target_matches(
                target,
                definitions,
                StrongDefinitionEntity::scan(plan.semantic().instance_scan()),
                StrongDefinitionRole::ScanProgram,
            ),
            88 => definition_target_matches(
                target,
                definitions,
                StrongDefinitionEntity::dispatch_table(plan.semantic().vtable().table()),
                StrongDefinitionRole::DispatchTable,
            ),
            96 => descriptor.itable_directory().descriptor_relocation() == Some(relocation),
            80 => match plan.semantic().parent().map(LinkDescriptorReference::kind) {
                Some(DescriptorReferenceKind::Local(exact)) => definition_target_matches(
                    target,
                    definitions,
                    StrongDefinitionEntity::exact_type(exact),
                    StrongDefinitionRole::TypeDescriptor,
                ),
                Some(DescriptorReferenceKind::External(exact)) => external_target_matches(
                    builtins.member_plan().target(),
                    target,
                    PersistentSymbolKey::TypeDescriptor(exact),
                ),
                None => false,
            },
            112 => relocation == descriptor.diagnostic_relocation(),
            144 => plan.semantic().release_policy().hook().is_some_and(|hook| {
                definition_target_matches(
                    target,
                    definitions,
                    StrongDefinitionEntity::callable_body(*hook),
                    StrongDefinitionRole::CallableBody,
                ) || external_target_matches(
                    builtins.member_plan().target(),
                    target,
                    PersistentSymbolKey::CallableBody(*hook),
                )
            }),
            offset if offset == 136 || offset >= 152 => {
                let function = plan.semantic().relations();
                let reference = if offset == 136 {
                    function.result().copied().flatten()
                } else {
                    function
                        .related_types()
                        .get(((offset - 152) / 8) as usize)
                        .copied()
                        .flatten()
                };
                match reference.map(LinkDescriptorReference::kind) {
                    Some(DescriptorReferenceKind::Local(exact)) => definition_target_matches(
                        target,
                        definitions,
                        StrongDefinitionEntity::exact_type(exact),
                        StrongDefinitionRole::TypeDescriptor,
                    ),
                    Some(DescriptorReferenceKind::External(exact)) => external_target_matches(
                        builtins.member_plan().target(),
                        target,
                        PersistentSymbolKey::TypeDescriptor(exact),
                    ),
                    None => false,
                }
            }
            _ => false,
        };
        if !target_matches {
            return descriptor_relocation_error(plan.exact_type(), offset);
        }
    }
    Ok(())
}

fn definition_target_matches(
    target: &VerifiedRelocationTargetV1,
    definitions: &std::collections::BTreeMap<
        ObjectDefinitionPlanId,
        (StrongDefinitionEntity, StrongDefinitionRole),
    >,
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> bool {
    matches!(target, VerifiedRelocationTargetV1::StrongDefinition { definition }
        if definitions.get(definition) == Some(&(entity, role)))
}

fn external_target_matches(
    profile: scoop_lir::LirTargetProfile,
    target: &VerifiedRelocationTargetV1,
    symbol: PersistentSymbolKey,
) -> bool {
    let symbol = scoop_identity::MangledSymbol::from_key(&symbol);
    let expected = profile
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(symbol.as_str());
    matches!(target, VerifiedRelocationTargetV1::ExternalUndefined { name, .. }
        if name == expected.as_bytes())
}

fn expected_relocation_offsets<D: Copy, C>(plan: &StrongTypeRegistrationPlan<D, C>) -> Vec<u64> {
    let mut expected = Vec::with_capacity(6);
    if plan.inline_scan().definition_plan().is_some() {
        expected.push(64);
    }
    let shape = plan.semantic().instance_shape();
    if !matches!(shape.object_scan(), RefScan::None) {
        expected.push(72);
    }
    if plan.semantic().parent().is_some() {
        expected.push(80);
    }
    if !plan.semantic().vtable().slots().is_empty() {
        expected.push(88);
    }
    if !plan.semantic().itables().is_empty() {
        expected.push(96);
    }
    expected.push(112);
    let function = plan.semantic().relations();
    if function.result().copied().flatten().is_some() {
        expected.push(136);
    }
    if plan.semantic().release_policy().hook().is_some() {
        expected.push(144);
    }
    for (index, reference) in function.related_types().iter().enumerate() {
        if reference.is_some() {
            expected.push(152 + index as u64 * 8);
        }
    }
    expected
}

fn descriptor_relocation_error<T>(
    exact_type: PersistentExactTypeId,
    offset_within_descriptor: u64,
) -> Result<T, StrongTypeDependencyFingerprintError> {
    Err(
        StrongTypeDependencyFingerprintError::DescriptorRelocationMismatch {
            exact_type,
            offset_within_descriptor,
        },
    )
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests;
