use super::*;

#[test]
fn default_table_preserves_complete_templates_and_exact_protected_key_index() {
    let f = Fixture::new();
    let first = template(&f, 1);
    let second = template(&f, 2);
    let table =
        CanonicalProtectedDefaultTemplatesV1::try_new(vec![second.clone(), first.clone()]).unwrap();
    assert_eq!(table.keys().keys(), &[first.key(), second.key()]);
    assert_eq!(table.keys().index(second.key()).unwrap().get(), 1);
    assert_eq!(table.get(second.key()), Some(&second));
    let bytes = encode(&table.index_locals().unwrap()).unwrap();
    let decoded: DecodedCanonicalProtectedDefaultTemplatesV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(decoded.resolve(&mut f.resolver()).unwrap(), table);
}

#[test]
fn default_table_rejects_duplicate_and_descending_input_without_repair() {
    let f = Fixture::new();
    let first = template(&f, 1);
    let second = template(&f, 2);
    assert!(matches!(
        CanonicalProtectedDefaultTemplatesV1::try_new(vec![first.clone(), first.clone()]),
        Err(ProtectedDefaultTemplateTableBuildError::Duplicate(_))
    ));
    for raw in [Raw(&[&first, &first]), Raw(&[&second, &first])] {
        assert!(matches!(
            decoded(&raw).resolve(&mut f.resolver()),
            Err(ProtectedDefaultTemplateTableResolutionError::Build(
                ProtectedDefaultTemplateTableBuildError::Duplicate(_)
                    | ProtectedDefaultTemplateTableBuildError::NonCanonicalOrder { .. }
            ))
        ));
    }
}

#[test]
fn empty_default_table_encodes_explicitly_and_has_no_fake_key() {
    let f = Fixture::new();
    let empty = CanonicalProtectedDefaultTemplatesV1::try_new(vec![]).unwrap();
    assert_eq!(encode(&empty.index_locals().unwrap()).unwrap(), vec![0x80]);
    assert!(empty.keys().keys().is_empty());
    assert!(empty.get(template(&f, 1).key()).is_none());
    assert_eq!(
        decoded(&empty.index_locals().unwrap())
            .resolve(&mut f.resolver())
            .unwrap(),
        empty
    );
}
