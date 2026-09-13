use super::*;

#[test]
fn accepts_the_closed_relocatable_object_envelope() {
    let bytes = object_bytes();

    let envelope = validate_darwin_arm64_object_envelope_v1(&bytes).unwrap();

    assert_eq!(envelope.byte_length(), bytes.len() as u64);
    assert_eq!(envelope.load_command_count(), 4);
    assert_eq!(envelope.section_count(), 0);
    assert_eq!(envelope.symbol_count(), 0);
    assert_eq!(envelope.relocation_count(), 0);
    assert_eq!(
        envelope.deployment(),
        DarwinDeploymentCommandV1::BuildVersion {
            minimum_os: 0x000d_0000,
            sdk: 0x000d_0000,
            tool_count: 0,
        }
    );
}

#[test]
fn rejects_wrong_header_contracts() {
    let mut wrong_file_type = object_bytes();
    write_u32(&mut wrong_file_type, 12, macho::MH_EXECUTE);
    assert_eq!(
        validate_darwin_arm64_object_envelope_v1(&wrong_file_type),
        Err(ObjectEnvelopeValidationError::WrongFileType(
            macho::MH_EXECUTE
        ))
    );

    let mut missing_subsections = object_bytes();
    write_u32(&mut missing_subsections, 24, 0);
    assert_eq!(
        validate_darwin_arm64_object_envelope_v1(&missing_subsections),
        Err(ObjectEnvelopeValidationError::MissingSubsectionsViaSymbols)
    );
}

#[test]
fn rejects_embedded_linker_inputs_and_unknown_commands() {
    let deployment_offset = 32 + 72 + 24 + 80;
    let mut linker_option = object_bytes();
    write_u32(
        &mut linker_option,
        deployment_offset,
        macho::LC_LINKER_OPTION,
    );
    assert_eq!(
        validate_darwin_arm64_object_envelope_v1(&linker_option),
        Err(ObjectEnvelopeValidationError::EmbeddedLinkerOption)
    );

    let mut dylib = object_bytes();
    write_u32(&mut dylib, deployment_offset, macho::LC_LOAD_DYLIB);
    assert_eq!(
        validate_darwin_arm64_object_envelope_v1(&dylib),
        Err(ObjectEnvelopeValidationError::UnsupportedLoadCommand(
            macho::LC_LOAD_DYLIB
        ))
    );
}

#[test]
fn rejects_out_of_bounds_tables() {
    let mut bytes = object_bytes();
    let symtab_offset = 32 + 72;
    write_u32(&mut bytes, symtab_offset + 8, u32::MAX);
    write_u32(&mut bytes, symtab_offset + 12, 1);

    assert_eq!(
        validate_darwin_arm64_object_envelope_v1(&bytes),
        Err(ObjectEnvelopeValidationError::SymbolTableOutOfBounds)
    );
}

#[test]
fn validates_section_relocation_and_symbol_ranges_as_one_layout() {
    let bytes = object_with_text_section();

    let envelope = validate_darwin_arm64_object_envelope_v1(&bytes).unwrap();

    assert_eq!(envelope.section_count(), 1);
    assert_eq!(envelope.symbol_count(), 1);
    assert_eq!(envelope.relocation_count(), 1);
    assert_eq!(envelope.symbols().len(), 1);
    assert_eq!(envelope.symbols()[0].name(), b"_f");
    assert_eq!(
        envelope.symbols()[0].kind(),
        DarwinArm64SymbolKindV1::ExternalStrongDefinition
    );
}

#[test]
fn rejects_overlapping_physical_tables() {
    let mut bytes = object_with_text_section();
    let symtab_offset = 32 + 152;
    let section_offset = 32 + 152 + 24 + 80 + 24;
    write_u32(&mut bytes, symtab_offset + 8, section_offset as u32);

    assert_eq!(
        validate_darwin_arm64_object_envelope_v1(&bytes),
        Err(ObjectEnvelopeValidationError::OverlappingFileRanges)
    );
}

#[test]
fn rejects_noncanonical_section_name_padding() {
    let mut bytes = object_with_text_section();
    let section_name_offset = 32 + 72;
    bytes[section_name_offset + 7] = b'x';

    assert_eq!(
        validate_darwin_arm64_object_envelope_v1(&bytes),
        Err(ObjectEnvelopeValidationError::NonCanonicalSectionName)
    );
}

fn object_bytes() -> Vec<u8> {
    let segment_size = 72_u32;
    let symtab_size = 24_u32;
    let dysymtab_size = 80_u32;
    let deployment_size = 24_u32;
    let command_bytes = segment_size + symtab_size + dysymtab_size + deployment_size;
    let payload_offset = 32 + command_bytes;
    let mut bytes = Vec::with_capacity(payload_offset as usize + 1);

    push_u32(&mut bytes, macho::MH_MAGIC_64);
    push_u32(&mut bytes, macho::CPU_TYPE_ARM64);
    push_u32(&mut bytes, macho::CPU_SUBTYPE_ARM64_ALL);
    push_u32(&mut bytes, macho::MH_OBJECT);
    push_u32(&mut bytes, 4);
    push_u32(&mut bytes, command_bytes);
    push_u32(&mut bytes, macho::MH_SUBSECTIONS_VIA_SYMBOLS);
    push_u32(&mut bytes, 0);

    push_u32(&mut bytes, macho::LC_SEGMENT_64);
    push_u32(&mut bytes, segment_size);
    bytes.extend_from_slice(&[0; 16]);
    push_u64(&mut bytes, 0);
    push_u64(&mut bytes, 0);
    push_u64(&mut bytes, u64::from(payload_offset));
    push_u64(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);

    push_u32(&mut bytes, macho::LC_SYMTAB);
    push_u32(&mut bytes, symtab_size);
    push_u32(&mut bytes, payload_offset);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, payload_offset);
    push_u32(&mut bytes, 1);

    push_u32(&mut bytes, macho::LC_DYSYMTAB);
    push_u32(&mut bytes, dysymtab_size);
    bytes.extend_from_slice(&[0; 72]);

    push_u32(&mut bytes, macho::LC_BUILD_VERSION);
    push_u32(&mut bytes, deployment_size);
    push_u32(&mut bytes, macho::PLATFORM_MACOS);
    push_u32(&mut bytes, 0x000d_0000);
    push_u32(&mut bytes, 0x000d_0000);
    push_u32(&mut bytes, 0);

    bytes.push(0);
    bytes
}

pub(super) fn object_with_text_section() -> Vec<u8> {
    let segment_size = 152_u32;
    let symtab_size = 24_u32;
    let dysymtab_size = 80_u32;
    let deployment_size = 24_u32;
    let command_bytes = segment_size + symtab_size + dysymtab_size + deployment_size;
    let section_offset = 32 + command_bytes;
    let relocation_offset = section_offset + 4;
    let symbol_offset = relocation_offset + 8;
    let string_offset = symbol_offset + 16;
    let mut bytes = Vec::with_capacity(string_offset as usize + 4);

    push_u32(&mut bytes, macho::MH_MAGIC_64);
    push_u32(&mut bytes, macho::CPU_TYPE_ARM64);
    push_u32(&mut bytes, macho::CPU_SUBTYPE_ARM64_ALL);
    push_u32(&mut bytes, macho::MH_OBJECT);
    push_u32(&mut bytes, 4);
    push_u32(&mut bytes, command_bytes);
    push_u32(&mut bytes, macho::MH_SUBSECTIONS_VIA_SYMBOLS);
    push_u32(&mut bytes, 0);

    push_u32(&mut bytes, macho::LC_SEGMENT_64);
    push_u32(&mut bytes, segment_size);
    bytes.extend_from_slice(&[0; 16]);
    push_u64(&mut bytes, 0);
    push_u64(&mut bytes, 4);
    push_u64(&mut bytes, u64::from(section_offset));
    push_u64(&mut bytes, 4);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, 0);

    push_fixed_name(&mut bytes, b"__text");
    push_fixed_name(&mut bytes, b"__TEXT");
    push_u64(&mut bytes, 0);
    push_u64(&mut bytes, 4);
    push_u32(&mut bytes, section_offset);
    push_u32(&mut bytes, 2);
    push_u32(&mut bytes, relocation_offset);
    push_u32(&mut bytes, 1);
    push_u32(
        &mut bytes,
        macho::S_REGULAR | macho::S_ATTR_PURE_INSTRUCTIONS | macho::S_ATTR_SOME_INSTRUCTIONS,
    );
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);

    push_u32(&mut bytes, macho::LC_SYMTAB);
    push_u32(&mut bytes, symtab_size);
    push_u32(&mut bytes, symbol_offset);
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, string_offset);
    push_u32(&mut bytes, 4);

    push_u32(&mut bytes, macho::LC_DYSYMTAB);
    push_u32(&mut bytes, dysymtab_size);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, 0);
    bytes.extend_from_slice(&[0; 48]);

    push_u32(&mut bytes, macho::LC_BUILD_VERSION);
    push_u32(&mut bytes, deployment_size);
    push_u32(&mut bytes, macho::PLATFORM_MACOS);
    push_u32(&mut bytes, 0x000d_0000);
    push_u32(&mut bytes, 0x000d_0000);
    push_u32(&mut bytes, 0);

    bytes.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd]);
    bytes.extend_from_slice(&[0; 8]);
    push_u32(&mut bytes, 1);
    bytes.push(macho::N_SECT | macho::N_EXT);
    bytes.push(1);
    push_u16(&mut bytes, 0);
    push_u64(&mut bytes, 0);
    bytes.extend_from_slice(b"\0_f\0");
    bytes
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn push_fixed_name(bytes: &mut Vec<u8>, name: &[u8]) {
    let mut fixed = [0; 16];
    fixed[..name.len()].copy_from_slice(name);
    bytes.extend_from_slice(&fixed);
}
