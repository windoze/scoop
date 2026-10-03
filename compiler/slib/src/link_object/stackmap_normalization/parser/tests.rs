use super::*;

const FUNCTION_COUNT_OFFSET: usize = 4;
const RECORD_COUNT_OFFSET: usize = 12;
const FIRST_FUNCTION_RECORD_COUNT_OFFSET: usize = 32;
const FIRST_LOCATION_RESERVED_OFFSET: usize = 73;
const POST_LOCATION_PADDING_OFFSET: usize = 132;
const PRE_LIVE_OUT_PADDING_OFFSET: usize = 136;
const LIVE_OUT_COUNT_OFFSET: usize = 138;
const LIVE_OUT_RESERVED_OFFSET: usize = 142;

#[test]
fn parses_one_complete_v3_blob_without_normalizing_payload() {
    let bytes = one_record_blob();
    let section = parse_llvm_stackmap_section_v3(&bytes).unwrap();

    assert_eq!(section.constants(), &[0xfeed, 0]);
    assert_eq!(section.record_count(), 1);
    let function = &section.functions()[0];
    assert_eq!(function.function_address_offset(), 16);
    assert_eq!(function.encoded_function_address(), 0);
    assert_eq!(function.stack_size(), 64);
    let record = &function.records()[0];
    assert_eq!(record.record_offset(), 56);
    assert_eq!(record.safepoint_id(), 42);
    assert_eq!(record.instruction_offset(), 8);
    assert_eq!(record.flags(), 0);
    assert_eq!(record.locations().len(), 5);
    assert_eq!(record.locations()[1].kind(), 5);
    assert_eq!(record.locations()[1].offset(), 1);
    assert_eq!(record.locations()[3].dwarf_register(), 31);
    assert_eq!(record.live_outs().len(), 1);
    assert_eq!(record.live_outs()[0].dwarf_register(), 19);
    assert_eq!(record.live_outs()[0].size(), 8);
}

#[test]
fn accepts_the_structurally_valid_empty_v3_blob() {
    let mut bytes = vec![3, 0, 0, 0];
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);

    let section = parse_llvm_stackmap_section_v3(&bytes).unwrap();
    assert!(section.constants().is_empty());
    assert!(section.functions().is_empty());
    assert_eq!(section.record_count(), 0);
}

#[test]
fn rejects_version_count_truncation_and_trailing_bytes() {
    let mut wrong_version = one_record_blob();
    wrong_version[0] = 2;
    assert_eq!(
        parse_llvm_stackmap_section_v3(&wrong_version),
        Err(LlvmStackmapSectionParseError::UnsupportedVersion(2))
    );

    let mut declared_count = one_record_blob();
    write_u32(&mut declared_count, RECORD_COUNT_OFFSET, 2);
    assert_eq!(
        parse_llvm_stackmap_section_v3(&declared_count),
        Err(LlvmStackmapSectionParseError::RecordCountMismatch {
            declared: 2,
            observed: 1,
        })
    );

    let mut function_count = one_record_blob();
    write_u64(&mut function_count, FIRST_FUNCTION_RECORD_COUNT_OFFSET, 2);
    assert_eq!(
        parse_llvm_stackmap_section_v3(&function_count),
        Err(LlvmStackmapSectionParseError::RecordCountMismatch {
            declared: 1,
            observed: 2,
        })
    );

    let mut impossible_table = one_record_blob();
    write_u32(&mut impossible_table, FUNCTION_COUNT_OFFSET, u32::MAX);
    assert!(matches!(
        parse_llvm_stackmap_section_v3(&impossible_table),
        Err(LlvmStackmapSectionParseError::Truncated {
            what: "function table",
            ..
        })
    ));

    let mut truncated = one_record_blob();
    truncated.pop();
    assert!(matches!(
        parse_llvm_stackmap_section_v3(&truncated),
        Err(LlvmStackmapSectionParseError::Truncated {
            what: "live-out table",
            ..
        })
    ));

    let mut trailing = one_record_blob();
    trailing.push(0);
    assert_eq!(
        parse_llvm_stackmap_section_v3(&trailing),
        Err(LlvmStackmapSectionParseError::TrailingBytes {
            offset: 144,
            count: 1,
        })
    );
}

#[test]
fn rejects_every_nonzero_reserved_and_alignment_region() {
    for (offset, what) in [
        (1, "header reserved bytes"),
        (FIRST_LOCATION_RESERVED_OFFSET, "location reserved byte"),
        (POST_LOCATION_PADDING_OFFSET, "post-location padding"),
        (PRE_LIVE_OUT_PADDING_OFFSET, "pre-live-out padding"),
        (LIVE_OUT_RESERVED_OFFSET, "live-out reserved byte"),
    ] {
        let mut bytes = one_record_blob();
        bytes[offset] = 1;
        assert_eq!(
            parse_llvm_stackmap_section_v3(&bytes),
            Err(LlvmStackmapSectionParseError::NonZeroReserved {
                offset: offset as u64,
                what,
            })
        );
    }

    let mut post_live_out = one_record_blob();
    write_u16(&mut post_live_out, LIVE_OUT_COUNT_OFFSET, 0);
    assert_eq!(
        parse_llvm_stackmap_section_v3(&post_live_out),
        Err(LlvmStackmapSectionParseError::NonZeroReserved {
            offset: 140,
            what: "post-live-out padding",
        })
    );
}

fn one_record_blob() -> Vec<u8> {
    let mut bytes = vec![3, 0, 0, 0];
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, 2);
    push_u32(&mut bytes, 1);

    push_u64(&mut bytes, 0);
    push_u64(&mut bytes, 64);
    push_u64(&mut bytes, 1);

    push_u64(&mut bytes, 0xfeed);
    push_u64(&mut bytes, 0);

    push_u64(&mut bytes, 42);
    push_u32(&mut bytes, 8);
    push_u16(&mut bytes, 0);
    push_u16(&mut bytes, 5);
    push_location(&mut bytes, 4, 8, 0, 0);
    push_location(&mut bytes, 5, 8, 0, 1);
    push_location(&mut bytes, 4, 8, 0, 0);
    push_location(&mut bytes, 3, 8, 31, 16);
    push_location(&mut bytes, 3, 8, 31, 16);
    align_zero(&mut bytes, 8);
    push_u16(&mut bytes, 0);
    push_u16(&mut bytes, 1);
    push_u16(&mut bytes, 19);
    bytes.push(0);
    bytes.push(8);
    align_zero(&mut bytes, 8);
    assert_eq!(bytes.len(), 144);
    bytes
}

fn push_location(bytes: &mut Vec<u8>, kind: u8, size: u16, register: u16, offset: i32) {
    bytes.push(kind);
    bytes.push(0);
    push_u16(bytes, size);
    push_u16(bytes, register);
    push_u16(bytes, 0);
    bytes.extend_from_slice(&offset.to_le_bytes());
}

fn align_zero(bytes: &mut Vec<u8>, alignment: usize) {
    while bytes.len() % alignment != 0 {
        bytes.push(0);
    }
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
