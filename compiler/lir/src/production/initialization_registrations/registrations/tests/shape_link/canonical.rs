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
        )
        .unwrap()
    });
    let table = CanonicalExternalShapeLinkImportsV1::from_checked(imports.to_vec()).unwrap();
    let reversed =
        CanonicalExternalShapeLinkImportsV1::from_checked(imports.into_iter().rev().collect())
            .unwrap();
    assert_eq!(encode(&table).unwrap(), encode(&reversed).unwrap());
    assert!(matches!(
        CanonicalExternalShapeLinkImportsV1::from_checked(vec![imports[0], imports[0]]),
        Err(ShapeLinkError::Duplicate { .. })
    ));
    let bytes = encode(&table).unwrap();
    let raw: DecodedCanonicalExternalShapeLinkImportsV1 = decode_canonical(&bytes).unwrap();
    let replayed = raw.validate_against(&table).unwrap();
    assert_eq!(encode(&replayed).unwrap(), bytes);
    let mut wrong = vec![0x82];
    for import in table.records().iter().rev() {
        wrong.extend(encode(import).unwrap());
    }
    let raw: DecodedCanonicalExternalShapeLinkImportsV1 = decode_canonical(&wrong).unwrap();
    assert!(raw.validate_against(&table).is_err());
}
