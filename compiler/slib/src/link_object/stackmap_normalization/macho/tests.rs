use object::macho;

use super::*;
use crate::link_object::validate_scoop_lir_llvm_22_1_object_envelope_v1;

#[test]
fn qualifies_exact_function_address_relocations() {
    for kind in [SymbolKind::Definition, SymbolKind::WeakDefinition] {
        let object = stackmap_object(0, &[RelocationKind::Unsigned], kind);
        let sections = validated_sections(&object.bytes);

        let verified = verify_darwin_arm64_stackmap_section_v3(&object.bytes, &sections)
            .unwrap()
            .unwrap();

        assert_eq!(verified.section_ordinal().get(), 1);
        assert_eq!(verified.constants(), &[0xfeed, 0]);
        assert_eq!(verified.record_count(), 1);
        assert_eq!(verified.functions().len(), 1);
        assert_eq!(verified.functions()[0].target_symbol_table_index(), 0);
        assert_eq!(
            verified.functions()[0].parsed().function_address_offset(),
            16
        );
    }
}

#[test]
fn reports_absent_stackmap_section_without_fabricating_an_empty_proof() {
    let object = object_with_section(
        b"__TEXT",
        b"__text",
        macho::S_REGULAR | macho::S_ATTR_PURE_INSTRUCTIONS | macho::S_ATTR_SOME_INSTRUCTIONS,
        &[0xaa, 0xbb, 0xcc, 0xdd],
        &[],
        SymbolKind::Definition,
    );
    let sections = validated_sections(&object.bytes);

    assert_eq!(
        verify_darwin_arm64_stackmap_section_v3(&object.bytes, &sections),
        Ok(None)
    );

    let envelope =
        crate::link_object::validate_darwin_arm64_object_envelope_v1(&object.bytes).unwrap();
    let generated = crate::link_object::validate_builtin_object_section_inventory_v1(
        envelope,
        BuiltinLinkObjectSectionProfileV1::GeneratedCBridge,
    )
    .unwrap();
    assert_eq!(
        verify_darwin_arm64_stackmap_section_v3(&object.bytes, &generated),
        Err(DarwinArm64StackmapSectionError::WrongSectionProfile(
            BuiltinLinkObjectSectionProfileV1::GeneratedCBridge,
        ))
    );
}

#[test]
fn rejects_missing_wrong_extra_and_non_definition_relocations() {
    let missing = stackmap_object(0, &[], SymbolKind::Definition);
    let sections = validated_sections(&missing.bytes);
    assert_eq!(
        verify_darwin_arm64_stackmap_section_v3(&missing.bytes, &sections),
        Err(
            DarwinArm64StackmapSectionError::MissingFunctionAddressRelocation {
                index: 0,
                offset: 16,
            }
        )
    );

    let wrong = stackmap_object(0, &[RelocationKind::Branch], SymbolKind::Definition);
    let sections = validated_sections(&wrong.bytes);
    assert!(matches!(
        verify_darwin_arm64_stackmap_section_v3(&wrong.bytes, &sections),
        Err(DarwinArm64StackmapSectionError::InvalidFunctionAddressRelocation { index: 0, .. })
    ));

    let extra = object_with_section(
        b"__LLVM_STACKMAPS",
        b"__llvm_stackmaps",
        macho::S_REGULAR,
        &one_record_blob(0),
        &[
            Relocation {
                offset: 16,
                kind: RelocationKind::Unsigned,
            },
            Relocation {
                offset: 24,
                kind: RelocationKind::Unsigned,
            },
        ],
        SymbolKind::Definition,
    );
    let sections = validated_sections(&extra.bytes);
    assert_eq!(
        verify_darwin_arm64_stackmap_section_v3(&extra.bytes, &sections),
        Err(DarwinArm64StackmapSectionError::UnexpectedRelocation { offset: 24 })
    );

    let undefined = stackmap_object(0, &[RelocationKind::Unsigned], SymbolKind::Undefined);
    let sections = validated_sections(&undefined.bytes);
    assert_eq!(
        verify_darwin_arm64_stackmap_section_v3(&undefined.bytes, &sections),
        Err(
            DarwinArm64StackmapSectionError::InvalidFunctionTargetSymbol {
                index: 0,
                table_index: 0,
            }
        )
    );
}

#[test]
fn binds_parsing_to_exact_object_bytes_and_requires_zero_address_slots() {
    let relocated = stackmap_object(1, &[RelocationKind::Unsigned], SymbolKind::Definition);
    let sections = validated_sections(&relocated.bytes);
    assert_eq!(
        verify_darwin_arm64_stackmap_section_v3(&relocated.bytes, &sections),
        Err(DarwinArm64StackmapSectionError::PreRelocatedFunctionAddress { index: 0, value: 1 })
    );

    let mut malformed = stackmap_object(0, &[RelocationKind::Unsigned], SymbolKind::Definition);
    malformed.bytes[malformed.section_offset + 1] = 1;
    let sections = validated_sections(&malformed.bytes);
    assert!(matches!(
        verify_darwin_arm64_stackmap_section_v3(&malformed.bytes, &sections),
        Err(DarwinArm64StackmapSectionError::Parse(
            LlvmStackmapSectionParseError::NonZeroReserved { offset: 1, .. }
        ))
    ));

    let original = stackmap_object(0, &[RelocationKind::Unsigned], SymbolKind::Definition);
    let sections = validated_sections(&original.bytes);
    let mut changed = original.bytes;
    let last = changed.len() - 1;
    changed[last] ^= 1;
    assert_eq!(
        verify_darwin_arm64_stackmap_section_v3(&changed, &sections),
        Err(DarwinArm64StackmapSectionError::ObjectBytesMismatch)
    );
}

fn validated_sections(bytes: &[u8]) -> ValidatedBuiltinObjectSectionInventoryV1 {
    validate_scoop_lir_llvm_22_1_object_envelope_v1(bytes)
        .unwrap()
        .into_sections()
}

fn stackmap_object(
    function_address: u64,
    relocation_kinds: &[RelocationKind],
    symbol_kind: SymbolKind,
) -> ObjectFixture {
    let relocations = relocation_kinds
        .iter()
        .copied()
        .map(|kind| Relocation { offset: 16, kind })
        .collect::<Vec<_>>();
    object_with_section(
        b"__LLVM_STACKMAPS",
        b"__llvm_stackmaps",
        macho::S_REGULAR,
        &one_record_blob(function_address),
        &relocations,
        symbol_kind,
    )
}

#[derive(Clone, Copy)]
enum RelocationKind {
    Unsigned,
    Branch,
}

#[derive(Clone, Copy)]
struct Relocation {
    offset: u32,
    kind: RelocationKind,
}

#[derive(Clone, Copy)]
enum SymbolKind {
    Definition,
    WeakDefinition,
    Undefined,
}

struct ObjectFixture {
    bytes: Vec<u8>,
    section_offset: usize,
}

fn object_with_section(
    segment_name: &[u8],
    section_name: &[u8],
    section_flags: u32,
    section_bytes: &[u8],
    relocations: &[Relocation],
    symbol_kind: SymbolKind,
) -> ObjectFixture {
    let segment_size = 152_u32;
    let symtab_size = 24_u32;
    let dysymtab_size = 80_u32;
    let command_bytes = segment_size + symtab_size + dysymtab_size;
    let section_offset = 32 + command_bytes;
    let section_size = u32::try_from(section_bytes.len()).unwrap();
    let relocation_count = u32::try_from(relocations.len()).unwrap();
    let relocation_offset = section_offset + section_size;
    let symbol_offset = relocation_offset + relocation_count * 8;
    let string_offset = symbol_offset + 16;
    let strings = b"\0_f\0";
    let mut bytes = Vec::with_capacity(string_offset as usize + strings.len());

    push_u32(&mut bytes, macho::MH_MAGIC_64);
    push_u32(&mut bytes, macho::CPU_TYPE_ARM64);
    push_u32(&mut bytes, macho::CPU_SUBTYPE_ARM64_ALL);
    push_u32(&mut bytes, macho::MH_OBJECT);
    push_u32(&mut bytes, 3);
    push_u32(&mut bytes, command_bytes);
    push_u32(&mut bytes, macho::MH_SUBSECTIONS_VIA_SYMBOLS);
    push_u32(&mut bytes, 0);

    push_u32(&mut bytes, macho::LC_SEGMENT_64);
    push_u32(&mut bytes, segment_size);
    bytes.extend_from_slice(&[0; 16]);
    push_u64(&mut bytes, 0);
    push_u64(&mut bytes, u64::from(section_size));
    push_u64(&mut bytes, u64::from(section_offset));
    push_u64(&mut bytes, u64::from(section_size));
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, 0);

    push_fixed_name(&mut bytes, section_name);
    push_fixed_name(&mut bytes, segment_name);
    push_u64(&mut bytes, 0);
    push_u64(&mut bytes, u64::from(section_size));
    push_u32(&mut bytes, section_offset);
    push_u32(&mut bytes, 3);
    push_u32(
        &mut bytes,
        if relocations.is_empty() {
            0
        } else {
            relocation_offset
        },
    );
    push_u32(&mut bytes, relocation_count);
    push_u32(&mut bytes, section_flags);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);

    push_u32(&mut bytes, macho::LC_SYMTAB);
    push_u32(&mut bytes, symtab_size);
    push_u32(&mut bytes, symbol_offset);
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, string_offset);
    push_u32(&mut bytes, u32::try_from(strings.len()).unwrap());

    push_u32(&mut bytes, macho::LC_DYSYMTAB);
    push_u32(&mut bytes, dysymtab_size);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    match symbol_kind {
        SymbolKind::Definition | SymbolKind::WeakDefinition => {
            push_u32(&mut bytes, 0);
            push_u32(&mut bytes, 1);
            push_u32(&mut bytes, 1);
            push_u32(&mut bytes, 0);
        }
        SymbolKind::Undefined => {
            push_u32(&mut bytes, 0);
            push_u32(&mut bytes, 0);
            push_u32(&mut bytes, 0);
            push_u32(&mut bytes, 1);
        }
    }
    bytes.extend_from_slice(&[0; 48]);

    bytes.extend_from_slice(section_bytes);
    for relocation in relocations {
        push_u32(&mut bytes, relocation.offset);
        let fields = match relocation.kind {
            RelocationKind::Unsigned => 3 << 25 | 1 << 27,
            RelocationKind::Branch => 1 << 24 | 2 << 25 | 1 << 27 | 2 << 28,
        };
        push_u32(&mut bytes, fields);
    }
    push_u32(&mut bytes, 1);
    match symbol_kind {
        SymbolKind::Definition | SymbolKind::WeakDefinition => {
            bytes.push(macho::N_SECT | macho::N_EXT);
            bytes.push(1);
        }
        SymbolKind::Undefined => {
            bytes.push(macho::N_UNDF | macho::N_EXT);
            bytes.push(0);
        }
    }
    push_u16(
        &mut bytes,
        if matches!(symbol_kind, SymbolKind::WeakDefinition) {
            macho::N_WEAK_DEF
        } else {
            0
        },
    );
    push_u64(&mut bytes, 0);
    bytes.extend_from_slice(strings);
    ObjectFixture {
        bytes,
        section_offset: section_offset as usize,
    }
}

fn one_record_blob(function_address: u64) -> Vec<u8> {
    let mut bytes = vec![3, 0, 0, 0];
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, 2);
    push_u32(&mut bytes, 1);
    push_u64(&mut bytes, function_address);
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
