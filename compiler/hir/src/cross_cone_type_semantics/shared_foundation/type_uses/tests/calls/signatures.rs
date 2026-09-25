use super::*;
use scoop_identity::NonEmptyVec;

#[test]
fn shared_call_signatures_close_compound_parameters_from_their_actual_provider() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    let mut source = Artifact::new(coordinate("values"));
    let value = source.nominal("Value", SourceNominalKind::Struct, &[]);
    let unused = source.nominal("Unused", SourceNominalKind::Class, &[]);
    let values = source.load(&[&core]);
    let mut provider = Artifact::new(coordinate("functions")).load(&[&core, &values]);
    let unit = CoreBuiltinNominal::Unit.identity_record().id();
    let tuple = SignatureTypeKey::Tuple(
        NonEmptyVec::new(vec![
            SignatureTypeKey::Nominal(value),
            SignatureTypeKey::Nominal(unit),
        ])
        .unwrap(),
    );
    let pointer = SignatureTypeKey::RawPointer(Box::new(SignatureTypeKey::Nominal(value)));
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_external_graph_authorities(&provider.identities)
        .unwrap();
    for key in [
        ExactTypeKey::Tuple(NonEmptyVec::new(vec![exact(value), exact(unit)]).unwrap()),
        ExactTypeKey::RawPointer(exact(value)),
    ] {
        pending
            .register_external_canonical_authority(
                CborIdentityRecord::<PersistentExactTypeId, _>::from_key(key).unwrap(),
            )
            .unwrap();
    }
    provider.identities = pending.finish().unwrap();
    let member = provider.callable_with_signature(
        PublicDeclarationOwnerV1::TopLevel,
        "consume",
        CallForm::Function,
        vec![tuple, pointer],
        SignatureTypeKey::Nominal(unit),
    );
    let dependencies = vec![&core, &values, &provider];
    let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
    consumer.calls(&provider, &[member]);
    let actual = consumer.uses(&dependencies).unwrap();
    assert_eq!(
        actual,
        selected(signature_uses(values.provider(), &[value]))
    );
    consumer
        .validate(&actual, &dependencies, &mut meter())
        .unwrap();
    let mut misdirected = signature_uses(values.provider(), &[value]);
    misdirected.push(signature(provider.provider(), value));
    misdirected.push(signature(values.provider(), unused));
    assert!(matches!(
        consumer.validate(&selected(misdirected), &dependencies, &mut meter()),
        Err(Error::TypeUseInventory)
    ));
    snapshot("call-signature-compound", &actual);
}
