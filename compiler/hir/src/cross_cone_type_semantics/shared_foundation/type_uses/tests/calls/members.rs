use super::*;

mod negative;
mod objects;

#[test]
fn shared_member_calls_keep_the_declaration_provider_for_local_derived_receivers() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    for (provider_coordinate, name) in [
        (ConeCoordinate::reserved_core(), "core"),
        (coordinate("members"), "ordinary"),
    ] {
        let mut source = Artifact::new(provider_coordinate);
        let base = source.nominal("Base", SourceNominalKind::Class, &[]);
        let mut provider = source.load(&[]);
        let member = provider.callable(nominal_owner(base), "run", CallForm::Function);
        let dependencies = dependencies(&core, &provider);
        let mut source = Artifact::new(coordinate_for_consumer());
        let derived = source.nominal("Derived", SourceNominalKind::Class, &[base]);
        let mut consumer = source.load(&dependencies);
        consumer.calls(&provider, &[member, member]);
        consumer.change_last_receiver(exact(derived));
        let mut expected = signature_uses(provider.provider(), &[base]);
        expected.extend([
            member_call(provider.provider(), base, member),
            member_call(provider.provider(), derived, member),
            SelectedExternalTypeUseV1::new(
                provider.provider(),
                SelectedTypeUseV1::Inheritance {
                    derived: exact(derived),
                    edge: SelectedDirectInheritanceEdgeV1::ClassBase { exact: exact(base) },
                },
            ),
        ]);
        let actual = consumer.uses(&dependencies).unwrap();
        assert_eq!(actual, selected(expected));
        consumer.validate(&actual, &dependencies).unwrap();
        snapshot(&format!("member-local-{name}"), &actual);
    }
}

#[test]
fn shared_member_calls_follow_interface_diamonds_across_actual_providers() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    let mut source = Artifact::new(coordinate("interfaces"));
    let root = source.nominal("Root", SourceNominalKind::Interface, &[]);
    let left = source.nominal("Left", SourceNominalKind::Interface, &[root]);
    let right = source.nominal("Right", SourceNominalKind::Interface, &[root]);
    let mut interfaces = source.load(&[&core]);
    let member = interfaces.callable(nominal_owner(root), "run", CallForm::Function);
    let mut source = Artifact::new(coordinate("values"));
    let value = source.nominal("Value", SourceNominalKind::Struct, &[left, right]);
    let values = source.load(&[&core, &interfaces]);
    let dependencies = vec![&core, &interfaces, &values];
    let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
    consumer.calls(&interfaces, &[member, member]);
    consumer.change_last_receiver(exact(value));
    let mut expected = signature_uses(interfaces.provider(), &[root]);
    expected.extend([
        signature(values.provider(), value),
        representation(values.provider(), value),
        representation(interfaces.provider(), left),
        representation(interfaces.provider(), right),
        member_call(interfaces.provider(), root, member),
        member_call(interfaces.provider(), value, member),
    ]);
    let actual = consumer.uses(&dependencies).unwrap();
    assert_eq!(actual, selected(expected));
    consumer.validate(&actual, &dependencies).unwrap();
    snapshot("member-interface-diamond", &actual);
}
