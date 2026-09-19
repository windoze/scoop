use super::*;

#[test]
fn shape_link_canonical_table_sorts_real_imports_and_rejects_duplicate_or_wire_order() {
    let fixture = ProviderFixture::new(false);
    let provider = fixture.provider();
    let support = fixture.support(true);
    let definitions = consumer();
    let imports = [
        Subject::InitializationDescriptor(fixture.unit().unit()),
        Subject::StaticStorage(fixture.unit().storage()),
    ]
    .map(|subject| {
        ExternalShapeLinkImportV1::replay(
            &provider,
            subject,
            ConeIdentity::CORE,
            &definitions,
            &support,
            &mut meter(),
        )
        .unwrap()
    });
    let table =
        CanonicalExternalShapeLinkImportsV1::from_checked(imports.to_vec(), &mut meter()).unwrap();
    let reversed = CanonicalExternalShapeLinkImportsV1::from_checked(
        imports.into_iter().rev().collect(),
        &mut meter(),
    )
    .unwrap();
    assert_eq!(encode(&table).unwrap(), encode(&reversed).unwrap());
    assert!(matches!(
        CanonicalExternalShapeLinkImportsV1::from_checked(
            vec![imports[0], imports[0]],
            &mut meter()
        ),
        Err(ShapeLinkError::Duplicate { .. })
    ));
    let bytes = encode(&table).unwrap();
    let raw: DecodedCanonicalExternalShapeLinkImportsV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let replayed = raw.validate_against(&table, &mut meter()).unwrap();
    assert_eq!(encode(&replayed).unwrap(), bytes);
    let mut wrong = vec![0x82];
    for import in table.records().iter().rev() {
        wrong.extend(encode(import).unwrap());
    }
    let raw: DecodedCanonicalExternalShapeLinkImportsV1 =
        decode_canonical(&wrong, DecodeLimits::default()).unwrap();
    assert!(raw.validate_against(&table, &mut meter()).is_err());
}

#[test]
fn shape_link_replay_has_inclusive_shared_work_and_heap_boundaries() {
    let fixture = ProviderFixture::new(false);
    let provider = fixture.provider();
    let subject = Subject::InitializationCell(fixture.unit().unit());
    let definitions = consumer();
    let support = fixture.support(true);
    let replay = |meter: &mut BudgetMeter| {
        ExternalShapeLinkImportV1::replay(
            &provider,
            subject,
            ConeIdentity::CORE,
            &definitions,
            &support,
            meter,
        )
    };
    let mut baseline = meter();
    let import = replay(&mut baseline).unwrap();
    let usage = baseline.usage();
    let mut limits = DecodeLimits {
        validation_work_units: usage.validation_work_units,
        logical_heap_bytes: usage.logical_heap_bytes,
        ..DecodeLimits::default()
    };
    let mut shared = BudgetMeter::new(limits);
    replay(&mut shared).unwrap();
    assert!(matches!(
        replay(&mut shared),
        Err(ShapeLinkError::Resource(_)
            | ShapeLinkError::Definition(StrongShapeDefinitionError::Resource(_)))
    ));
    limits.validation_work_units -= 1;
    assert!(matches!(
        replay(&mut BudgetMeter::new(limits)),
        Err(ShapeLinkError::Resource(_)
            | ShapeLinkError::Definition(StrongShapeDefinitionError::Resource(_)))
    ));
    let mut baseline = meter();
    CanonicalExternalShapeLinkImportsV1::from_checked(vec![import], &mut baseline).unwrap();
    let usage = baseline.usage();
    let limits = DecodeLimits {
        logical_heap_bytes: usage.logical_heap_bytes - 1,
        ..DecodeLimits::default()
    };
    assert!(matches!(
        CanonicalExternalShapeLinkImportsV1::from_checked(
            vec![import],
            &mut BudgetMeter::new(limits)
        ),
        Err(ShapeLinkError::Resource(_)
            | ShapeLinkError::Definition(StrongShapeDefinitionError::Resource(_)))
    ));
}
