use super::*;
use scoop_identity::{
    ConeIdentity, ObjectDefinitionPlanKey, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_wire::{decode_canonical, encode, sha256};

fn definition(cone: ConeIdentity) -> ObjectDefinitionPlanId {
    ObjectDefinitionPlanId::from_key(
        &ObjectDefinitionPlanKey::strong(
            cone,
            StrongDefinitionEntity::cone_image(cone),
            StrongDefinitionRole::ImageDescriptor,
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn selected_records_keep_table_order_and_recount_each_table() {
    let selected = definition(ConeIdentity::CORE);
    let omitted = definition(
        scoop_identity::ConeCoordinate::reserved_single_file()
            .identity()
            .unwrap(),
    );
    let mut records = RuntimeImageRecords::new(vec![0x70, 0x71]);
    for table in 0..6 {
        records.push(table, omitted, vec![0xff]);
        records.push(table, selected, vec![table as u8, 0x22]);
        records.push(table, selected, vec![table as u8, 0x33]);
    }
    let mut expected = (DOMAIN.len() as u64).to_le_bytes().to_vec();
    expected.extend_from_slice(DOMAIN.as_bytes());
    expected.extend_from_slice(&[0x70, 0x71]);
    for table in 0..6 {
        expected.extend_from_slice(&2_u64.to_le_bytes());
        expected.extend_from_slice(&[table, 0x22, table, 0x33]);
    }
    let actual = records
        .fingerprint_for_definitions(&BTreeSet::from([selected]))
        .unwrap();
    assert_eq!(actual.as_array(), sha256(&expected).as_array());

    let bytes = encode(&records).unwrap();
    let decoded = decode_canonical::<DecodedRuntimeImageRecords>(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
}

#[test]
fn empty_selection_preserves_six_empty_tables() {
    let mut records = RuntimeImageRecords::new(vec![0x70]);
    records.push(5, definition(ConeIdentity::CORE), vec![0x20]);
    let mut expected = (DOMAIN.len() as u64).to_le_bytes().to_vec();
    expected.extend_from_slice(DOMAIN.as_bytes());
    expected.push(0x70);
    expected.extend_from_slice(&[0; 6 * 8]);
    let actual = records
        .fingerprint_for_definitions(&BTreeSet::new())
        .unwrap();
    assert_eq!(actual.as_array(), sha256(&expected).as_array());
}
