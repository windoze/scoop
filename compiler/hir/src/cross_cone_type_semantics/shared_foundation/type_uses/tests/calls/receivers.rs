use super::*;

#[test]
fn shared_call_receiver_demands_its_original_static_type_before_argument_adaptation() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    let mut source = Artifact::new(coordinate("receiver-provider"));
    let base = source.nominal("Base", SourceNominalKind::Class, &[]);
    let derived = source.nominal("Derived", SourceNominalKind::Class, &[base]);
    let mut provider = source.load(&[&core]);
    let member = provider.callable(nominal_owner(base), "run", CallForm::Function);
    let dependencies = dependencies(&core, &provider);
    let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
    consumer.calls(&provider, &[member, member]);
    consumer.change_last_receiver(exact(derived));
    let mut uses = signature_uses(provider.provider(), &[base, derived]);
    uses.extend([
        member_call(provider.provider(), base, member),
        member_call(provider.provider(), derived, member),
    ]);
    let expected = selected(uses);
    let actual = consumer.uses(&dependencies).unwrap();
    assert_eq!(actual, expected);
    consumer.validate(&actual, &dependencies).unwrap();
    let missing = selected(signature_uses(provider.provider(), &[base]));
    assert!(matches!(
        consumer.validate(&missing, &dependencies),
        Err(Error::TypeUseInventory)
    ));
    snapshot("call-receiver-static", &actual);
}
