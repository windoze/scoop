use scoop_lir::StrongImmortalObjectRegistrationPlanV1;

use super::StrongImmortalObjectRegistrationValidationError;

const DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5049_4d4d;
pub(in crate::link_object) const ABI_VERSION: u32 = 6;
pub(in crate::link_object) const DESCRIPTOR_SIZE: usize = 152;

pub(super) fn validate_record_bytes(
    object: &[u8],
    file_start: u64,
    plan: StrongImmortalObjectRegistrationPlanV1,
) -> Result<(), StrongImmortalObjectRegistrationValidationError> {
    let start = usize::try_from(file_start).map_err(|_| {
        StrongImmortalObjectRegistrationValidationError::RecordRangeOverflow(plan.object())
    })?;
    let end = start.checked_add(DESCRIPTOR_SIZE).ok_or(
        StrongImmortalObjectRegistrationValidationError::RecordRangeOverflow(plan.object()),
    )?;
    let actual = object.get(start..end).ok_or(
        StrongImmortalObjectRegistrationValidationError::RecordRangeOverflow(plan.object()),
    )?;
    let expected = expected_record(plan);
    if let Some(offset) = actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| *actual != expected)
    {
        return Err(
            StrongImmortalObjectRegistrationValidationError::RecordByteMismatch {
                object: plan.object(),
                offset_within_atom: u16::try_from(offset)
                    .expect("immortal descriptor offset fits u16"),
                expected: expected[offset],
                actual: actual[offset],
            },
        );
    }
    Ok(())
}

pub(super) fn expected_record(
    plan: StrongImmortalObjectRegistrationPlanV1,
) -> [u8; DESCRIPTOR_SIZE] {
    let mut bytes = [0; DESCRIPTOR_SIZE];
    write_u64(&mut bytes, 0, DESCRIPTOR_MAGIC);
    write_u32(&mut bytes, 8, ABI_VERSION);
    write_u32(&mut bytes, 12, DESCRIPTOR_SIZE as u32);
    match plan.definition_owner() {
        scoop_lir::RegistrationDefinitionOwner::Strong => write_u32(&mut bytes, 16, 1),
        scoop_lir::RegistrationDefinitionOwner::Odr { group, member } => {
            write_u32(&mut bytes, 16, 2);
            bytes[56..88].copy_from_slice(group.as_array());
            bytes[88..120].copy_from_slice(member.as_array());
        }
    }
    bytes[24..56].copy_from_slice(plan.object().as_array());
    write_u64(&mut bytes, 128, plan.object_size());
    write_u64(&mut bytes, 136, plan.required_alignment());
    bytes
}

pub(in crate::link_object) fn expected_final_record(
    plan: StrongImmortalObjectRegistrationPlanV1,
) -> [u8; DESCRIPTOR_SIZE] {
    expected_record(plan)
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
