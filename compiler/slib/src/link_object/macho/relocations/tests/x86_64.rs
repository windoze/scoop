use super::*;

fn decode(
    data: &[u8],
    raw: &[[u8; 8]],
) -> Result<Vec<ObservedMachORelocationV1>, MachORelocationInventoryValidationError> {
    decode_macho_section_relocations_v1(
        MachOArchitecture::X86_64,
        NonZeroU32::new(1).unwrap(),
        data,
        &raw.concat(),
        2,
        2,
    )
}

#[test]
fn displacements_keep_the_instruction_bias_and_target_kind() {
    use MachOX86_64PcRelativeKindV1 as Kind;
    for (kind, trailing) in [
        (Kind::Signed, 0),
        (Kind::Branch, 0),
        (Kind::GotLoad, 0),
        (Kind::Got, 0),
        (Kind::Signed1, 1),
        (Kind::Signed2, 2),
        (Kind::Signed4, 4),
        (Kind::Tlv, 0),
    ] {
        let relocations = decode(
            &(-12_i32).to_le_bytes(),
            &[raw(0, 1, true, 2, true, kind as u8)],
        )
        .unwrap();
        assert_eq!(
            relocations[0].shape(),
            MachORelocationShapeV1::X86_64PcRelative32 {
                kind,
                target: ObjectRelocationTargetV1::SymbolTableIndex(1),
            }
        );
        assert_eq!(
            kind.section_target_address(relocations[0].encoded_value(), 32),
            Some(24 + trailing)
        );
    }
    let local = decode(
        &[0; 4],
        &[raw(0, 2, true, 2, false, macho::X86_64_RELOC_SIGNED)],
    )
    .unwrap();
    assert_eq!(
        local[0].shape(),
        MachORelocationShapeV1::X86_64PcRelative32 {
            kind: Kind::Signed,
            target: ObjectRelocationTargetV1::SectionOrdinal(NonZeroU32::new(2).unwrap()),
        }
    );
}

#[test]
fn absolute_and_subtractor_pairs_keep_their_width() {
    for length in [2, 3] {
        let width = 1 << length;
        let data = vec![0; width];
        let direct = decode(
            &data,
            &[raw(
                0,
                1,
                false,
                length,
                false,
                macho::X86_64_RELOC_UNSIGNED,
            )],
        )
        .unwrap();
        assert_eq!(usize::from(direct[0].shape().width_bytes()), width);
        let pair = decode(
            &data,
            &[
                raw(0, 0, false, length, true, macho::X86_64_RELOC_SUBTRACTOR),
                raw(0, 1, false, length, true, macho::X86_64_RELOC_UNSIGNED),
            ],
        )
        .unwrap();
        let minuend = ObjectRelocationTargetV1::SymbolTableIndex(1);
        let subtrahend = ObjectRelocationTargetV1::SymbolTableIndex(0);
        assert_eq!(pair.len(), 1);
        assert_eq!(
            pair[0].shape(),
            if length == 2 {
                MachORelocationShapeV1::Subtractor32 {
                    minuend,
                    subtrahend,
                }
            } else {
                MachORelocationShapeV1::Subtractor64 {
                    minuend,
                    subtrahend,
                }
            }
        );
    }
    let local = decode(
        &[0; 4],
        &[
            raw(0, 1, false, 2, false, macho::X86_64_RELOC_SUBTRACTOR),
            raw(0, 2, false, 2, false, macho::X86_64_RELOC_UNSIGNED),
        ],
    )
    .unwrap();
    assert_eq!(
        local[0].shape(),
        MachORelocationShapeV1::Subtractor32 {
            minuend: ObjectRelocationTargetV1::SectionOrdinal(NonZeroU32::new(2).unwrap()),
            subtrahend: ObjectRelocationTargetV1::SectionOrdinal(NonZeroU32::new(1).unwrap()),
        }
    );
}

#[test]
fn rejects_invalid_fields_and_incomplete_pairs() {
    for entry in [
        raw(0, 0, false, 2, true, macho::X86_64_RELOC_BRANCH),
        raw(0, 0, true, 3, true, macho::X86_64_RELOC_SIGNED),
        raw(0, 1, true, 2, false, macho::X86_64_RELOC_TLV),
        raw(0, 0, true, 3, true, macho::X86_64_RELOC_UNSIGNED),
    ] {
        assert!(matches!(
            decode(&[0; 8], &[entry]),
            Err(MachORelocationInventoryValidationError::InvalidRelocationFields { .. })
        ));
    }
    assert!(matches!(
        decode(
            &[0; 8],
            &[raw(0, 0, false, 3, true, macho::X86_64_RELOC_SUBTRACTOR)]
        ),
        Err(MachORelocationInventoryValidationError::MissingRelocationPair { .. })
    ));
    assert!(matches!(
        decode(
            &[0; 8],
            &[
                raw(0, 0, false, 3, true, macho::X86_64_RELOC_SUBTRACTOR),
                raw(0, 1, false, 2, true, macho::X86_64_RELOC_UNSIGNED),
            ]
        ),
        Err(MachORelocationInventoryValidationError::InvalidRelocationFields { .. })
    ));
    assert_eq!(
        decode(
            &[0; 4],
            &[raw(1, 0, true, 2, true, macho::X86_64_RELOC_BRANCH)]
        ),
        Err(MachORelocationInventoryValidationError::RelocationSiteOutOfBounds)
    );
}
