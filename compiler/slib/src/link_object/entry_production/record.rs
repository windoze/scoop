use scoop_lir::ExecutableEntryPlanV1;

pub(super) const ROOT_ENTRY_DESCRIPTOR_SIZE: usize = 192;
const ROOT_ENTRY_DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5045_4e54;
const METADATA_ABI_VERSION: u32 = 5;

pub(super) fn expected_root_entry_record(
    plan: &ExecutableEntryPlanV1,
) -> [u8; ROOT_ENTRY_DESCRIPTOR_SIZE] {
    let mut bytes = [0; ROOT_ENTRY_DESCRIPTOR_SIZE];
    write_u64(&mut bytes, 0, ROOT_ENTRY_DESCRIPTOR_MAGIC);
    write_u32(&mut bytes, 8, METADATA_ABI_VERSION);
    write_u32(&mut bytes, 12, ROOT_ENTRY_DESCRIPTOR_SIZE as u32);
    bytes[16..48].copy_from_slice(plan.root_cone().as_array());
    bytes[48..80].copy_from_slice(plan.main().body().as_array());
    bytes[112..144].copy_from_slice(plan.gateway().as_array());
    bytes
}

pub(super) fn expected_final_root_entry_record(
    plan: &ExecutableEntryPlanV1,
    gateway_definition: &[u8; 32],
) -> [u8; ROOT_ENTRY_DESCRIPTOR_SIZE] {
    let mut bytes = expected_root_entry_record(plan);
    bytes[80..112].copy_from_slice(plan.source_signature_fingerprint().as_array());
    bytes[144..176].copy_from_slice(gateway_definition);
    bytes
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
