use scoop_identity::{DefinitionAtomRole, PersistentExactTypeId};
use scoop_lir::{RefScan, StrongTypeDescriptorRefV1, StrongTypeRegistrationPlanV1};

use super::{StrongTypeDependencyFingerprintError, TypeDependencyArtifactV1};
use crate::link_object::{
    BuiltinObjectSectionRoleV1, VerifiedBuiltinObjectStrongRelocationSetV1,
    VerifiedDarwinArm64RelocationShapeV1, VerifiedRelocationTargetV1,
    VerifiedStrongTypeRegistrationV1,
};

pub(super) const TYPE_DESCRIPTOR_SIZE: usize = 128;

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

pub(super) fn validate_descriptor(
    actual: &[u8],
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    verified: &VerifiedStrongTypeRegistrationV1,
    plan: &StrongTypeRegistrationPlanV1,
) -> Result<(), StrongTypeDependencyFingerprintError> {
    validate_descriptor_bytes(actual, plan)?;
    validate_descriptor_relocations(builtins, verified, plan)
}

fn validate_descriptor_bytes(
    actual: &[u8],
    plan: &StrongTypeRegistrationPlanV1,
) -> Result<(), StrongTypeDependencyFingerprintError> {
    let expected = expected_descriptor_bytes(plan);
    if let Some(offset) = actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| *actual != expected)
    {
        return Err(
            StrongTypeDependencyFingerprintError::DescriptorByteMismatch {
                exact_type: plan.exact_type(),
                offset_within_descriptor: u8::try_from(offset)
                    .expect("type descriptor offsets fit u8"),
                expected: expected[offset],
                actual: actual[offset],
            },
        );
    }
    Ok(())
}

fn expected_descriptor_bytes(plan: &StrongTypeRegistrationPlanV1) -> [u8; TYPE_DESCRIPTOR_SIZE] {
    let semantic = plan.semantic();
    let shape = semantic.instance_shape();
    let mut bytes = [0; TYPE_DESCRIPTOR_SIZE];
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
    bytes
}

fn validate_descriptor_relocations(
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    verified: &VerifiedStrongTypeRegistrationV1,
    plan: &StrongTypeRegistrationPlanV1,
) -> Result<(), StrongTypeDependencyFingerprintError> {
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
            || relocation.encoded_value() != 0
        {
            return descriptor_relocation_error(plan.exact_type(), offset);
        }
        let target = match relocation.shape() {
            VerifiedDarwinArm64RelocationShapeV1::Unsigned64 { target } => target,
            _ => return descriptor_relocation_error(plan.exact_type(), offset),
        };
        let target_matches = match offset {
            64 => matches!(
                (plan.inline_scan().definition_plan(), target),
                (
                    Some(expected),
                    VerifiedRelocationTargetV1::StrongDefinition { definition }
                ) if expected == *definition
            ),
            72 | 88 | 96 => matches!(
                target,
                VerifiedRelocationTargetV1::LocalDefinition { .. }
                    | VerifiedRelocationTargetV1::StrongDefinition { .. }
            ),
            80 => match plan.semantic().parent() {
                Some(StrongTypeDescriptorRefV1::Local(_)) => {
                    matches!(target, VerifiedRelocationTargetV1::StrongDefinition { .. })
                }
                Some(StrongTypeDescriptorRefV1::CoreExternal(_)) => {
                    matches!(target, VerifiedRelocationTargetV1::ExternalUndefined { .. })
                }
                None => false,
            },
            112 => relocation == descriptor.diagnostic_relocation(),
            _ => false,
        };
        if !target_matches {
            return descriptor_relocation_error(plan.exact_type(), offset);
        }
    }
    Ok(())
}

fn expected_relocation_offsets(plan: &StrongTypeRegistrationPlanV1) -> Vec<u64> {
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
