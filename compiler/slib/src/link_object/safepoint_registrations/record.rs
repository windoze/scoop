use scoop_lir::StrongSafepointRegistrationPlanV1;

use super::StrongSafepointRegistrationValidationError;

const DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5053_5054;
pub(in crate::link_object) const ABI_VERSION: u32 = 6;
pub(super) const NORMALIZED_STACKMAP_FINGERPRINT_OFFSET: usize = 168;
const DIGEST_WIDTH: usize = 32;
pub(in crate::link_object) const DESCRIPTOR_SIZE: usize = 200;

pub(super) fn validate_record_bytes(
    object: &[u8],
    file_start: u64,
    plan: StrongSafepointRegistrationPlanV1,
) -> Result<(), StrongSafepointRegistrationValidationError> {
    let start = usize::try_from(file_start).map_err(|_| {
        StrongSafepointRegistrationValidationError::RecordRangeOverflow(plan.site())
    })?;
    let end = start
        .checked_add(DESCRIPTOR_SIZE)
        .ok_or(StrongSafepointRegistrationValidationError::RecordRangeOverflow(plan.site()))?;
    let actual = object
        .get(start..end)
        .ok_or(StrongSafepointRegistrationValidationError::RecordRangeOverflow(plan.site()))?;
    let expected = expected_record(plan);
    if let Some(offset) = actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| *actual != expected)
    {
        return Err(
            StrongSafepointRegistrationValidationError::RecordByteMismatch {
                site: plan.site(),
                offset_within_atom: u16::try_from(offset).expect("descriptor offset fits u16"),
                expected: expected[offset],
                actual: actual[offset],
            },
        );
    }
    Ok(())
}

pub(super) fn expected_record(plan: StrongSafepointRegistrationPlanV1) -> [u8; DESCRIPTOR_SIZE] {
    let mut bytes = [0; DESCRIPTOR_SIZE];
    write_u64(&mut bytes, 0, DESCRIPTOR_MAGIC);
    write_u32(&mut bytes, 8, ABI_VERSION);
    write_u32(&mut bytes, 12, DESCRIPTOR_SIZE as u32);
    bytes[16..120].copy_from_slice(
        &crate::link_object::registration_identity::provisional_registration_identity(
            plan.site().as_array(),
            plan.definition_owner(),
        ),
    );
    write_u64(&mut bytes, 120, plan.safepoint().get());
    write_u32(&mut bytes, 128, plan.role().tag());
    write_u32(&mut bytes, 132, plan.root_pair_count());
    bytes[136..168].copy_from_slice(plan.owner().as_array());
    bytes
}

pub(in crate::link_object) fn expected_final_record(
    plan: StrongSafepointRegistrationPlanV1,
    stackmap: &[u8; DIGEST_WIDTH],
) -> [u8; DESCRIPTOR_SIZE] {
    let mut bytes = expected_record(plan);
    bytes[NORMALIZED_STACKMAP_FINGERPRINT_OFFSET
        ..NORMALIZED_STACKMAP_FINGERPRINT_OFFSET + DIGEST_WIDTH]
        .copy_from_slice(stackmap);
    bytes
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
