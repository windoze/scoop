use scoop_lir::StrongStaticStorageRegistrationPlanV1;

use super::StrongStaticStorageRegistrationValidationError;

const DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5053_544f;
const ABI_VERSION: u32 = 1;
const STRONG_LINKAGE: u32 = 1;
pub(super) const DESCRIPTOR_SIZE: usize = 296;

pub(super) fn validate_record_bytes(
    object: &[u8],
    file_start: u64,
    plan: &StrongStaticStorageRegistrationPlanV1,
    template_addend: u64,
    relocation_table_addend: u64,
) -> Result<(), StrongStaticStorageRegistrationValidationError> {
    let start = usize::try_from(file_start).map_err(|_| {
        StrongStaticStorageRegistrationValidationError::RecordRangeOverflow(
            plan.semantic().storage(),
        )
    })?;
    let end = start.checked_add(DESCRIPTOR_SIZE).ok_or(
        StrongStaticStorageRegistrationValidationError::RecordRangeOverflow(
            plan.semantic().storage(),
        ),
    )?;
    let actual = object.get(start..end).ok_or(
        StrongStaticStorageRegistrationValidationError::RecordRangeOverflow(
            plan.semantic().storage(),
        ),
    )?;
    let expected = expected_record(plan, template_addend, relocation_table_addend);
    if let Some(offset) = actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| *actual != expected)
    {
        return Err(
            StrongStaticStorageRegistrationValidationError::RecordByteMismatch {
                storage: plan.semantic().storage(),
                offset_within_atom: u16::try_from(offset)
                    .expect("static-storage descriptor offset fits u16"),
                expected: expected[offset],
                actual: actual[offset],
            },
        );
    }
    Ok(())
}

pub(super) fn expected_record(
    plan: &StrongStaticStorageRegistrationPlanV1,
    template_addend: u64,
    relocation_table_addend: u64,
) -> [u8; DESCRIPTOR_SIZE] {
    let semantic = plan.semantic();
    let mut bytes = [0; DESCRIPTOR_SIZE];
    write_u64(&mut bytes, 0, DESCRIPTOR_MAGIC);
    write_u32(&mut bytes, 8, ABI_VERSION);
    write_u32(&mut bytes, 12, DESCRIPTOR_SIZE as u32);
    write_u32(&mut bytes, 16, STRONG_LINKAGE);
    bytes[24..56].copy_from_slice(semantic.storage().as_array());
    write_u32(&mut bytes, 152, semantic.scan_kind().tag());
    write_u32(&mut bytes, 156, semantic.initial_state().tag());
    write_u64(&mut bytes, 168, semantic.byte_size());
    write_u64(&mut bytes, 176, semantic.allocation_extent());
    write_u64(&mut bytes, 184, semantic.required_alignment());
    write_u64(&mut bytes, 264, template_addend);
    write_u64(
        &mut bytes,
        272,
        u64::try_from(semantic.initial_state().initial_template().len()).unwrap(),
    );
    write_u64(&mut bytes, 280, relocation_table_addend);
    write_u64(
        &mut bytes,
        288,
        u64::try_from(semantic.initial_state().immortal_relocations().len()).unwrap(),
    );
    bytes
}

pub(in crate::link_object) fn expected_final_record(
    plan: &StrongStaticStorageRegistrationPlanV1,
    template_addend: u64,
    relocation_table_addend: u64,
    registration: &[u8; 32],
    scan: &[u8; 32],
    layout: &[u8; 32],
) -> [u8; DESCRIPTOR_SIZE] {
    let mut bytes = expected_record(plan, template_addend, relocation_table_addend);
    bytes[120..152].copy_from_slice(registration);
    bytes[200..232].copy_from_slice(scan);
    bytes[232..264].copy_from_slice(layout);
    bytes
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
