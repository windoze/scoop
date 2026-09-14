use scoop_lir::ConeImagePlanV1;

pub(super) const IMAGE_DESCRIPTOR_SIZE: usize = 240;
const IMAGE_DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5049_4d47;
const METADATA_ABI_VERSION: u32 = 1;

pub(super) fn expected_image_record(plan: &ConeImagePlanV1) -> [u8; IMAGE_DESCRIPTOR_SIZE] {
    let mut bytes = [0; IMAGE_DESCRIPTOR_SIZE];
    write_u64(&mut bytes, 0, IMAGE_DESCRIPTOR_MAGIC);
    write_u32(&mut bytes, 8, METADATA_ABI_VERSION);
    write_u32(&mut bytes, 12, IMAGE_DESCRIPTOR_SIZE as u32);
    bytes[64..96].copy_from_slice(plan.cone().identity().as_array());
    write_u64(
        &mut bytes,
        24,
        plan.cone().coordinate().group().len() as u64,
    );
    write_u64(&mut bytes, 40, plan.cone().coordinate().name().len() as u64);
    write_u64(
        &mut bytes,
        56,
        plan.cone().coordinate().version().len() as u64,
    );
    write_u64(&mut bytes, 136, plan.dependencies().len() as u64);
    write_u64(
        &mut bytes,
        152,
        plan.tables().static_storages().len() as u64,
    );
    write_u64(
        &mut bytes,
        168,
        plan.tables().immortal_objects().len() as u64,
    );
    write_u64(
        &mut bytes,
        184,
        plan.tables().initialization_units().len() as u64,
    );
    write_u64(
        &mut bytes,
        200,
        plan.tables().type_registrations().len() as u64,
    );
    write_u64(&mut bytes, 216, plan.tables().safepoints().len() as u64);
    write_u64(&mut bytes, 232, plan.tables().callables().len() as u64);
    bytes
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
