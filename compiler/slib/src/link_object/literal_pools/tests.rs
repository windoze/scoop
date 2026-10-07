use super::*;
use crate::link_object::{
    BuiltinLinkObjectSectionProfileV1, ObjectEnvelopeFormatV1, ObjectSectionFlagsV1,
    ObjectSymbolAttributesV1, ObservedObjectRelocationV1, ObservedObjectSymbolV1,
    ValidatedObjectEnvelopeV1,
};

#[test]
fn local_table_preserves_the_complete_referenced_byte_span() {
    let (bytes, sections) = table();
    let pools = LiteralPools::read(&bytes, &sections, &[]).unwrap();
    let ordinal = NonZeroU32::new(2).unwrap();
    assert!(pools.contains_section(ordinal));
    assert_eq!(pools.at(ordinal, 4).unwrap().bytes(), &bytes[4..]);
    assert!(pools.at(ordinal, 5).is_none());
    assert!(pools.at(ordinal, 24).is_none());
}

#[test]
fn unreferenced_writable_and_relocated_data_are_not_local_tables() {
    let (bytes, sections) = table();
    let mut unreferenced = sections.clone();
    unreferenced.envelope.relocations.clear();
    let mut writable = sections.clone();
    writable.envelope.sections[1].segment_name = b"__DATA".to_vec();
    writable.envelope.sections[1].section_name = b"__data".to_vec();
    writable.roles[1] = BuiltinObjectSectionRoleV1::WritableData;
    let mut relocated = sections;
    let mut relocation = relocated.envelope.relocations[0];
    relocation.containing_section_ordinal = NonZeroU32::new(2).unwrap();
    relocated.envelope.relocations.push(relocation);
    for sections in [unreferenced, writable, relocated] {
        assert!(
            LiteralPools::read(&bytes, &sections, &[])
                .unwrap()
                .0
                .is_empty()
        );
    }
}

#[test]
fn fixed_width_literals_keep_their_element_bounds() {
    let (bytes, mut sections) = table();
    sections.envelope.sections[1].section_name = b"__literal4".to_vec();
    let pools = LiteralPools::read(&bytes, &sections, &[]).unwrap();
    let ordinal = NonZeroU32::new(2).unwrap();
    assert_eq!(pools.at(ordinal, 4).unwrap().bytes(), &[1, 2, 3, 4]);
    assert_eq!(pools.at(ordinal, 8).unwrap().bytes(), &[5, 6, 7, 8]);
    assert!(pools.at(ordinal, 5).is_none());
    assert!(pools.at(ordinal, 24).is_none());
}

fn table() -> (Vec<u8>, ValidatedBuiltinObjectSectionInventoryV1) {
    let bytes: Vec<u8> = [0, 0, 0, 0].into_iter().chain(1..=20).collect();
    let sections = vec![
        ObservedObjectSectionV1 {
            segment_name: b"__TEXT".to_vec(),
            section_name: b"__text".to_vec(),
            virtual_address: 0,
            file_offset: Some(0),
            flags: ObjectSectionFlagsV1::MachO(object::macho::S_ATTR_PURE_INSTRUCTIONS),
            byte_size: 4,
            alignment_power: 2,
        },
        ObservedObjectSectionV1 {
            segment_name: b"__TEXT".to_vec(),
            section_name: b"__const".to_vec(),
            virtual_address: 4,
            file_offset: Some(4),
            flags: ObjectSectionFlagsV1::MachO(object::macho::S_REGULAR),
            byte_size: 20,
            alignment_power: 0,
        },
    ];
    let envelope = ValidatedObjectEnvelopeV1 {
        byte_length: bytes.len() as u64,
        content_digest: scoop_wire::sha256(&bytes),
        format: ObjectEnvelopeFormatV1::DarwinArm64 {
            load_command_count: 1,
            deployment: None,
        },
        sections,
        symbols: vec![ObservedObjectSymbolV1 {
            table_index: 0,
            name: b"ltmp1".to_vec(),
            kind: ObjectSymbolKindV1::LocalSectionDefinition,
            section_ordinal: NonZeroU32::new(2),
            value: 4,
            attributes: ObjectSymbolAttributesV1::MachO {
                no_dead_strip: false,
                private_external: false,
            },
        }],
        relocations: vec![ObservedObjectRelocationV1 {
            containing_section_ordinal: NonZeroU32::new(1).unwrap(),
            offset: 0,
            encoded_value: 0,
            shape: ObjectRelocationShapeV1::DarwinArm64(DarwinArm64RelocationShapeV1::Page21 {
                target: DarwinArm64RelocationTargetV1::SymbolTableIndex(0),
                explicit_addend: None,
            }),
        }],
    };
    (
        bytes,
        ValidatedBuiltinObjectSectionInventoryV1 {
            envelope,
            profile: BuiltinLinkObjectSectionProfileV1::ScoopLir,
            roles: vec![
                BuiltinObjectSectionRoleV1::Text,
                BuiltinObjectSectionRoleV1::ReadOnlyData,
            ],
        },
    )
}
