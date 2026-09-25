use super::*;

#[test]
fn shared_member_calls_reject_later_unrelated_and_same_named_receivers() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    let mut source = Artifact::new(coordinate("members"));
    let owner = source.nominal("Owner", SourceNominalKind::Class, &[]);
    let unrelated = source.nominal("Unrelated", SourceNominalKind::Class, &[]);
    let mut provider = source.load(&[&core]);
    let member = provider.callable(nominal_owner(owner), "run", CallForm::Function);
    let mut source = Artifact::new(coordinate("impostors"));
    let impostor = source.nominal("Owner", SourceNominalKind::Class, &[]);
    let impostors = source.load(&[&core]);
    let dependencies = vec![&core, &provider, &impostors];
    for receiver in [
        unrelated,
        impostor,
        CoreBuiltinNominal::Any.identity_record().id(),
    ] {
        let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
        consumer.calls(&provider, &[member, member]);
        let actual = consumer.uses(&dependencies).unwrap();
        consumer.change_last_receiver(exact(receiver));
        for error in [
            consumer.uses(&dependencies).unwrap_err(),
            consumer
                .validate(&actual, &dependencies, &mut meter())
                .unwrap_err(),
        ] {
            assert!(matches!(error, Error::MemberCallReceiver {
                position,
                receiver: crate::SourceCallReceiver::Receiver { static_type },
                owner: actual_owner,
            } if position.expression_index == 1
                && static_type == exact(receiver)
                && actual_owner == exact(owner)));
        }
    }
}

#[test]
fn shared_member_calls_require_the_exact_occurrence_derived_selected_partition() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    let mut source = Artifact::new(coordinate("members"));
    let owner = source.nominal("Owner", SourceNominalKind::Class, &[]);
    let other = source.nominal("Other", SourceNominalKind::Class, &[]);
    let mut provider = source.load(&[&core]);
    let getter = provider.callable(nominal_owner(owner), "value", CallForm::Getter);
    let unused = provider.callable(nominal_owner(owner), "unused", CallForm::Getter);
    let dependencies = dependencies(&core, &provider);
    let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
    consumer.calls(&provider, &[getter]);
    let actual = consumer.uses(&dependencies).unwrap();
    let index = actual
        .records()
        .iter()
        .position(|record| matches!(record.usage(), SelectedTypeUseV1::MemberCall { .. }))
        .unwrap();
    let mut missing = actual.records().to_vec();
    missing.remove(index);
    reject(&consumer, &dependencies, missing);
    let InheritanceCallableDeclarationV1::Getter(id) = getter else {
        panic!("the fixture must provide a getter");
    };
    for replacement in [
        member_call(core.provider(), owner, getter),
        member_call(provider.provider(), other, getter),
        member_call(provider.provider(), owner, unused),
        member_call(
            provider.provider(),
            owner,
            InheritanceCallableDeclarationV1::Setter(id),
        ),
    ] {
        let mut changed = actual.records().to_vec();
        changed[index] = replacement;
        reject(&consumer, &dependencies, changed);
    }
    let mut extra = actual.records().to_vec();
    extra.push(member_call(provider.provider(), owner, unused));
    reject(&consumer, &dependencies, extra);
}

fn reject(consumer: &Loaded, dependencies: &[&Loaded], records: Vec<SelectedExternalTypeUseV1>) {
    assert!(matches!(
        consumer.validate(&selected(records), dependencies, &mut meter()),
        Err(Error::TypeUseInventory)
    ));
}
