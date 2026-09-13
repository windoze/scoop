use object::macho;
use scoop_identity::DefinitionAtomRole;

use crate::link_object::{PlannedMemberStrongObjectSymbolsV1, PlannedStrongObjectSymbolRoleV1};

use super::Corruption;

const TEXT_SIZE: u64 = 16;
const STACK_SIZE: u64 = 64;

pub(super) fn object_bytes(
    symbols: &PlannedMemberStrongObjectSymbolsV1,
    safepoints: &[u64],
    corruption: Corruption,
) -> Vec<u8> {
    let mut record_ids = [safepoints[1], safepoints[0]];
    if matches!(corruption, Corruption::UnknownSafepoint) {
        record_ids[0] = u64::MAX;
    }
    let stackmap = stackmap_blob(&record_ids);
    let text = match corruption {
        Corruption::MissingFrameChain => [0xd503_201f, 0x9100_03fd, 0x9400_0000, 0x9400_0000],
        Corruption::NonCallReturnPc => [0xa9bf_7bfd, 0x9100_03fd, 0x9400_0000, 0xd503_201f],
        Corruption::None | Corruption::UnknownSafepoint | Corruption::WrongStackmapAtomRole => {
            [0xa9bf_7bfd, 0x9100_03fd, 0x9400_0000, 0x9400_0000]
        }
    };
    macho_object(symbols, &text, &stackmap)
}

fn stackmap_blob(record_ids: &[u64; 2]) -> Vec<u8> {
    let mut bytes = vec![3, 0, 0, 0];
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 2);
    push_u64(&mut bytes, 0);
    push_u64(&mut bytes, STACK_SIZE);
    push_u64(&mut bytes, 2);
    push_record(&mut bytes, record_ids[0], 16);
    push_record(&mut bytes, record_ids[1], 12);
    bytes
}

fn push_record(bytes: &mut Vec<u8>, safepoint: u64, instruction_offset: u32) {
    push_u64(bytes, safepoint);
    push_u32(bytes, instruction_offset);
    push_u16(bytes, 0);
    push_u16(bytes, 3);
    for _ in 0..3 {
        bytes.push(4);
        bytes.push(0);
        push_u16(bytes, 8);
        push_u16(bytes, 0);
        push_u16(bytes, 0);
        push_u32(bytes, 0);
    }
    align_zero(bytes, 8);
    push_u16(bytes, 0);
    push_u16(bytes, 0);
    align_zero(bytes, 8);
}

fn macho_object(
    symbols: &PlannedMemberStrongObjectSymbolsV1,
    instructions: &[u32; 4],
    stackmap: &[u8],
) -> Vec<u8> {
    let segment_size = 72_u32 + 2 * 80;
    let command_bytes = segment_size + 24 + 80;
    let text_offset = 32 + command_bytes;
    let stackmap_offset = text_offset + u32::try_from(TEXT_SIZE).unwrap();
    let stackmap_size = u32::try_from(stackmap.len()).unwrap();
    let relocation_offset = stackmap_offset + stackmap_size;
    let symbol_offset = relocation_offset + 8;
    let symbol_bytes = u32::try_from(symbols.symbols().len() * 16).unwrap();
    let string_offset = symbol_offset + symbol_bytes;
    let mut strings = vec![0];
    let string_indexes = symbols
        .symbols()
        .iter()
        .map(|symbol| {
            let index = u32::try_from(strings.len()).unwrap();
            strings.extend_from_slice(symbol.macho_name());
            strings.push(0);
            index
        })
        .collect::<Vec<_>>();
    let string_size = u32::try_from(strings.len()).unwrap();
    let mut bytes = Vec::with_capacity((string_offset + string_size) as usize);

    push_header(&mut bytes, command_bytes);
    push_segment(&mut bytes, segment_size, text_offset, stackmap_size);
    push_section(
        &mut bytes,
        b"__text",
        b"__TEXT",
        0,
        u32::try_from(TEXT_SIZE).unwrap(),
        text_offset,
        2,
        0,
        0,
        macho::S_REGULAR | macho::S_ATTR_PURE_INSTRUCTIONS | macho::S_ATTR_SOME_INSTRUCTIONS,
    );
    push_section(
        &mut bytes,
        b"__llvm_stackmaps",
        b"__LLVM_STACKMAPS",
        TEXT_SIZE,
        stackmap_size,
        stackmap_offset,
        3,
        relocation_offset,
        1,
        macho::S_REGULAR,
    );
    push_symbol_commands(
        &mut bytes,
        symbols.symbols().len(),
        symbol_offset,
        string_offset,
        string_size,
    );

    for instruction in instructions {
        push_u32(&mut bytes, *instruction);
    }
    bytes.extend_from_slice(stackmap);
    let target = symbols
        .symbols()
        .iter()
        .position(|symbol| {
            matches!(
                symbol.role(),
                PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. }
            )
        })
        .unwrap();
    push_u32(&mut bytes, 16);
    push_u32(
        &mut bytes,
        u32::try_from(target).unwrap() | 3 << 25 | 1 << 27,
    );
    for (symbol, string_index) in symbols.symbols().iter().zip(string_indexes) {
        let (section, value) = symbol_location(symbol.role(), stackmap.len());
        push_u32(&mut bytes, string_index);
        bytes.push(macho::N_SECT | macho::N_EXT);
        bytes.push(section);
        push_u16(&mut bytes, 0);
        push_u64(&mut bytes, value);
    }
    bytes.extend_from_slice(&strings);
    bytes
}

fn push_header(bytes: &mut Vec<u8>, command_bytes: u32) {
    push_u32(bytes, macho::MH_MAGIC_64);
    push_u32(bytes, macho::CPU_TYPE_ARM64);
    push_u32(bytes, macho::CPU_SUBTYPE_ARM64_ALL);
    push_u32(bytes, macho::MH_OBJECT);
    push_u32(bytes, 3);
    push_u32(bytes, command_bytes);
    push_u32(bytes, macho::MH_SUBSECTIONS_VIA_SYMBOLS);
    push_u32(bytes, 0);
}

fn push_segment(bytes: &mut Vec<u8>, segment_size: u32, text_offset: u32, stackmap_size: u32) {
    push_u32(bytes, macho::LC_SEGMENT_64);
    push_u32(bytes, segment_size);
    bytes.extend_from_slice(&[0; 16]);
    push_u64(bytes, 0);
    push_u64(bytes, TEXT_SIZE + u64::from(stackmap_size));
    push_u64(bytes, u64::from(text_offset));
    push_u64(bytes, TEXT_SIZE + u64::from(stackmap_size));
    push_u32(bytes, 0);
    push_u32(bytes, 0);
    push_u32(bytes, 2);
    push_u32(bytes, 0);
}

#[allow(clippy::too_many_arguments)]
fn push_section(
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

fn push_symbol_commands(
    bytes: &mut Vec<u8>,
    symbol_count: usize,
    symbol_offset: u32,
    string_offset: u32,
    string_size: u32,
) {
    let symbol_count = u32::try_from(symbol_count).unwrap();
    push_u32(bytes, macho::LC_SYMTAB);
    push_u32(bytes, 24);
    push_u32(bytes, symbol_offset);
    push_u32(bytes, symbol_count);
    push_u32(bytes, string_offset);
    push_u32(bytes, string_size);

    push_u32(bytes, macho::LC_DYSYMTAB);
    push_u32(bytes, 80);
    push_u32(bytes, 0);
    push_u32(bytes, 0);
    push_u32(bytes, 0);
    push_u32(bytes, symbol_count);
    push_u32(bytes, symbol_count);
    push_u32(bytes, 0);
    bytes.extend_from_slice(&[0; 48]);
}

fn symbol_location(role: PlannedStrongObjectSymbolRoleV1, stackmap_size: usize) -> (u8, u64) {
    match role {
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            atom_role: DefinitionAtomRole::Primary,
            ..
        } => (1, 0),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            atom_role: DefinitionAtomRole::Primary,
            ..
        } => (1, TEXT_SIZE),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            atom_role: DefinitionAtomRole::Stackmap | DefinitionAtomRole::AddressTakenConstant,
            ..
        } => (2, TEXT_SIZE),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            atom_role: DefinitionAtomRole::Stackmap | DefinitionAtomRole::AddressTakenConstant,
            ..
        } => (2, TEXT_SIZE + u64::try_from(stackmap_size).unwrap()),
        _ => unreachable!("fixture has only primary and stackmap atoms"),
    }
}

fn align_zero(bytes: &mut Vec<u8>, alignment: usize) {
    while !bytes.len().is_multiple_of(alignment) {
        bytes.push(0);
    }
}

fn push_fixed_name(bytes: &mut Vec<u8>, name: &[u8]) {
    let mut fixed = [0; 16];
    fixed[..name.len()].copy_from_slice(name);
    bytes.extend_from_slice(&fixed);
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
