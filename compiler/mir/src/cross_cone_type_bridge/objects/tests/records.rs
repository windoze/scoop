use super::*;

#[test]
fn object_and_companion_keep_one_provider_owned_ensure_and_read_plan() {
    let fixture = Fixture::new();
    for index in 0..2 {
        let record = fixture.object(index);
        assert_eq!(record.provider(), fixture.objects[index].key().origin());
        assert_eq!(record.read().object(), exact(fixture.objects[index].id()));
        assert_eq!(record.backing(), exact(fixture.backings[index].id()));
        assert_eq!(record.unit(), fixture.units[index].id());
        let binding = fixture.callables.get(record.ensure()).unwrap();
        assert_eq!(
            binding.lowered_signature().exact().result(),
            fixture.unit_exact
        );
        assert!(!binding.lowered_signature().exact().receiver().is_present());
    }
}

#[test]
fn mismatched_object_backing_unit_ensure_and_read_are_rejected() {
    let fixture = Fixture::new();
    let source = fixture.object(0);
    let other = fixture.object(1);
    for axis in 0..4 {
        let result = ParamFreeMirObjectValueV1::try_new(
            fixture.authority(),
            source.value(),
            if axis == 0 {
                other.backing()
            } else {
                source.backing()
            },
            if axis == 1 {
                other.unit()
            } else {
                source.unit()
            },
            if axis == 2 {
                other.ensure()
            } else {
                source.ensure()
            },
            if axis == 3 {
                other.read()
            } else {
                source.read()
            },
            &mut meter(),
        );
        assert!(matches!(
            (axis, result),
            (0 | 3, Err(MirObjectBridgeError::ObjectIdentity))
                | (1, Err(MirObjectBridgeError::UnitIdentity))
                | (2, Err(MirObjectBridgeError::EnsureIdentity))
        ));
    }
}

#[test]
fn object_table_retains_canonical_value_keys_and_rejects_duplicates() {
    let fixture = Fixture::new();
    let table = CanonicalMirObjectValuesV1::try_new(
        vec![fixture.object(1), fixture.object(0)],
        &mut meter(),
    )
    .unwrap();
    assert!(table.records()[0].value() < table.records()[1].value());
    assert_eq!(table.get(fixture.values[0].id()), Some(&fixture.object(0)));
    assert!(matches!(
        CanonicalMirObjectValuesV1::try_new(
            vec![fixture.object(0), fixture.object(0)],
            &mut meter()
        ),
        Err(MirObjectBridgeError::DuplicateObject { .. })
    ));
}
