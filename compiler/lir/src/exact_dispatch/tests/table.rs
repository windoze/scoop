use super::*;

#[test]
fn canonical_exports_borrow_foundation_and_reader_rejects_omission() {
    let fixture = DirectFixture::new(1);
    let mut resolver = fixture.local_resolver();
    let record = ExactDispatchExportV1::replay(
        TARGET,
        (&fixture.vtable).into(),
        &[fixture.identity_input()],
        &fixture.foundation,
        &mut resolver,
    )
    .unwrap();
    let table =
        CanonicalExactDispatchExportsV1::try_new(TARGET, &fixture.foundation, vec![record.clone()])
            .unwrap();
    assert_eq!(table.get(record.table()), Some(&record));

    let bytes = encode(&table).unwrap();
    let decoded = decode_canonical::<DecodedCanonicalExactDispatchExportsV1>(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(decoded.validate_against(&table).unwrap(), table);
    let omitted = decode_canonical::<DecodedCanonicalExactDispatchExportsV1>(b"\x80").unwrap();
    assert!(matches!(
        omitted.validate_against(&table),
        Err(ExactDispatchTableError::Count {
            expected: 1,
            actual: 0
        })
    ));

    let private_only =
        CanonicalExactDispatchExportsV1::try_new(TARGET, &fixture.foundation, Vec::new()).unwrap();
    assert!(private_only.records().is_empty());
    assert_eq!(fixture.foundation.dispatch_tables().len(), 1);
    assert!(matches!(
        CanonicalExactDispatchExportsV1::try_new(
            TARGET,
            &fixture.foundation,
            vec![record.clone(), record]
        ),
        Err(ExactDispatchTableError::Duplicate(_))
    ));

    let itable = empty_itable();
    let mut resolver = |_| Ok(None);
    let record = ExactDispatchExportV1::replay(
        TARGET,
        (&itable.table).into(),
        &[],
        &itable.foundation,
        &mut resolver,
    )
    .unwrap();
    assert_eq!(
        record.role(),
        ExactDispatchRoleV1::Itable {
            interface_exact: itable.interface
        }
    );
}
