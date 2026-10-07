use super::*;

pub(super) fn push_header(bytes: &mut Vec<u8>, command_bytes: u32) {
    push_u32(bytes, macho::MH_MAGIC_64);
    push_u32(bytes, macho::CPU_TYPE_ARM64);
    push_u32(bytes, macho::CPU_SUBTYPE_ARM64_ALL);
    push_u32(bytes, macho::MH_OBJECT);
    push_u32(bytes, 3);
    push_u32(bytes, command_bytes);
    push_u32(bytes, macho::MH_SUBSECTIONS_VIA_SYMBOLS);
    push_u32(bytes, 0);
}

pub(super) fn push_segment(
    bytes: &mut Vec<u8>,
    segment_size: u32,
    text_offset: u32,
    stackmap_size: u32,
    registration_size: u32,
    writable_virtual_size: u32,
    writable_file_size: u32,
) {
    push_u32(bytes, macho::LC_SEGMENT_64);
    push_u32(bytes, segment_size);
    bytes.extend_from_slice(&[0; 16]);
    push_u64(bytes, 0);
    push_u64(
        bytes,
        TEXT_SIZE
            + u64::from(stackmap_size)
            + u64::from(registration_size)
            + u64::from(writable_virtual_size),
    );
    push_u64(bytes, u64::from(text_offset));
    push_u64(
        bytes,
        TEXT_SIZE
            + u64::from(stackmap_size)
            + u64::from(registration_size)
            + u64::from(writable_file_size),
    );
    push_u32(bytes, 0);
    push_u32(bytes, 0);
    push_u32(bytes, 4);
    push_u32(bytes, 0);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn push_section(
    bytes: &mut Vec<u8>,
    section: &[u8],
    segment: &[u8],
    address: u64,
    size: u32,
    offset: u32,
    alignment: u32,
    relocation_offset: u32,
    relocation_count: u32,
    flags: u32,
) {
    push_fixed_name(bytes, section);
    push_fixed_name(bytes, segment);
    push_u64(bytes, address);
    push_u64(bytes, u64::from(size));
    push_u32(bytes, offset);
    push_u32(bytes, alignment);
    push_u32(bytes, relocation_offset);
    push_u32(bytes, relocation_count);
    push_u32(bytes, flags);
    push_u32(bytes, 0);
    push_u32(bytes, 0);
    push_u32(bytes, 0);
}

pub(super) fn push_symbol_commands(
    bytes: &mut Vec<u8>,
    local_symbol_count: usize,
    defined_symbol_count: usize,
    undefined_symbol_count: usize,
    symbol_offset: u32,
    string_offset: u32,
    string_size: u32,
) {
    let local_symbol_count = u32::try_from(local_symbol_count).unwrap();
    let defined_symbol_count = u32::try_from(defined_symbol_count).unwrap();
    let undefined_symbol_count = u32::try_from(undefined_symbol_count).unwrap();
    let symbol_count = local_symbol_count + defined_symbol_count + undefined_symbol_count;
    push_u32(bytes, macho::LC_SYMTAB);
    push_u32(bytes, 24);
    push_u32(bytes, symbol_offset);
    push_u32(bytes, symbol_count);
    push_u32(bytes, string_offset);
    push_u32(bytes, string_size);

    push_u32(bytes, macho::LC_DYSYMTAB);
    push_u32(bytes, 80);
    push_u32(bytes, 0);
    push_u32(bytes, local_symbol_count);
    push_u32(bytes, local_symbol_count);
    push_u32(bytes, defined_symbol_count);
    push_u32(bytes, local_symbol_count + defined_symbol_count);
    push_u32(bytes, undefined_symbol_count);
    bytes.extend_from_slice(&[0; 48]);
}
