use super::*;

#[test]
fn normalizes_direct_arm64_relocation_shapes() {
    let branch = inventory(
        &[0x11, 0x22, 0x33, 0x44],
        vec![raw(0, 0, true, 2, true, macho::ARM64_RELOC_BRANCH26)],
        1,
    )
    .unwrap();
    assert_eq!(branch.len(), 1);
    assert_eq!(branch[0].containing_section_ordinal().get(), 1);
    assert_eq!(branch[0].offset(), 0);
    assert_eq!(branch[0].encoded_value(), 0x4433_2211);
    assert_eq!(
        branch[0].shape(),
        DarwinArm64RelocationShapeV1::Branch26 {
            target: DarwinArm64RelocationTargetV1::SymbolTableIndex(0),
        }
    );

    let unsigned = inventory(
        &[0; 8],
        vec![raw(0, 1, false, 3, false, macho::ARM64_RELOC_UNSIGNED)],
        0,
    )
    .unwrap();
    assert_eq!(
        unsigned[0].shape(),
        DarwinArm64RelocationShapeV1::Unsigned64 {
            target: DarwinArm64RelocationTargetV1::SectionOrdinal(NonZeroU32::new(1).unwrap()),
        }
    );

    let tlvp = inventory(
        &[0; 8],
        vec![
            raw(0, 0, true, 2, true, macho::ARM64_RELOC_TLVP_LOAD_PAGE21),
            raw(4, 0, false, 2, true, macho::ARM64_RELOC_TLVP_LOAD_PAGEOFF12),
        ],
        1,
    )
    .unwrap();
    assert_eq!(
        tlvp[0].shape(),
        DarwinArm64RelocationShapeV1::TlvpLoadPage21 {
            target: DarwinArm64RelocationTargetV1::SymbolTableIndex(0),
        }
    );
    assert_eq!(
        tlvp[1].shape(),
        DarwinArm64RelocationShapeV1::TlvpLoadPageOffset12 {
            target: DarwinArm64RelocationTargetV1::SymbolTableIndex(0),
        }
    );
}

#[test]
fn collapses_subtractor_and_explicit_addend_pairs() {
    let subtractor = inventory(
        &[0; 8],
        vec![
            raw(0, 0, false, 3, true, macho::ARM64_RELOC_SUBTRACTOR),
            raw(0, 1, false, 3, true, macho::ARM64_RELOC_UNSIGNED),
        ],
        2,
    )
    .unwrap();
    assert_eq!(subtractor.len(), 1);
    assert_eq!(
        subtractor[0].shape(),
        DarwinArm64RelocationShapeV1::Subtractor64 {
            minuend: DarwinArm64RelocationTargetV1::SymbolTableIndex(1),
            subtrahend: DarwinArm64RelocationTargetV1::SymbolTableIndex(0),
        }
    );

    let addend = inventory(
        &[0; 4],
        vec![
            raw(0, 0x00ff_fffc, false, 2, false, macho::ARM64_RELOC_ADDEND),
            raw(0, 0, true, 2, true, macho::ARM64_RELOC_PAGE21),
        ],
        1,
    )
    .unwrap();
    assert_eq!(addend.len(), 1);
    assert_eq!(
        addend[0].shape(),
        DarwinArm64RelocationShapeV1::Page21 {
            target: DarwinArm64RelocationTargetV1::SymbolTableIndex(0),
            explicit_addend: Some(-4),
        }
    );
}

#[test]
fn rejects_scattered_unknown_and_wrong_field_combinations() {
    let mut scattered = raw(0, 0, true, 2, true, macho::ARM64_RELOC_BRANCH26);
    scattered[..4].copy_from_slice(&macho::R_SCATTERED.to_le_bytes());
    assert_eq!(
        inventory(&[0; 4], vec![scattered], 1),
        Err(DarwinArm64RelocationInventoryValidationError::ScatteredRelocation)
    );

    assert!(matches!(
        inventory(
            &[0; 4],
            vec![raw(
                0,
                0,
                false,
                2,
                true,
                macho::ARM64_RELOC_TLVP_LOAD_PAGE21,
            )],
            1,
        ),
        Err(
            DarwinArm64RelocationInventoryValidationError::InvalidRelocationFields {
                kind: macho::ARM64_RELOC_TLVP_LOAD_PAGE21,
                ..
            }
        )
    ));

    assert!(matches!(
        inventory(
            &[0; 4],
            vec![raw(0, 0, false, 2, true, macho::ARM64_RELOC_BRANCH26,)],
            1,
        ),
        Err(
            DarwinArm64RelocationInventoryValidationError::InvalidRelocationFields {
                kind: macho::ARM64_RELOC_BRANCH26,
                ..
            }
        )
    ));
}

#[test]
fn rejects_broken_pairs_targets_sites_and_duplicates() {
    assert_eq!(
        inventory(
            &[0; 4],
            vec![raw(0, 1, true, 2, true, macho::ARM64_RELOC_BRANCH26,)],
            1,
        ),
        Err(DarwinArm64RelocationInventoryValidationError::SymbolTargetOutOfBounds { index: 1 })
    );
    assert_eq!(
        inventory(
            &[0; 4],
            vec![raw(1, 0, true, 2, true, macho::ARM64_RELOC_BRANCH26)],
            1,
        ),
        Err(DarwinArm64RelocationInventoryValidationError::RelocationSiteOutOfBounds)
    );
    assert_eq!(
        inventory(
            &[0; 8],
            vec![raw(0, 0, false, 3, true, macho::ARM64_RELOC_SUBTRACTOR,)],
            1,
        ),
        Err(
            DarwinArm64RelocationInventoryValidationError::MissingRelocationPair {
                first: macho::ARM64_RELOC_SUBTRACTOR,
            }
        )
    );
    assert_eq!(
        inventory(
            &[0; 4],
            vec![
                raw(0, 0, true, 2, true, macho::ARM64_RELOC_BRANCH26),
                raw(0, 0, true, 2, true, macho::ARM64_RELOC_BRANCH26),
            ],
            1,
        ),
        Err(DarwinArm64RelocationInventoryValidationError::DuplicateRelocationOffset)
    );
}

fn inventory(
    section_bytes: &[u8],
    raw_relocations: Vec<[u8; 8]>,
    symbol_count: u32,
) -> Result<Vec<ObservedMachORelocationV1>, DarwinArm64RelocationInventoryValidationError> {
    let mut bytes = section_bytes.to_vec();
    let relocation_file_offset = bytes.len() as u64;
    for relocation in &raw_relocations {
        bytes.extend_from_slice(relocation);
    }
    let section = ObservedMachOSectionV1 {
        segment_name: fixed_name(b"__TEXT"),
        section_name: fixed_name(b"__text"),
        virtual_address: 0,
        file_offset: Some(0),
        relocation_file_offset,
        flags: macho::S_REGULAR,
        byte_size: section_bytes.len() as u64,
        alignment_power: 0,
        relocation_count: raw_relocations.len() as u32,
    };
    validate_darwin_arm64_relocation_inventory_v1(&bytes, &[section], symbol_count)
}

fn raw(offset: u32, target: u32, pcrel: bool, length: u8, external: bool, kind: u8) -> [u8; 8] {
    let word1 = target
        | u32::from(pcrel) << 24
        | u32::from(length) << 25
        | u32::from(external) << 27
        | u32::from(kind) << 28;
    let mut bytes = [0; 8];
    bytes[..4].copy_from_slice(&offset.to_le_bytes());
    bytes[4..].copy_from_slice(&word1.to_le_bytes());
    bytes
}

fn fixed_name(name: &[u8]) -> [u8; 16] {
    let mut result = [0; 16];
    result[..name.len()].copy_from_slice(name);
    result
}
