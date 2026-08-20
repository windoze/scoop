use super::*;

#[test]
fn context_contract_round_trips_without_changing_declaration_identity_or_abi() {
    let fixture = Fixture::new();
    let original = fixture.record();
    let mut declaration = original.declaration_data().clone();
    declaration.context_parameters = vec![SourceParameterShapeV1::new(
        identifier("service"),
        binder(0),
    )];
    let contextual =
        CallableInterfaceRecordV1::from_declaration(declaration, original.access()).unwrap();
    let actual = decode_record(&contextual)
        .resolve(&mut fixture.authority())
        .unwrap();
    assert_eq!(actual, contextual);
    assert_eq!(actual.declaration(), original.declaration());
    assert_eq!(actual.parameters(), original.parameters());
    assert_eq!(actual.context_parameters()[0].name().as_str(), "service");
    assert_ne!(encode(&actual).unwrap(), encode(&original).unwrap());
}
